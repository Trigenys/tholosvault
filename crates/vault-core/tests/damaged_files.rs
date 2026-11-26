//! Morceaux corrompus → fichiers atteints : un morceau dédupliqué atteint
//! chaque fichier qui le référence.

mod common;

use std::fs;

use tempfile::tempdir;
use vault_core::reader::Archive;

#[test]
fn corrupt_shared_chunk_hits_every_file_using_it() {
    let tmp = tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    common::fixture(&src);
    let (path, _) = common::pack(&src, "a.vault");

    let off = {
        let ar = Archive::open(&path).unwrap();
        assert!(ar.files_using_chunks(&[]).is_empty());
        let c = ar.find("binaire.bin").unwrap().chunks[0];
        ar.chunks()[c as usize].offset + 20
    };
    let mut b = fs::read(&path).unwrap();
    b[off as usize] ^= 0xff;
    fs::write(&path, b).unwrap();

    let ar = Archive::open(&path).unwrap();
    let r = ar.verify();
    assert_eq!(r.bad_chunks.len(), 1);
    // `binaire-bis.bin` est un doublon exact : même morceau, même dégât.
    assert_eq!(
        ar.files_using_chunks(&r.bad_chunks),
        ["binaire-bis.bin", "binaire.bin"]
    );
    // Les racines restent cohérentes avec les tables : seul le contenu est atteint.
    assert!(r.bad_files.is_empty());
    // Indice hors table ignoré.
    assert!(ar.files_using_chunks(&[u32::MAX]).is_empty());
}
