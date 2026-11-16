//! Lecture par plage : sémantique `pread`, localité des morceaux décompressés,
//! métadonnées lisibles sans décompression (L-002).

mod common;

use std::fs;

use tempfile::tempdir;
use vault_core::reader::Archive;
use vault_core::{Error, PackOptions, ParityOptions};

const BIG: &str = "gros.bin";

fn setup() -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>) {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    let big = common::pseudo_random(3 * 1024 * 1024 + 777, 7);
    fs::write(src.join(BIG), &big).unwrap();
    let opts = PackOptions {
        parity: ParityOptions::disabled(),
        ..PackOptions::default()
    };
    let (archive, _) = common::pack_with(&src, "a.vault", &opts);
    (tmp, archive, big)
}

fn clamp(data: &[u8], offset: u64, len: usize) -> &[u8] {
    let start = (offset.min(data.len() as u64)) as usize;
    let end = start.saturating_add(len).min(data.len());
    &data[start..end]
}

#[test]
fn read_range_matches_read_file_on_offset_grid() {
    let (_tmp, path, big) = setup();
    let ar = Archive::open(&path).unwrap();
    let full = ar.read_file(BIG).unwrap();
    assert_eq!(full, big);
    let idx = ar.find_index(BIG).unwrap();
    let f = &ar.files()[idx];
    assert!(
        f.chunks.len() >= 4,
        "le fixture doit couvrir plusieurs morceaux"
    );

    let mut offsets = vec![0u64, 1, f.size - 1, f.size, f.size + 1, u64::MAX];
    let mut pos = 0u64;
    for &c in &f.chunks {
        pos += ar.chunks()[c as usize].len as u64;
        offsets.extend([pos.saturating_sub(1), pos, pos + 1]);
    }
    let lens = [0usize, 1, 4096, 300 * 1024, f.size as usize, usize::MAX];
    for &off in &offsets {
        for &len in &lens {
            let got = ar.read_range(BIG, off, len).unwrap();
            assert_eq!(got, clamp(&full, off, len), "offset {off} len {len}");
            let span = ar.chunk_span(idx, off, len).unwrap();
            if got.is_empty() {
                assert!(span.is_empty());
            } else {
                let covered: u64 = span
                    .clone()
                    .map(|p| ar.chunks()[f.chunks[p] as usize].len as u64)
                    .sum();
                assert!(covered >= got.len() as u64);
            }
        }
    }
}

#[test]
fn read_range_into_truncates_like_pread() {
    let (_tmp, path, big) = setup();
    let ar = Archive::open(&path).unwrap();
    let idx = ar.find_index(BIG).unwrap();
    let mut buf = vec![0u8; 10_000];
    let off = big.len() as u64 - 100;
    assert_eq!(ar.read_range_into(idx, off, &mut buf).unwrap(), 100);
    assert_eq!(&buf[..100], &big[big.len() - 100..]);
    assert_eq!(
        ar.read_range_into(idx, big.len() as u64, &mut buf).unwrap(),
        0
    );
    assert_eq!(ar.read_range_into(idx, 0, &mut []).unwrap(), 0);
    assert!(matches!(
        ar.read_range_into(ar.files().len(), 0, &mut buf),
        Err(Error::MalformedSection { .. })
    ));
}

#[test]
fn empty_file_duplicate_and_missing_path() {
    let (_tmp, path, _) = setup();
    let ar = Archive::open(&path).unwrap();
    assert!(ar.read_range("vide", 0, 10).unwrap().is_empty());
    let a = ar.read_range("docs/notes.txt", 1000, 5000).unwrap();
    let b = ar.read_range("docs/notes-copie.txt", 1000, 5000).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.len(), 5000);
    assert!(matches!(
        ar.read_range("absent", 0, 1),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn small_read_touches_at_most_two_chunks() {
    let (_tmp, path, _) = setup();
    let ar = Archive::open(&path).unwrap();
    let idx = ar.find_index(BIG).unwrap();
    let size = ar.files()[idx].size;
    for off in (0..size).step_by(4096) {
        let span = ar.chunk_span(idx, off, 4096).unwrap();
        assert!(span.len() <= 2, "offset {off} : {} morceaux", span.len());
    }
}

/// Corrompre un seul morceau n'affecte que les lectures qui le traversent.
#[test]
fn corruption_is_local_to_the_chunk() {
    let (_tmp, path, big) = setup();
    let (target, target_start, target_len, other_start) = {
        let ar = Archive::open(&path).unwrap();
        let f = ar.find(BIG).unwrap();
        let c1 = &ar.chunks()[f.chunks[1] as usize];
        let c0_len = ar.chunks()[f.chunks[0] as usize].len as u64;
        (
            c1.offset as usize,
            c0_len,
            c1.len as usize,
            c0_len + c1.len as u64,
        )
    };
    let mut bytes = fs::read(&path).unwrap();
    for b in &mut bytes[target + 4..target + 12] {
        *b ^= 0xff;
    }
    fs::write(&path, &bytes).unwrap();
    let ar = Archive::open(&path).unwrap();
    assert!(ar.read_range(BIG, target_start + 10, 16).is_err());
    assert!(target_len > 16);
    assert_eq!(
        ar.read_range(BIG, 0, 64).unwrap(),
        &big[..64],
        "le premier morceau reste lisible"
    );
    assert_eq!(
        ar.read_range(BIG, other_start, 64).unwrap(),
        &big[other_start as usize..other_start as usize + 64]
    );
}

/// L-002 : toutes les frames zstd à zéro, les métadonnées restent exploitables.
#[test]
fn metadata_readable_without_any_decompression() {
    let (_tmp, path, _) = setup();
    let ranges: Vec<(usize, usize)> = {
        let ar = Archive::open(&path).unwrap();
        ar.chunks()
            .iter()
            .map(|c| (c.offset as usize, c.compressed_len as usize))
            .collect()
    };
    let mut bytes = fs::read(&path).unwrap();
    for (off, len) in ranges {
        bytes[off..off + len].fill(0);
    }
    fs::write(&path, &bytes).unwrap();
    let ar = Archive::open(&path).unwrap();
    let idx = ar.find_index(BIG).unwrap();
    assert!(ar.find(BIG).is_some());
    assert!(!ar.chunk_span(idx, 0, 4096).unwrap().is_empty());
    assert!(ar.read_range(BIG, 0, 1).is_err());
}
