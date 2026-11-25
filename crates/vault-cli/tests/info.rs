//! Non-régression de `tholos info` : binaire réel, sortie texte et JSON
//! cohérentes avec `tholos pack` et la taille du fichier.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tholos"))
        .args(args)
        .output()
        .expect("lancement de tholos")
}

/// Archive de deux fichiers identiques ; renvoie (archive, racine affichée
/// par `pack`, taille d'un fichier).
fn packed(name: &str) -> (PathBuf, String, u64) {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("info-{name}"));
    let _ = fs::remove_dir_all(&d);
    let src = d.join("src");
    fs::create_dir_all(src.join("sous")).unwrap();
    let text = "ligne répétée pour la compression\n".repeat(20_000);
    fs::write(src.join("a.txt"), &text).unwrap();
    fs::write(src.join("sous/copie.txt"), &text).unwrap();
    let archive = d.join("a.vault");
    let o = run(&["pack", src.to_str().unwrap(), archive.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let root = String::from_utf8_lossy(&o.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("racine ").map(str::to_string))
        .expect("racine affichée par pack");
    (archive, root, text.len() as u64)
}

#[test]
fn json_output_is_consistent_with_the_file() {
    let (a, root, one) = packed("json");
    let o = run(&["info", a.to_str().unwrap(), "--json"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["schema"], "tholos-info/1");
    assert_eq!(v["files"], 2);
    assert_eq!(v["archive_root"], root.as_str());
    assert_eq!(
        v["archive_bytes"].as_u64().unwrap(),
        fs::metadata(&a).unwrap().len()
    );
    assert_eq!(v["logical_bytes"].as_u64().unwrap(), 2 * one);
    // Deux fichiers identiques : la moitié de la taille logique est dédupliquée.
    assert_eq!(v["dedup_saved_bytes"].as_u64().unwrap(), one);
    assert!(v["parity_bytes"].as_u64().unwrap() > 0);
    assert!(v["ratio"].as_f64().unwrap() > 2.0);
    assert_eq!(v["codec"]["name"], "zstd");
}

#[test]
fn text_output_names_every_figure() {
    let (a, root, _) = packed("texte");
    let o = run(&["info", a.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let out = String::from_utf8_lossy(&o.stdout);
    for key in [
        "format",
        "codec",
        "découpage",
        "fichiers",
        "taille logique",
        "morceaux",
        "déduplication",
        "métadonnées",
        "parité",
        "archive",
        "racine",
    ] {
        assert!(out.contains(key), "{key} absent de\n{out}");
    }
    assert!(out.contains(&root), "{out}");
    assert!(out.contains("% de l'archive"), "{out}");
}

#[test]
fn missing_archive_is_an_actionable_error() {
    let o = run(&["info", "absente-pour-le-test.vault"]);
    assert!(!o.status.success());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("ouverture de absente-pour-le-test.vault"),
        "{err}"
    );
}
