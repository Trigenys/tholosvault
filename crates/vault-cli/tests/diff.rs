//! Non-régression de `tholos diff` : binaire réel, sorties texte et JSON.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tholos"))
        .args(args)
        .output()
        .expect("lancement de tholos")
}

fn pack(src: &Path, archive: &Path) {
    let o = run(&["pack", src.to_str().unwrap(), archive.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

/// Deux versions d'un même arbre : `garde.txt` inchangé, `change.txt` modifié,
/// `parti.txt` supprimé, `neuf.txt` ajouté.
fn versions(name: &str) -> (PathBuf, PathBuf) {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("diff-{name}"));
    let _ = fs::remove_dir_all(&d);
    let src = d.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("garde.txt"), "stable\n".repeat(10_000)).unwrap();
    fs::write(src.join("change.txt"), "avant\n".repeat(10_000)).unwrap();
    fs::write(src.join("parti.txt"), "adieu").unwrap();
    let v1 = d.join("v1.vault");
    pack(&src, &v1);
    fs::write(src.join("change.txt"), "après\n".repeat(12_000)).unwrap();
    fs::remove_file(src.join("parti.txt")).unwrap();
    fs::write(src.join("neuf.txt"), "bonjour").unwrap();
    let v2 = d.join("v2.vault");
    pack(&src, &v2);
    (v1, v2)
}

#[test]
fn json_lists_changes_and_transfer() {
    let (v1, v2) = versions("json");
    let o = run(&["diff", v1.to_str().unwrap(), v2.to_str().unwrap(), "--json"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["schema"], "tholos-diff/1");
    assert_eq!(v["identical"], false);
    assert_eq!(v["same_chunking"], true);
    assert_eq!(v["counts"]["added"], 1);
    assert_eq!(v["counts"]["removed"], 1);
    assert_eq!(v["counts"]["modified"], 1);
    assert_eq!(v["counts"]["unchanged"], 1);
    let files = v["files"].as_array().unwrap();
    // Sans --all, les fichiers inchangés sont omis.
    assert_eq!(files.len(), 3);
    let by = |p: &str| files.iter().find(|f| f["path"] == p).unwrap()["change"].clone();
    assert_eq!(by("neuf.txt"), "added");
    assert_eq!(by("parti.txt"), "removed");
    assert_eq!(by("change.txt"), "modified");
    assert_eq!(v["transfer"]["chunks"], 2);
    assert!(v["reused"]["chunks"].as_u64().unwrap() >= 1);

    let o = run(&[
        "diff",
        v1.to_str().unwrap(),
        v2.to_str().unwrap(),
        "--json",
        "--all",
    ]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["files"].as_array().unwrap().len(), 4);
}

#[test]
fn text_output_marks_each_change() {
    let (v1, v2) = versions("texte");
    let o = run(&["diff", v1.to_str().unwrap(), v2.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("+ neuf.txt"), "{out}");
    assert!(out.contains("- parti.txt"), "{out}");
    assert!(out.contains("~ change.txt"), "{out}");
    assert!(!out.contains("garde.txt"), "{out}");
    assert!(out.contains("1 ajouté(s), 1 supprimé(s), 1 modifié(s), 1 inchangé(s)"));
    assert!(out.contains("à transférer   2 morceau(x)"), "{out}");
}

#[test]
fn same_archive_is_identical() {
    let (v1, _) = versions("ident");
    let o = run(&["diff", v1.to_str().unwrap(), v1.to_str().unwrap()]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("archives identiques"));
}

#[test]
fn missing_archive_is_an_actionable_error() {
    let (v1, _) = versions("absente");
    let o = run(&["diff", v1.to_str().unwrap(), "/nexiste/pas.vault"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("ouverture de /nexiste/pas.vault"));
}
