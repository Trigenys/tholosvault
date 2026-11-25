//! Sélection de fichiers et écriture en flux (`tholos extract`) : la mémoire
//! reste bornée par un morceau, un morceau corrompu arrête l'écriture.

mod common;

use std::fs;
use std::io::{self, Write};

use tempfile::tempdir;
use vault_core::reader::Archive;
use vault_core::{Error, PackOptions, ParityOptions};

fn setup() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    Vec<(String, Vec<u8>)>,
) {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    let files = common::fixture(&src);
    let opts = PackOptions {
        parity: ParityOptions::disabled(),
        ..PackOptions::default()
    };
    let (archive, _) = common::pack_with(&src, "a.vault", &opts);
    (tmp, archive, files)
}

fn paths(ar: &Archive, idx: &[usize]) -> Vec<String> {
    idx.iter().map(|&i| ar.files()[i].path.clone()).collect()
}

#[test]
fn select_without_selector_returns_every_file() {
    let (_tmp, path, files) = setup();
    let ar = Archive::open(&path).unwrap();
    let all = ar.select(&[]).unwrap();
    assert_eq!(all, (0..files.len()).collect::<Vec<_>>());
}

#[test]
fn select_matches_exact_paths_and_directories() {
    let (_tmp, path, _) = setup();
    let ar = Archive::open(&path).unwrap();
    assert_eq!(
        paths(&ar, &ar.select(&["docs"]).unwrap()),
        ["docs/notes-copie.txt", "docs/notes.txt"]
    );
    assert_eq!(
        paths(&ar, &ar.select(&["docs/", "vide"]).unwrap()),
        ["docs/notes-copie.txt", "docs/notes.txt", "vide"]
    );
    assert_eq!(
        paths(&ar, &ar.select(&["z"]).unwrap()),
        ["z/profond/petit.dat"]
    );
    // Recouvrement : chaque fichier une seule fois, ordre de l'archive.
    assert_eq!(
        paths(&ar, &ar.select(&["docs/notes.txt", "docs"]).unwrap()),
        ["docs/notes-copie.txt", "docs/notes.txt"]
    );
    // Un préfixe de nom n'est pas un répertoire.
    assert_eq!(
        paths(&ar, &ar.select(&["binaire.bin"]).unwrap()),
        ["binaire.bin"]
    );
}

#[test]
fn select_rejects_unknown_and_escaping_selectors() {
    let (_tmp, path, _) = setup();
    let ar = Archive::open(&path).unwrap();
    assert!(matches!(ar.select(&["absent"]), Err(Error::NotFound(p)) if p == "absent"));
    assert!(matches!(ar.select(&["doc"]), Err(Error::NotFound(_))));
    assert!(matches!(ar.select(&["../x"]), Err(Error::InvalidPath(_))));
    assert!(matches!(ar.select(&["/docs"]), Err(Error::InvalidPath(_))));
}

#[test]
fn write_file_to_streams_exact_content() {
    let (_tmp, path, files) = setup();
    let ar = Archive::open(&path).unwrap();
    for (rel, data) in &files {
        let idx = ar.find_index(rel).unwrap();
        let mut out = Vec::new();
        let n = ar.write_file_to(idx, &mut out).unwrap();
        assert_eq!(n, data.len() as u64, "{rel}");
        assert_eq!(&out, data, "{rel}");
    }
}

/// Puits qui compte la plus grande écriture reçue : la mémoire de l'écriture
/// en flux est bornée par un morceau, pas par le fichier.
struct MaxWrite {
    total: u64,
    max: usize,
}

impl Write for MaxWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.total += buf.len() as u64;
        self.max = self.max.max(buf.len());
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn write_file_to_never_buffers_more_than_one_chunk() {
    let (_tmp, path, _) = setup();
    let ar = Archive::open(&path).unwrap();
    let idx = ar.find_index("binaire.bin").unwrap();
    let f = &ar.files()[idx];
    assert!(
        f.chunks.len() >= 2,
        "le fixture doit couvrir plusieurs morceaux"
    );
    let largest = f
        .chunks
        .iter()
        .map(|&c| ar.chunks()[c as usize].len as usize)
        .max()
        .unwrap();
    let mut sink = MaxWrite { total: 0, max: 0 };
    ar.write_file_to(idx, &mut sink).unwrap();
    assert_eq!(sink.total, f.size);
    assert!(sink.max <= largest && (sink.max as u64) < f.size);
}

#[test]
fn write_file_to_stops_at_corrupted_chunk_with_intact_prefix() {
    let (_tmp, path, files) = setup();
    let (offset, first_len, expected) = {
        let ar = Archive::open(&path).unwrap();
        let idx = ar.find_index("binaire.bin").unwrap();
        let f = &ar.files()[idx];
        let second = ar.chunks()[f.chunks[1] as usize];
        let first_len = ar.chunks()[f.chunks[0] as usize].len as usize;
        (second.offset + 8, first_len, files[0].1.clone())
    };
    let mut bytes = fs::read(&path).unwrap();
    bytes[offset as usize] ^= 0xff;
    fs::write(&path, &bytes).unwrap();

    let ar = Archive::open(&path).unwrap();
    let idx = ar.find_index("binaire.bin").unwrap();
    let mut out = Vec::new();
    let err = ar.write_file_to(idx, &mut out).unwrap_err();
    assert!(
        matches!(err, Error::ChunkMismatch { .. } | Error::Decompress { .. }),
        "{err}"
    );
    assert_eq!(out.len(), first_len);
    assert_eq!(out, expected[..first_len]);
}

#[test]
fn write_file_to_reports_sink_failure() {
    struct Full;
    impl Write for Full {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::StorageFull, "plein"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let (_tmp, path, _) = setup();
    let ar = Archive::open(&path).unwrap();
    let idx = ar.find_index("docs/notes.txt").unwrap();
    assert!(matches!(
        ar.write_file_to(idx, &mut Full),
        Err(Error::Output(_))
    ));
}
