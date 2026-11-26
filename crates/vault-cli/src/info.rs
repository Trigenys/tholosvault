//! `tholos info` : bilan d'une archive sans décompression (`Archive::summary`).

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;
use vault_core::{merkle, Archive, ArchiveSummary};

pub fn run(archive: &Path, as_json: bool) -> Result<()> {
    let ar =
        Archive::open(archive).with_context(|| format!("ouverture de {}", archive.display()))?;
    let s = ar.summary();
    if as_json {
        println!("{}", serde_json::to_string_pretty(&to_json(&s))?);
    } else {
        print!("{}", to_text(&s));
    }
    Ok(())
}

/// Schéma stable `tholos-info/1` : clés ajoutées seulement, jamais renommées.
pub fn to_json(s: &ArchiveSummary) -> serde_json::Value {
    json!({
        "schema": "tholos-info/1",
        "format_version": s.format_version,
        "codec": {
            "name": "zstd",
            "level": s.codec_level,
            "version": s.codec_version_string(),
        },
        "chunking": { "min": s.chunk_min, "avg": s.chunk_avg, "max": s.chunk_max },
        "files": s.files,
        "logical_bytes": s.logical_bytes,
        "chunk_refs": s.chunk_refs,
        "unique_chunks": s.unique_chunks,
        "unique_bytes": s.unique_bytes,
        "dedup_saved_bytes": s.dedup_saved_bytes(),
        "compressed_bytes": s.compressed_bytes,
        "metadata_bytes": s.metadata_bytes,
        "parity_bytes": s.parity_bytes,
        "parity_percent": round2(s.parity_percent()),
        "archive_bytes": s.archive_bytes,
        "ratio": s.ratio().map(round2),
        "archive_root": merkle::to_hex(&s.archive_root),
    })
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

pub fn to_text(s: &ArchiveSummary) -> String {
    let ratio = s
        .ratio()
        .map_or_else(|| "—".to_string(), |r| format!("{r:.2}×"));
    let parity = if s.parity_bytes == 0 {
        "aucune".to_string()
    } else {
        format!(
            "{} ({:.1} % de l'archive)",
            bytes(s.parity_bytes),
            s.parity_percent()
        )
    };
    format!(
        "format          v{}\n\
         codec           zstd {} (niveau {})\n\
         découpage       {} / {} / {} (min / moyen / max)\n\
         fichiers        {}\n\
         taille logique  {}\n\
         morceaux        {} uniques pour {} références\n\
         déduplication   {} économisés\n\
         frames zstd     {}\n\
         métadonnées     {}\n\
         parité          {}\n\
         archive         {} (ratio {})\n\
         racine          {}\n",
        s.format_version,
        s.codec_version_string(),
        s.codec_level,
        bytes(s.chunk_min as u64),
        bytes(s.chunk_avg as u64),
        bytes(s.chunk_max as u64),
        s.files,
        bytes(s.logical_bytes),
        s.unique_chunks,
        s.chunk_refs,
        bytes(s.dedup_saved_bytes()),
        bytes(s.compressed_bytes),
        bytes(s.metadata_bytes),
        parity,
        bytes(s.archive_bytes),
        ratio,
        merkle::to_hex(&s.archive_root),
    )
}

/// Taille lisible en unités binaires, valeur exacte entre parenthèses au-delà de 1 KiB.
pub(crate) fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["o", "Kio", "Mio", "Gio", "Tio"];
    if n < 1024 {
        return format!("{n} o");
    }
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {} ({n} o)", UNITS[u])
}

#[cfg(test)]
mod tests {
    use super::bytes;

    #[test]
    fn bytes_are_human_readable_and_exact() {
        assert_eq!(bytes(0), "0 o");
        assert_eq!(bytes(1023), "1023 o");
        assert_eq!(bytes(1024), "1.0 Kio (1024 o)");
        assert_eq!(bytes(3 * 1024 * 1024 / 2), "1.5 Mio (1572864 o)");
        assert_eq!(bytes(1 << 40), "1.0 Tio (1099511627776 o)");
    }
}
