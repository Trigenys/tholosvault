//! Arbre de test et utilitaires partagés par les suites v0 et v1.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use vault_core::{pack_dir, PackOptions, PackSummary};

pub fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
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

/// Arbre de test : binaire 1 MiB, texte répétitif, doublon exact, fichier vide,
/// sous-répertoire. Retourne la liste (chemin relatif, contenu).
pub fn fixture(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let random = pseudo_random(1024 * 1024 + 123, 42);
    let text: Vec<u8> = "ligne de texte assez redondante pour zstd\n"
        .repeat(5000)
        .into_bytes();
    let files = vec![
        ("binaire.bin".to_string(), random.clone()),
        ("docs/notes.txt".to_string(), text.clone()),
        ("docs/notes-copie.txt".to_string(), text),
        ("vide".to_string(), Vec::new()),
        ("z/profond/petit.dat".to_string(), b"abc".to_vec()),
        ("binaire-bis.bin".to_string(), random),
    ];
    for (rel, data) in &files {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }
    files
}

pub fn pack_with(src: &Path, name: &str, opts: &PackOptions) -> (PathBuf, PackSummary) {
    let dst = src.parent().unwrap().join(name);
    let summary = pack_dir(src, &dst, opts).unwrap();
    (dst, summary)
}

pub fn pack(src: &Path, name: &str) -> (PathBuf, PackSummary) {
    pack_with(src, name, &PackOptions::default())
}
