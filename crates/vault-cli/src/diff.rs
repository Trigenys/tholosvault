//! `tholos diff` : aperçu du delta entre deux archives (`vault_core::diff`).

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;
use vault_core::{merkle, Archive, ArchiveDiff, FileChange, FileDiff};

use crate::info::bytes;

pub fn run(old: &Path, new: &Path, as_json: bool, all: bool) -> Result<()> {
    let a = Archive::open(old).with_context(|| format!("ouverture de {}", old.display()))?;
    let b = Archive::open(new).with_context(|| format!("ouverture de {}", new.display()))?;
    let d = vault_core::diff(&a, &b);
    if as_json {
        println!("{}", serde_json::to_string_pretty(&to_json(&d, all))?);
    } else {
        if !d.same_chunking {
            eprintln!(
                "attention : paramètres de découpe différents ; les morceaux ne coïncident pas \
                 et un fichier identique peut apparaître modifié (réarchiver l'ancienne version \
                 avec la même découpe pour un delta utile)"
            );
        }
        print!("{}", to_text(&d, all));
    }
    Ok(())
}

/// Schéma stable `tholos-diff/1` : clés ajoutées seulement, jamais renommées.
pub fn to_json(d: &ArchiveDiff, all: bool) -> serde_json::Value {
    let files: Vec<_> = d
        .files
        .iter()
        .filter(|f| all || f.change != FileChange::Unchanged)
        .map(|f| {
            json!({
                "path": f.path,
                "change": f.change.as_str(),
                "old_size": f.old_size,
                "new_size": f.new_size,
                "new_chunk_refs": f.new_chunk_refs,
                "new_chunk_bytes": f.new_chunk_bytes,
            })
        })
        .collect();
    json!({
        "schema": "tholos-diff/1",
        "old_root": merkle::to_hex(&d.old_root),
        "new_root": merkle::to_hex(&d.new_root),
        "identical": d.is_identical(),
        "same_chunking": d.same_chunking,
        "counts": {
            "added": d.count(FileChange::Added),
            "removed": d.count(FileChange::Removed),
            "modified": d.count(FileChange::Modified),
            "unchanged": d.count(FileChange::Unchanged),
        },
        "transfer": {
            "chunks": d.transfer_chunks,
            "logical_bytes": d.transfer_logical_bytes,
            "compressed_bytes": d.transfer_compressed_bytes,
            "metadata_bytes": d.new_metadata_bytes,
            "percent": (d.transfer_percent() * 100.0).round() / 100.0,
        },
        "reused": {
            "chunks": d.reused_chunks,
            "compressed_bytes": d.reused_compressed_bytes,
        },
        "new_compressed_bytes": d.new_compressed_bytes,
        "files": files,
    })
}

fn line(f: &FileDiff) -> String {
    let size = |s: Option<u64>| s.map_or_else(|| "—".to_string(), |n| n.to_string());
    match f.change {
        FileChange::Added => format!("+ {}  ({} o)", f.path, size(f.new_size)),
        FileChange::Removed => format!("- {}  ({} o)", f.path, size(f.old_size)),
        FileChange::Unchanged => format!("  {}", f.path),
        FileChange::Modified => format!(
            "~ {}  ({} → {} o, {} morceau(x) nouveau(x), {} o)",
            f.path,
            size(f.old_size),
            size(f.new_size),
            f.new_chunk_refs,
            f.new_chunk_bytes
        ),
    }
}

pub fn to_text(d: &ArchiveDiff, all: bool) -> String {
    let mut out = String::new();
    if d.is_identical() {
        out.push_str("archives identiques (même racine) : rien à transférer\n");
        out.push_str(&format!("racine {}\n", merkle::to_hex(&d.new_root)));
        return out;
    }
    for f in d
        .files
        .iter()
        .filter(|f| all || f.change != FileChange::Unchanged)
    {
        out.push_str(&line(f));
        out.push('\n');
    }
    out.push_str(&format!(
        "{} ajouté(s), {} supprimé(s), {} modifié(s), {} inchangé(s)\n",
        d.count(FileChange::Added),
        d.count(FileChange::Removed),
        d.count(FileChange::Modified),
        d.count(FileChange::Unchanged),
    ));
    out.push_str(&format!(
        "à transférer   {} morceau(x), {} compressés ({:.1} % des frames), {} décompressés\n",
        d.transfer_chunks,
        bytes(d.transfer_compressed_bytes),
        d.transfer_percent(),
        bytes(d.transfer_logical_bytes),
    ));
    out.push_str(&format!(
        "réutilisés     {} morceau(x), {}\n",
        d.reused_chunks,
        bytes(d.reused_compressed_bytes),
    ));
    out.push_str(&format!("métadonnées    {}\n", bytes(d.new_metadata_bytes)));
    out.push_str(&format!(
        "racines        {} → {}\n",
        merkle::to_hex(&d.old_root),
        merkle::to_hex(&d.new_root)
    ));
    out
}
