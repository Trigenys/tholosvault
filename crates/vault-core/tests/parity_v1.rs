//! Tests d'injection de corruptions du format v1 (L-004) : aléatoire 1–5 %,
//! contiguë 1–10 %, trailer, métadonnées, au-delà du budget. Sous Windows, la
//! vue mmap est toujours fermée avant toute réécriture (I-001).

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{fixture, pack_with, pseudo_random};
use vault_core::format::{HEADER_LEN, TRAILER_LEN};
use vault_core::{repair_file, scan_file, Archive, Error, PackOptions, ParityOptions};

fn opts(percent: u8) -> PackOptions {
    PackOptions {
        parity: ParityOptions {
            percent,
            ..ParityOptions::default()
        },
        ..PackOptions::default()
    }
}

/// Archive v1 au budget donné ; retourne (chemin, octets d'origine, longueur protégée).
fn archive(percent: u8) -> (tempfile::TempDir, PathBuf, Vec<u8>, u64) {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    fixture(&src);
    let (dst, _) = pack_with(&src, "a.tholos", &opts(percent));
    let original = fs::read(&dst).unwrap();
    let protected_len = {
        let ar = Archive::open(&dst).unwrap();
        ar.trailer().parity_off()
    };
    (tmp, dst, original, protected_len)
}

fn tmp_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".tmp");
    PathBuf::from(s)
}

/// Corrompt `n` shards de 4 KiB choisis pseudo-aléatoirement dans la zone protégée.
fn corrupt_random_shards(bytes: &mut [u8], protected_len: u64, n: usize, seed: u64) -> usize {
    let shards = protected_len.div_ceil(4096) as usize;
    let noise = pseudo_random(n * 8, seed);
    let mut hit = std::collections::BTreeSet::new();
    let mut k = 0;
    while hit.len() < n {
        let r = u64::from_le_bytes(noise[k * 8 % noise.len()..][..8].try_into().unwrap());
        k += 1;
        let i = (r.wrapping_mul(2654435761) % shards as u64) as usize;
        if hit.insert(i) {
            let start = i * 4096;
            let end = (start + 4096).min(protected_len as usize);
            for b in &mut bytes[start..end] {
                *b ^= 0xa5;
            }
        }
    }
    hit.len()
}

fn corrupt_contiguous(bytes: &mut [u8], protected_len: u64, start: usize, permille: u64) -> usize {
    let len = (protected_len * permille / 1000) as usize;
    for b in &mut bytes[start..start + len] {
        *b = 0xff;
    }
    len
}

fn assert_repaired(dst: &Path, original: &[u8], expected_shards: Option<u64>) {
    let report = repair_file(dst).unwrap();
    assert!(report.rewritten, "{report:?}");
    if let Some(n) = expected_shards {
        assert_eq!(report.patched_shards, n, "{report:?}");
    }
    assert!(report.verify_after.is_ok(), "{:?}", report.verify_after);
    assert_eq!(
        fs::read(dst).unwrap(),
        original,
        "octets identiques après réparation"
    );
    assert!(
        !tmp_path(dst).exists(),
        "pas de fichier temporaire résiduel"
    );
    // Idempotence : une seconde réparation ne réécrit rien.
    let again = repair_file(dst).unwrap();
    assert!(!again.rewritten);
    assert_eq!(again.patched_shards, 0);
}

#[test]
fn clean_v1_archive_has_parity_and_verifies() {
    let (_tmp, dst, original, protected_len) = archive(5);
    let ar = Archive::open(&dst).unwrap();
    assert_eq!(ar.header().format_version, 1);
    assert!(ar.has_parity());
    let parity_len = ar.trailer().parity_len;
    assert!(parity_len > 0);
    assert!(
        parity_len < protected_len / 10,
        "parité {parity_len} o pour {protected_len} o protégés"
    );
    let report = ar.verify();
    assert!(report.is_ok());
    assert!(report.repairable());
    let d = report.damage.unwrap();
    assert!(!d.needs_repair());
    assert!(d.parity_valid);
    assert_eq!(scan_file(&dst).unwrap(), d);
    assert_eq!(
        original.len() as u64,
        protected_len + parity_len + 2 * TRAILER_LEN as u64
    );
}

#[test]
fn deterministic_with_parity_l001() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    fixture(&src);
    let (a, sa) = pack_with(&src, "a.tholos", &opts(5));
    let (b, sb) = pack_with(&src, "b.tholos", &opts(5));
    assert_eq!(sa, sb);
    assert!(sa.parity_bytes > 0);
    assert_eq!(fs::read(a).unwrap(), fs::read(b).unwrap());
}

#[test]
fn v0_archive_without_parity_is_still_readable() {
    let (_tmp, dst, _, _) = archive(0);
    let mut bytes = fs::read(&dst).unwrap();
    bytes[8] = 0; // version 0 (le trailer v0 porte déjà parity_len = 0)
    bytes[9] = 0;
    fs::write(&dst, &bytes).unwrap();
    let ar = Archive::open(&dst).unwrap();
    assert_eq!(ar.header().format_version, 0);
    assert!(!ar.has_parity());
    assert_eq!(ar.read_file("z/profond/petit.dat").unwrap(), b"abc");
    let report = ar.verify();
    assert!(report.is_ok() && report.repairable());
    let d = report.damage.unwrap();
    assert!(d.geometry.is_none() && !d.needs_repair());
}

#[test]
fn random_shard_corruption_1_percent_is_repaired() {
    let (_tmp, dst, original, protected_len) = archive(5);
    let mut bytes = original.clone();
    let n = (protected_len.div_ceil(4096) / 100).max(1) as usize;
    corrupt_random_shards(&mut bytes, protected_len, n, 1);
    fs::write(&dst, &bytes).unwrap();
    assert_repaired(&dst, &original, Some(n as u64));
}

#[test]
fn random_shard_corruption_2_percent_is_repaired_via_archive_repair() {
    let (_tmp, dst, original, protected_len) = archive(5);
    let mut bytes = original.clone();
    let n = (protected_len.div_ceil(4096) * 2 / 100).max(1) as usize;
    corrupt_random_shards(&mut bytes, protected_len, n, 7);
    fs::write(&dst, &bytes).unwrap();

    let ar = Archive::open(&dst).unwrap();
    let report = ar.verify();
    assert!(!report.is_ok() || report.damage.as_ref().unwrap().needs_repair());
    assert!(report.repairable(), "{report:?}");
    let repair = ar.repair().unwrap(); // consomme la vue mmap avant d'écrire (I-001)
    assert_eq!(repair.patched_shards, n as u64);
    assert_eq!(fs::read(&dst).unwrap(), original);
}

#[test]
fn random_shard_corruption_5_percent_is_repaired_at_15_percent_budget() {
    let (_tmp, dst, original, protected_len) = archive(15);
    let mut bytes = original.clone();
    let n = (protected_len.div_ceil(4096) * 5 / 100) as usize;
    corrupt_random_shards(&mut bytes, protected_len, n, 3);
    fs::write(&dst, &bytes).unwrap();
    assert_repaired(&dst, &original, Some(n as u64));
}

#[test]
fn contiguous_corruption_1_3_5_percent_is_repaired() {
    for (permille, seed) in [(10u64, 11usize), (30, 500_000), (50, 123_456)] {
        let (_tmp, dst, original, protected_len) = archive(5);
        let mut bytes = original.clone();
        corrupt_contiguous(&mut bytes, protected_len, seed, permille);
        fs::write(&dst, &bytes).unwrap();
        let d = scan_file(&dst).unwrap();
        assert!(d.repairable, "{permille} ‰ : {d:?}");
        assert_repaired(&dst, &original, None);
    }
}

#[test]
fn contiguous_corruption_10_percent_is_repaired_at_15_percent_budget() {
    let (_tmp, dst, original, protected_len) = archive(15);
    let mut bytes = original.clone();
    corrupt_contiguous(&mut bytes, protected_len, 70_000, 100);
    fs::write(&dst, &bytes).unwrap();
    assert_repaired(&dst, &original, None);
}

#[test]
fn header_and_tables_corruption_blocks_open_but_repairs() {
    let (_tmp, dst, original, protected_len) = archive(5);
    let mut bytes = original.clone();
    bytes[0..HEADER_LEN].fill(0); // en-tête détruit
    let idx_off = Archive::open(&dst).unwrap().trailer().chunk_index_off as usize;
    bytes[idx_off..idx_off + 100].fill(0x11); // index des morceaux abîmé
    assert!(idx_off + 100 < protected_len as usize);
    fs::write(&dst, &bytes).unwrap();
    assert!(Archive::open(&dst).is_err());
    let d = scan_file(&dst).unwrap();
    assert!(d.repairable && d.needs_repair(), "{d:?}");
    assert_repaired(&dst, &original, None);
}

#[test]
fn destroyed_trailer_copy_is_restored() {
    let (_tmp, dst, original, _) = archive(5);
    let mut bytes = original.clone();
    let n = bytes.len();
    bytes[n - TRAILER_LEN..].fill(0xff);
    fs::write(&dst, &bytes).unwrap();
    let ar = Archive::open(&dst).unwrap();
    let report = ar.verify();
    assert!(report.is_ok());
    let d = report.damage.as_ref().unwrap();
    assert_eq!(d.bad_trailer_copies, 1);
    assert!(d.needs_repair() && d.repairable);
    drop(ar);
    let r = repair_file(&dst).unwrap();
    assert_eq!(r.trailer_copies_restored, 1);
    assert_eq!(r.patched_shards, 0);
    assert_eq!(fs::read(&dst).unwrap(), original);

    // Les deux copies détruites : la copie embarquée dans la section de parité
    // est retrouvée par balayage, l'archive reste lisible et les deux copies
    // de queue sont restaurées.
    bytes[n - 2 * TRAILER_LEN..].fill(0xff);
    fs::write(&dst, &bytes).unwrap();
    let ar = Archive::open(&dst).unwrap();
    assert_eq!(ar.read_file("z/profond/petit.dat").unwrap(), b"abc");
    let d = ar.verify().damage.unwrap();
    assert_eq!(d.bad_trailer_copies, 2);
    assert!(d.repairable && d.bad_data_shards.is_empty());
    drop(ar);
    let r = repair_file(&dst).unwrap();
    assert_eq!((r.trailer_copies_restored, r.patched_shards), (2, 0));
    assert_eq!(fs::read(&dst).unwrap(), original);
}

#[test]
fn destroyed_trailers_and_parity_header_are_unrecoverable_without_writing() {
    let (_tmp, dst, original, protected_len) = archive(5);
    let mut bytes = original.clone();
    let n = bytes.len();
    bytes[n - 2 * TRAILER_LEN..].fill(0xff);
    bytes[protected_len as usize + 44] ^= 1; // CRC de l'en-tête de la section
    fs::write(&dst, &bytes).unwrap();
    assert!(matches!(Archive::open(&dst), Err(Error::NoValidTrailer)));
    assert!(matches!(scan_file(&dst), Err(Error::NoValidTrailer)));
    assert!(matches!(repair_file(&dst), Err(Error::NoValidTrailer)));
    assert_eq!(fs::read(&dst).unwrap(), bytes);
    assert!(!tmp_path(&dst).exists());
}

#[test]
fn destroyed_trailers_without_parity_are_unrecoverable() {
    let (_tmp, dst, original, _) = archive(0);
    let mut bytes = original;
    let n = bytes.len();
    bytes[n - 2 * TRAILER_LEN..].fill(0);
    fs::write(&dst, &bytes).unwrap();
    assert!(matches!(Archive::open(&dst), Err(Error::NoValidTrailer)));
}

#[test]
fn trailer_scan_ignores_stale_magic_inside_data() {
    // Un faux « THOLOSPR » dans la zone protégée (données utilisateur) ne doit
    // pas tromper le balayage : seul l'en-tête cohérent avec la fin du fichier compte.
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    fixture(&src);
    let mut lure = b"THOLOSPR".repeat(2000);
    lure.extend(pseudo_random(100_000, 9));
    fs::write(src.join("leurre.bin"), &lure).unwrap();
    let (dst, _) = pack_with(&src, "l.tholos", &opts(5));
    let original = fs::read(&dst).unwrap();
    let mut bytes = original.clone();
    let n = bytes.len();
    bytes[n - 2 * TRAILER_LEN..].fill(0xff);
    fs::write(&dst, &bytes).unwrap();
    assert_eq!(
        Archive::open(&dst)
            .unwrap()
            .read_file("leurre.bin")
            .unwrap(),
        lure
    );
    assert_repaired(&dst, &original, Some(0));
}

#[test]
fn corruption_beyond_budget_is_reported_unrepairable_without_writing() {
    let (_tmp, dst, original, protected_len) = archive(5);
    let mut bytes = original.clone();
    corrupt_contiguous(&mut bytes, protected_len, HEADER_LEN, 200);
    fs::write(&dst, &bytes).unwrap();
    let ar = Archive::open(&dst).unwrap();
    let report = ar.verify();
    assert!(!report.is_ok() && !report.repairable());
    let d = report.damage.unwrap();
    assert!(d.overflow.is_some());
    drop(ar);
    let err = repair_file(&dst).unwrap_err();
    assert!(matches!(err, Error::Unrepairable { .. }), "{err}");
    assert_eq!(fs::read(&dst).unwrap(), bytes, "aucune écriture");
    assert!(!tmp_path(&dst).exists());
}

#[test]
fn archive_without_parity_cannot_repair_data() {
    let (_tmp, dst, original, _) = archive(0);
    let mut bytes = original.clone();
    let c = Archive::open(&dst).unwrap().chunks()[0];
    bytes[c.offset as usize + 1] ^= 0x5a;
    fs::write(&dst, &bytes).unwrap();
    let report = Archive::open(&dst).unwrap().verify();
    assert!(!report.is_ok() && !report.repairable());
    assert!(matches!(repair_file(&dst), Err(Error::NoParity)));
    assert_eq!(fs::read(&dst).unwrap(), bytes);
}
