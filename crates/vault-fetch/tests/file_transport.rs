//! Transport fichier de bout en bout : archive réelle, téléchargeur réel.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::{tempdir, TempDir};
use vault_core::{pack_dir, PackOptions};
use vault_fetch::{FetchOptions, Fetcher, FileTransport, Outcome, TransportError};

fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
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

fn archive() -> (TempDir, PathBuf) {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(src.join("d")).unwrap();
    fs::write(src.join("gros.bin"), pseudo_random(3 * 1024 * 1024 + 11, 5)).unwrap();
    fs::write(src.join("d/texte.txt"), "abc\n".repeat(50_000)).unwrap();
    let path = tmp.path().join("source.vault");
    pack_dir(&src, &path, &PackOptions::default()).unwrap();
    (tmp, path)
}

fn opts(dest: &Path, source: &Path) -> FetchOptions {
    let mut o = FetchOptions::new(dest, source.display().to_string());
    o.connections = 3;
    o.segment_bytes = 512 * 1024;
    o
}

#[tokio::test(flavor = "multi_thread")]
async fn copies_and_verifies_a_local_archive() {
    let (tmp, source) = archive();
    let dest = tmp.path().join("copie.vault");
    let t = FileTransport::new(&source, &dest).unwrap();
    let s = Fetcher::new(t, opts(&dest, &source)).run().await.unwrap();
    assert_eq!(s.outcome, Outcome::Complete);
    assert_eq!(fs::read(&dest).unwrap(), fs::read(&source).unwrap());
    assert_eq!(s.chunks_rejected, 0);

    // Idempotence : une seconde exécution ne relit que le trailer (2 × 128 o),
    // aucun octet de contenu.
    let t = FileTransport::new(&source, &dest).unwrap();
    let s = Fetcher::new(t, opts(&dest, &source)).run().await.unwrap();
    assert_eq!(s.outcome, Outcome::AlreadyComplete);
    assert!(s.bytes_received <= 256, "{}", s.bytes_received);
}

#[tokio::test(flavor = "multi_thread")]
async fn corrupted_source_chunk_is_rejected_not_copied() {
    let (tmp, source) = archive();
    let mut bytes = fs::read(&source).unwrap();
    // Octet au milieu de la première frame (après l'en-tête de 64 octets).
    bytes[64 + 100] ^= 0xff;
    fs::write(&source, &bytes).unwrap();
    let dest = tmp.path().join("copie.vault");
    let t = FileTransport::new(&source, &dest).unwrap();
    let err = Fetcher::new(t, opts(&dest, &source)).run().await;
    assert!(
        err.is_err(),
        "une source abîmée ne doit pas donner une copie « complète »"
    );
}

#[test]
fn refuses_destination_equal_to_source() {
    let (tmp, source) = archive();
    let err = FileTransport::new(&source, &source).unwrap_err();
    assert!(matches!(err, TransportError::Protocol(m) if m.contains("--out")));
    // Même fichier par un autre chemin.
    let other = tmp.path().join(".").join("source.vault");
    assert!(FileTransport::new(&source, &other).is_err());
    assert_eq!(
        fs::read(&source).unwrap().len() as u64,
        fs::metadata(&source).unwrap().len()
    );
}

#[test]
fn missing_source_is_actionable() {
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("absente.vault");
    let err = FileTransport::new(&missing, &tmp.path().join("x")).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("introuvable") && msg.contains("absente.vault"),
        "{msg}"
    );
}
