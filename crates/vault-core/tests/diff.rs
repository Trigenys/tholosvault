//! Delta entre deux archives (H6) : classement des fichiers et morceaux à
//! transférer, lus dans les tables seules.

mod common;

use std::fs;

use tempfile::tempdir;
use vault_core::reader::Archive;
use vault_core::{diff, ChunkParams, FileChange, PackOptions};

#[test]
fn identical_sources_need_no_transfer() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    let (a, _) = common::pack(&src, "a.vault");
    // Niveau zstd différent : mêmes morceaux (BLAKE3 du contenu décompressé).
    let (b, _) = common::pack_with(
        &src,
        "b.vault",
        &PackOptions {
            level: 1,
            ..PackOptions::default()
        },
    );
    let (a, b) = (Archive::open(&a).unwrap(), Archive::open(&b).unwrap());
    let d = diff(&a, &b);
    assert!(d.is_identical());
    assert!(d.same_chunking);
    assert_eq!(d.transfer_chunks, 0);
    assert_eq!(d.transfer_compressed_bytes, 0);
    assert_eq!(d.reused_chunks, b.chunks().len() as u64);
    assert_eq!(d.changed().count(), 0);
    assert_eq!(d.count(FileChange::Unchanged), b.files().len());
}

#[test]
fn classifies_files_and_counts_only_fresh_chunks() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    let (old, _) = common::pack(&src, "old.vault");

    // Ajout en tête d'un gros fichier : seuls les premiers morceaux changent (CDC).
    let mut bin = fs::read(src.join("binaire.bin")).unwrap();
    bin.splice(0..0, common::pseudo_random(10_000, 7));
    fs::write(src.join("binaire.bin"), &bin).unwrap();
    fs::remove_file(src.join("z/profond/petit.dat")).unwrap();
    let added = common::pseudo_random(300_000, 9);
    fs::write(src.join("nouveau.bin"), &added).unwrap();
    let (new, _) = common::pack(&src, "new.vault");

    let (old, new) = (Archive::open(&old).unwrap(), Archive::open(&new).unwrap());
    let d = diff(&old, &new);
    assert!(!d.is_identical());

    let change = |p: &str| d.files.iter().find(|f| f.path == p).unwrap().change;
    assert_eq!(change("binaire.bin"), FileChange::Modified);
    assert_eq!(change("binaire-bis.bin"), FileChange::Unchanged);
    assert_eq!(change("z/profond/petit.dat"), FileChange::Removed);
    assert_eq!(change("nouveau.bin"), FileChange::Added);
    assert_eq!(d.count(FileChange::Modified), 1);
    assert_eq!(d.count(FileChange::Added), 1);
    assert_eq!(d.count(FileChange::Removed), 1);
    assert_eq!(d.files.len(), new.files().len() + 1);
    assert!(d.files.windows(2).all(|w| w[0].path < w[1].path));

    // Le fichier modifié ne réclame qu'une petite partie de ses morceaux.
    let m = d.files.iter().find(|f| f.path == "binaire.bin").unwrap();
    let total = new.find("binaire.bin").unwrap().chunks.len() as u64;
    assert!(m.new_chunk_refs >= 1 && m.new_chunk_refs < total, "{m:?}");
    let a = d.files.iter().find(|f| f.path == "nouveau.bin").unwrap();
    assert_eq!(a.new_chunk_bytes, added.len() as u64);

    // Bilan global cohérent avec les tables de la nouvelle archive.
    assert_eq!(
        d.transfer_chunks + d.reused_chunks,
        new.chunks().len() as u64
    );
    let frames: u64 = new.chunks().iter().map(|c| c.compressed_len as u64).sum();
    assert_eq!(d.new_compressed_bytes, frames);
    assert_eq!(
        d.transfer_compressed_bytes + d.reused_compressed_bytes,
        frames
    );
    assert!(d.transfer_percent() > 0.0 && d.transfer_percent() < 60.0);
    // H6 : le delta tient dans les morceaux nouveaux (ajout + tête modifiée).
    let fresh_logical: u64 = d.files.iter().map(|f| f.new_chunk_bytes).sum();
    assert!(d.transfer_logical_bytes <= fresh_logical);
    assert!(d.transfer_logical_bytes >= added.len() as u64);
}

#[test]
fn different_chunking_is_reported() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    let (a, _) = common::pack(&src, "a.vault");
    let (b, _) = common::pack_with(
        &src,
        "b.vault",
        &PackOptions {
            chunk: ChunkParams {
                min: 16 * 1024,
                avg: 32 * 1024,
                max: 64 * 1024,
            },
            ..PackOptions::default()
        },
    );
    let d = diff(&Archive::open(&a).unwrap(), &Archive::open(&b).unwrap());
    assert!(!d.same_chunking);
    // La racine d'un fichier dépend de sa découpe : à contenu égal, un fichier
    // de plusieurs morceaux apparaît modifié ; d'où l'avertissement exposé.
    assert!(d.count(FileChange::Modified) > 0);
}
