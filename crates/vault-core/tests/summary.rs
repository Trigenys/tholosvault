//! Bilan d'archive (`tholos info`) : chiffres cohérents avec le résumé
//! d'écriture et la taille réelle du fichier, sans décompression.

mod common;

use std::fs;

use tempfile::tempdir;
use vault_core::reader::Archive;
use vault_core::{codec, format, PackOptions, ParityOptions};

#[test]
fn summary_matches_pack_summary_and_file_size() {
    for parity in [true, false] {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir_all(&src).unwrap();
        let files = common::fixture(&src);
        let opts = if parity {
            PackOptions::default()
        } else {
            PackOptions {
                parity: ParityOptions::disabled(),
                ..PackOptions::default()
            }
        };
        let (path, packed) = common::pack_with(&src, "a.vault", &opts);
        let ar = Archive::open(&path).unwrap();
        let s = ar.summary();

        assert_eq!(s.archive_bytes, fs::metadata(&path).unwrap().len());
        assert_eq!(s.archive_bytes, packed.archive_bytes);
        assert_eq!(s.files, files.len() as u64);
        assert_eq!(s.files, packed.files);
        let logical: u64 = files.iter().map(|(_, d)| d.len() as u64).sum();
        assert_eq!(s.logical_bytes, logical);
        assert_eq!(s.logical_bytes, packed.input_bytes);
        assert_eq!(s.unique_chunks, packed.unique_chunks);
        assert_eq!(s.chunk_refs, packed.total_chunk_refs);
        assert_eq!(s.parity_bytes, packed.parity_bytes);
        assert_eq!(s.compressed_bytes, packed.compressed_bytes);
        assert_eq!(s.parity_bytes > 0, parity);
        assert_eq!(s.archive_root, packed.archive_root);
        assert_eq!(s.format_version, format::FORMAT_VERSION);
        assert_eq!(s.codec_version, codec::zstd_version());
        // Le fixture contient deux doublons exacts : la déduplication économise
        // au moins la copie du binaire de 1 MiB.
        assert!(s.dedup_saved_bytes() >= 1024 * 1024);
        assert!(s.unique_chunks < s.chunk_refs);
        assert!(s.compressed_bytes < s.unique_bytes);
        assert!(s.ratio().unwrap() > 1.0);
        let pct = s.parity_percent();
        if parity {
            assert!(pct > 0.0 && pct < 20.0, "{pct}");
        } else {
            assert_eq!(pct, 0.0);
        }
        // En-tête + frames + tables + parité + trailers = archive.
        assert_eq!(
            format::HEADER_LEN as u64
                + s.compressed_bytes
                + s.metadata_bytes
                + s.parity_bytes
                + 2 * format::TRAILER_LEN as u64,
            s.archive_bytes
        );
    }
}

#[test]
fn codec_version_is_human_readable() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a"), b"a").unwrap();
    let (path, _) = common::pack(&src, "a.vault");
    let mut s = Archive::open(&path).unwrap().summary();
    s.codec_version = 10507;
    assert_eq!(s.codec_version_string(), "1.5.7");
}

#[test]
fn empty_archive_has_no_ratio() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    let (path, _) = common::pack(&src, "a.vault");
    let s = Archive::open(&path).unwrap().summary();
    assert_eq!(s.files, 0);
    assert_eq!(s.ratio(), None);
    assert_eq!(s.dedup_saved_bytes(), 0);
}
