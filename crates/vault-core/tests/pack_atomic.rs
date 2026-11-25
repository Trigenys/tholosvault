//! Archivage atomique et idempotent : la cible n'est jamais archivée
//! elle-même, un échec ne laisse ni fichier tronqué ni temporaire, et
//! l'archive précédente reste intacte jusqu'au remplacement.

mod common;

use std::fs;

use tempfile::tempdir;
use vault_core::reader::Archive;
use vault_core::writer::tmp_path;
use vault_core::{pack_dir, PackOptions};

#[test]
fn repacking_into_the_source_tree_is_idempotent() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    let dst = src.join("sortie.vault");

    let first = pack_dir(&src, &dst, &PackOptions::default()).unwrap();
    let bytes_first = fs::read(&dst).unwrap();
    // Seconde exécution : la cible (désormais dans l'arbre) ne doit pas s'archiver.
    let second = pack_dir(&src, &dst, &PackOptions::default()).unwrap();
    assert_eq!(first.archive_root, second.archive_root);
    assert_eq!(fs::read(&dst).unwrap(), bytes_first, "octets identiques");

    let ar = Archive::open(&dst).unwrap();
    assert!(ar.find("sortie.vault").is_none());
    assert!(ar.verify().is_ok());
    assert!(!tmp_path(&dst).exists(), "aucun temporaire laissé");
}

#[test]
fn target_named_through_another_path_is_still_excluded() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("d")).unwrap();
    common::fixture(&src);
    let dst = src.join("d").join("..").join("a.vault");
    let first = pack_dir(&src, &dst, &PackOptions::default()).unwrap();
    let second = pack_dir(&src, &src.join("a.vault"), &PackOptions::default()).unwrap();
    assert_eq!(first.archive_root, second.archive_root);
    assert!(Archive::open(&dst).unwrap().find("a.vault").is_none());
}

#[test]
fn leftover_temporary_from_a_crash_is_ignored_and_replaced() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    let dst = src.join("x.vault");
    let clean = pack_dir(&src, &dst, &PackOptions::default()).unwrap();
    fs::remove_file(&dst).unwrap();
    // Reste d'une construction interrompue, dans l'arbre source.
    fs::write(tmp_path(&dst), b"construction interrompue").unwrap();
    let again = pack_dir(&src, &dst, &PackOptions::default()).unwrap();
    assert_eq!(clean.archive_root, again.archive_root);
    assert!(!tmp_path(&dst).exists());
}

#[test]
fn failure_keeps_previous_target_and_leaves_no_temporary() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    // Cible occupée par un répertoire non vide : le renommage final échoue.
    let dst = tmp.path().join("occupe");
    fs::create_dir_all(dst.join("garde")).unwrap();
    fs::write(dst.join("garde/f"), b"intact").unwrap();
    let err = pack_dir(&src, &dst, &PackOptions::default()).unwrap_err();
    assert!(err.to_string().contains("occupe"), "{err}");
    assert_eq!(fs::read(dst.join("garde/f")).unwrap(), b"intact");
    assert!(!tmp_path(&dst).exists(), "temporaire supprimé après échec");
}

#[test]
fn failed_rebuild_does_not_truncate_existing_archive() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    let dst = tmp.path().join("a.vault");
    pack_dir(&src, &dst, &PackOptions::default()).unwrap();
    let before = fs::read(&dst).unwrap();
    // Source absente : échec avant toute écriture, archive précédente intacte.
    let missing = tmp.path().join("absent");
    assert!(pack_dir(&missing, &dst, &PackOptions::default()).is_err());
    assert_eq!(fs::read(&dst).unwrap(), before);
    assert!(Archive::open(&dst).unwrap().verify().is_ok());
}
