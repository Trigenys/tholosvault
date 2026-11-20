//! Tests de non-régression du format v0 : aller-retour, déterminisme (L-001),
//! métadonnées lisibles malgré des données corrompues (L-002), détection de
//! corruption (L-004, détection seulement), trailer répliqué. La suite tourne
//! avec et sans section de parité : les contrats v0 restent verts en v1.

mod common;

use std::fs;

use common::{fixture, pack, pack_with};
use vault_core::format::{HEADER_LEN, TRAILER_LEN};
use vault_core::{Archive, Error, PackOptions, ParityOptions};

/// Options v0 : aucune section de parité.
fn v0() -> PackOptions {
    PackOptions {
        parity: ParityOptions::disabled(),
        ..PackOptions::default()
    }
}

#[test]
fn roundtrip_lists_reads_and_verifies() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    let mut files = fixture(&src);
    let (dst, summary) = pack_with(&src, "a.tholos", &v0());

    let ar = Archive::open(&dst).unwrap();
    files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let listed: Vec<&str> = ar.files().iter().map(|f| f.path.as_str()).collect();
    let expected: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(listed, expected, "ordre bytewise des chemins");

    for (rel, data) in &files {
        assert_eq!(&ar.read_file(rel).unwrap(), data, "{rel}");
        assert_eq!(ar.find(rel).unwrap().size, data.len() as u64);
    }
    assert!(matches!(ar.read_file("absent"), Err(Error::NotFound(_))));

    let report = ar.verify();
    assert!(report.is_ok(), "{report:?}");
    assert_eq!(report.chunks_checked, summary.unique_chunks);
    assert_eq!(summary.files, 6);
    assert_eq!(*ar.archive_root(), summary.archive_root);
    assert_eq!(fs::metadata(&dst).unwrap().len(), summary.archive_bytes);
}

#[test]
fn deterministic_bytes_l001() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    fixture(&src);
    let (a, sa) = pack(&src, "a.tholos");
    let (b, sb) = pack(&src, "b.tholos");
    assert_eq!(sa, sb);
    assert_eq!(
        fs::read(a).unwrap(),
        fs::read(b).unwrap(),
        "même entrée, mêmes octets"
    );
}

#[test]
fn duplicates_are_deduplicated() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    fixture(&src);
    let (dst, summary) = pack(&src, "a.tholos");
    let ar = Archive::open(&dst).unwrap();

    let bin = ar.find("binaire.bin").unwrap();
    let bis = ar.find("binaire-bis.bin").unwrap();
    assert_eq!(
        bin.chunks, bis.chunks,
        "doublon : mêmes indices de morceaux"
    );
    assert_eq!(bin.root, bis.root);
    assert!(summary.total_chunk_refs > summary.unique_chunks);

    let notes = ar.find("docs/notes.txt").unwrap();
    let copie = ar.find("docs/notes-copie.txt").unwrap();
    assert_eq!(notes.chunks, copie.chunks);

    // Chaque morceau est retrouvable par son hachage.
    for (i, c) in ar.chunks().iter().enumerate() {
        assert_eq!(ar.chunk_index_by_hash(&c.hash), Some(i as u32));
    }
}

#[test]
fn corrupted_data_is_detected_but_metadata_still_readable() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    fixture(&src);
    let (dst, _) = pack(&src, "a.tholos");

    // Corrompt le premier morceau de binaire.bin dans la zone de données.
    let ar = Archive::open(&dst).unwrap();
    let first = ar.find("binaire.bin").unwrap().chunks[0];
    let c = ar.chunks()[first as usize];
    drop(ar);
    let mut bytes = fs::read(&dst).unwrap();
    let pos = c.offset as usize + c.compressed_len as usize / 2;
    assert!(pos >= HEADER_LEN);
    bytes[pos] ^= 0x5a;
    fs::write(&dst, &bytes).unwrap();

    let ar = Archive::open(&dst).unwrap();
    assert_eq!(
        ar.files().len(),
        6,
        "la liste se lit sans décompresser (L-002)"
    );
    assert!(
        ar.read_file("docs/notes.txt").is_ok(),
        "fichier intact lisible"
    );
    let err = ar.read_file("binaire.bin").unwrap_err();
    assert!(
        matches!(err, Error::ChunkMismatch { .. } | Error::Decompress { .. }),
        "{err}"
    );
    let report = ar.verify();
    assert_eq!(report.bad_chunks, vec![first]);
    assert!(
        report.bad_files.is_empty(),
        "les racines Merkle stockées restent cohérentes"
    );
    assert!(report.archive_root_ok);
    assert!(!report.is_ok());
}

#[test]
fn trailer_replica_survives_last_copy_corruption() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    fixture(&src);
    // Sans parité : la réplique de queue est la seule redondance du trailer.
    let (dst, _) = pack_with(&src, "a.tholos", &v0());

    let mut bytes = fs::read(&dst).unwrap();
    let n = bytes.len();
    for b in &mut bytes[n - TRAILER_LEN..] {
        *b = 0xff;
    }
    fs::write(&dst, &bytes).unwrap();
    let ar = Archive::open(&dst).unwrap();
    assert!(ar.verify().is_ok());
    drop(ar); // Windows : le mmap verrouille le fichier.

    // Les deux copies détruites, sans parité : erreur explicite.
    for b in &mut bytes[n - 2 * TRAILER_LEN..] {
        *b = 0xff;
    }
    fs::write(&dst, &bytes).unwrap();
    assert!(matches!(Archive::open(&dst), Err(Error::NoValidTrailer)));
}

#[test]
fn empty_directory_packs_and_opens() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    let (dst, summary) = pack(&src, "vide.tholos");
    assert_eq!(summary.files, 0);
    let ar = Archive::open(&dst).unwrap();
    assert!(ar.files().is_empty());
    assert!(ar.verify().is_ok());
}

#[test]
fn garbage_is_rejected_with_actionable_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("x.tholos");
    fs::write(&p, b"court").unwrap();
    assert!(matches!(Archive::open(&p), Err(Error::TooShort { .. })));
    fs::write(&p, vec![0u8; 1024]).unwrap();
    assert!(matches!(Archive::open(&p), Err(Error::BadHeader(_))));
}
