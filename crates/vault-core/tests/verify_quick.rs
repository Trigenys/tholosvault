//! Vérification rapide : métadonnées + CRC32C des shards + échantillon de
//! morceaux. Sur une archive abîmée, le verdict doit être celui de la
//! vérification complète.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;
use vault_core::reader::Archive;
use vault_core::{PackOptions, ParityOptions};

fn packed(parity: bool) -> (tempfile::TempDir, PathBuf) {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    let big = common::pseudo_random(3 * 1024 * 1024, 11);
    fs::write(src.join("gros.bin"), big).unwrap();
    let opts = if parity {
        PackOptions::default()
    } else {
        PackOptions {
            parity: ParityOptions::disabled(),
            ..PackOptions::default()
        }
    };
    let (archive, _) = common::pack_with(&src, "a.vault", &opts);
    (tmp, archive)
}

fn flip(path: &Path, offset: u64) {
    let mut b = fs::read(path).unwrap();
    b[offset as usize] ^= 0x5a;
    fs::write(path, b).unwrap();
}

/// Offset d'un octet au milieu de la frame du dernier morceau.
fn last_frame_byte(path: &Path) -> u64 {
    let ar = Archive::open(path).unwrap();
    let c = ar.chunks().iter().max_by_key(|c| c.offset).unwrap();
    c.offset + c.compressed_len as u64 / 2
}

#[test]
fn healthy_archive_checks_only_the_sample() {
    for parity in [true, false] {
        let (_tmp, path) = packed(parity);
        let ar = Archive::open(&path).unwrap();
        let total = ar.chunks().len() as u64;
        assert!(total > 8);
        let r = ar.verify_quick(4);
        assert!(r.is_ok(), "parité {parity}");
        assert_eq!(r.chunks_checked, 4);
        let d = r.damage.as_ref().unwrap();
        assert!(!d.needs_repair());
        assert_eq!(d.geometry.is_some(), parity);
        assert_eq!(ar.verify_quick(0).chunks_checked, 0);
        assert_eq!(ar.verify_quick(usize::MAX).chunks_checked, total);
    }
}

#[test]
fn damaged_frame_with_parity_escalates_to_full_verdict() {
    let (_tmp, path) = packed(true);
    flip(&path, last_frame_byte(&path));
    let ar = Archive::open(&path).unwrap();
    // Échantillon vide : seul le CRC des shards peut voir le dégât.
    let quick = ar.verify_quick(0);
    let full = ar.verify();
    assert!(!quick.is_ok());
    assert_eq!(quick, full);
    assert_eq!(quick.chunks_checked, ar.chunks().len() as u64);
    assert!(quick.repairable());
}

#[test]
fn damaged_chunk_index_with_parity_is_detected() {
    let (_tmp, path) = packed(true);
    let off = Archive::open(&path).unwrap().trailer().chunk_index_off;
    // Premier octet du hachage de l'entrée 0 (offset 16 dans l'entrée).
    flip(&path, off + 16);
    let ar = Archive::open(&path).unwrap();
    let quick = ar.verify_quick(0);
    assert!(!quick.is_ok());
    assert_eq!(quick, ar.verify());
}

#[test]
fn damaged_parity_shard_keeps_content_ok_and_flags_repair() {
    let (_tmp, path) = packed(true);
    let len = fs::metadata(&path).unwrap().len();
    // Dernier octet de la section de parité, juste avant les deux trailers.
    flip(&path, len - 2 * 128 - 1);
    let ar = Archive::open(&path).unwrap();
    let r = ar.verify_quick(2);
    assert!(r.is_ok());
    assert_eq!(r.chunks_checked, 2, "pas d'escalade pour la seule parité");
    let d = r.damage.as_ref().unwrap();
    assert!(d.needs_repair() && !d.bad_parity_shards.is_empty());
    assert!(r.repairable());
}

#[test]
fn without_parity_only_the_sample_sees_content() {
    let (_tmp, path) = packed(false);
    flip(&path, last_frame_byte(&path));
    let ar = Archive::open(&path).unwrap();
    let last = ar
        .chunks()
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| c.offset)
        .unwrap()
        .0 as u32;
    // Échantillon d'un morceau (le premier) : dégât hors échantillon non vu.
    // Limite documentée ; seule la vérification complète fait foi.
    if last != 0 {
        assert!(ar.verify_quick(1).is_ok());
    }
    // Échantillon couvrant tout : même verdict que la vérification complète.
    let r = ar.verify_quick(usize::MAX);
    assert_eq!(r.bad_chunks, [last]);
    assert_eq!(r, ar.verify());
    assert!(!r.repairable());
}
