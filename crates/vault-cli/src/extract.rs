//! `tholos extract` : extraction complète ou sélective, en flux.
//!
//! Chaque fichier est écrit morceau par morceau (mémoire bornée par un
//! morceau, voir `Archive::write_file_to`) dans un fichier temporaire voisin,
//! renommé seulement une fois complet : un morceau corrompu ne laisse jamais
//! de fichier tronqué à la place de la cible, et relancer la commande après
//! `tholos repair` reprend proprement.

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use vault_core::Archive;

/// Suffixe des fichiers en cours d'écriture.
const PART_SUFFIX: &str = ".tholos-part";

pub fn run(archive: &Path, out_dir: &Path, selectors: &[String]) -> Result<()> {
    let ar =
        Archive::open(archive).with_context(|| format!("ouverture de {}", archive.display()))?;
    let sel: Vec<&str> = selectors.iter().map(String::as_str).collect();
    let chosen = ar.select(&sel).context(
        "sélection refusée (chemins de `tholos list`, ou répertoire sans barre initiale)",
    )?;

    let mut bytes = 0u64;
    let mut failed = Vec::new();
    for &idx in &chosen {
        let rel = &ar.files()[idx].path;
        match extract_one(&ar, idx, out_dir, rel) {
            Ok(n) => bytes += n,
            Err(e) => {
                eprintln!("échec : {rel} : {e:#}");
                failed.push(rel.clone());
            }
        }
    }
    let done = chosen.len() - failed.len();
    println!(
        "{done} fichier(s) extrait(s) ({bytes} octets) dans {}",
        out_dir.display()
    );
    if !failed.is_empty() {
        bail!(
            "{} fichier(s) non extrait(s) ; lancer `tholos verify {}` puis `tholos repair` si la parité le permet, et relancer l'extraction",
            failed.len(),
            archive.display()
        );
    }
    Ok(())
}

fn extract_one(ar: &Archive, idx: usize, out_dir: &Path, rel: &str) -> Result<u64> {
    let dest = safe_join(out_dir, rel)?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).with_context(|| parent.display().to_string())?;
    }
    let mut part = dest.clone().into_os_string();
    part.push(PART_SUFFIX);
    let part = PathBuf::from(part);
    let written = (|| -> Result<u64> {
        let file = fs::File::create(&part).with_context(|| part.display().to_string())?;
        let mut w = BufWriter::new(file);
        let n = ar.write_file_to(idx, &mut w)?;
        w.flush().with_context(|| part.display().to_string())?;
        Ok(n)
    })();
    match written {
        Ok(n) => {
            fs::rename(&part, &dest).with_context(|| dest.display().to_string())?;
            Ok(n)
        }
        Err(e) => {
            let _ = fs::remove_file(&part);
            Err(e)
        }
    }
}

/// Refuse les chemins qui sortiraient du répertoire cible (règle de `vault_core::path`).
fn safe_join(root: &Path, rel: &str) -> Result<PathBuf> {
    let parts = vault_core::path::components(rel)
        .with_context(|| format!("chemin d'archive refusé : {rel}"))?;
    let mut out = root.to_path_buf();
    out.extend(parts);
    Ok(out)
}
