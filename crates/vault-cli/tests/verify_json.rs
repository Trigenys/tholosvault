//! Non-régression de `tholos verify --json` (schéma `tholos-verify/1`) :
//! archive saine, morceau corrompu puis réparé, en-tête illisible.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tholos"))
        .args(args)
        .output()
        .expect("lancement de tholos")
}

fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x & 0xff) as u8
        })
        .collect()
}

/// `gros.bin` et `copie.bin` identiques (morceaux partagés), `petit.txt`.
fn packed(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("verify-json-{name}"));
    let _ = fs::remove_dir_all(&d);
    let src = d.join("src");
    fs::create_dir_all(&src).unwrap();
    let big = noise(1024 * 1024, 5);
    fs::write(src.join("gros.bin"), &big).unwrap();
    fs::write(src.join("copie.bin"), &big).unwrap();
    fs::write(src.join("petit.txt"), "petit").unwrap();
    let archive = d.join("a.vault");
    let o = run(&["pack", src.to_str().unwrap(), archive.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    archive
}

fn verify_json(a: &Path, extra: &[&str]) -> (Output, Value) {
    let mut args = vec!["verify", a.to_str().unwrap(), "--json"];
    args.extend_from_slice(extra);
    let o = run(&args);
    let v = serde_json::from_slice(&o.stdout).unwrap_or_else(|e| {
        panic!(
            "JSON invalide ({e}) : {}",
            String::from_utf8_lossy(&o.stdout)
        )
    });
    (o, v)
}

fn flip(path: &Path, offset: u64) {
    let mut b = fs::read(path).unwrap();
    b[offset as usize] ^= 0xff;
    fs::write(path, b).unwrap();
}

#[test]
fn healthy_archive_full_and_quick() {
    let a = packed("sain");
    let (o, v) = verify_json(&a, &[]);
    assert!(o.status.success());
    assert_eq!(v["schema"], "tholos-verify/1");
    assert_eq!(v["mode"], "full");
    assert_eq!(v["ok"], true);
    assert_eq!(v["needs_repair"], false);
    assert_eq!(v["parity"], true);
    assert_eq!(v["files"], 3);
    assert_eq!(v["chunks_checked"], v["chunks_total"]);
    assert!(v["bad_chunks"].as_array().unwrap().is_empty());

    let (o, v) = verify_json(&a, &["--quick", "--sample", "1"]);
    assert!(o.status.success());
    assert_eq!(
        (v["mode"].as_str(), v["escalated"].as_bool()),
        (Some("quick"), Some(false))
    );
    assert_eq!(v["chunks_checked"], 1);
}

#[test]
fn corrupt_chunk_names_damaged_files_then_repair_clears_it() {
    let a = packed("abime");
    // Premier morceau : juste après l'en-tête de 64 octets.
    flip(&a, 64 + 100);

    let (o, v) = verify_json(&a, &[]);
    assert!(!o.status.success(), "contenu abîmé : code de sortie 1");
    assert!(String::from_utf8_lossy(&o.stderr).contains("vérification échouée (réparable)"));
    assert_eq!(v["ok"], false);
    assert_eq!(v["needs_repair"], true);
    assert_eq!(v["repairable"], true);
    assert_eq!(v["bad_chunks"].as_array().unwrap().len(), 1);
    let hit: Vec<&str> = v["damaged_files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    assert_eq!(
        hit,
        ["copie.bin", "gros.bin"],
        "morceau dédupliqué : deux fichiers"
    );
    assert!(!v["damage"]["bad_data_shards"]
        .as_array()
        .unwrap()
        .is_empty());

    // Rapide : le CRC voit le dégât et escalade vers la vérification complète.
    let (_, v) = verify_json(&a, &["--quick", "--sample", "1"]);
    assert_eq!(
        (v["mode"].as_str(), v["escalated"].as_bool()),
        (Some("full"), Some(true))
    );
    assert_eq!(v["ok"], false);

    let o = run(&["repair", a.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let (o, v) = verify_json(&a, &[]);
    assert!(o.status.success());
    assert_eq!(v["ok"], true);
}

#[test]
fn text_output_names_damaged_files() {
    let a = packed("texte");
    flip(&a, 64 + 100);
    let o = run(&["verify", a.to_str().unwrap()]);
    assert!(!o.status.success());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("fichier atteint : gros.bin"), "{err}");
    assert!(err.contains("fichier atteint : copie.bin"), "{err}");
}

#[test]
fn unreadable_header_still_reports_shards() {
    let a = packed("entete");
    flip(&a, 0); // magie de l'en-tête
    let (o, v) = verify_json(&a, &[]);
    assert!(!o.status.success());
    assert_eq!(v["readable"], false);
    assert_eq!(v["ok"], false);
    assert_eq!(v["repairable"], true);
    assert!(v["error"].as_str().is_some_and(|e| !e.is_empty()));
}
