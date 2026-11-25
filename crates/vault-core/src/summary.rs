//! Bilan d'une archive lu dans l'en-tête, le trailer et les tables, sans
//! décompresser un seul morceau (L-002) : données de `tholos info` et de
//! l'écran « Bibliothèque » (taille archive vs taille logique, ratio, racine).

use crate::format::{TRAILER_COPIES, TRAILER_LEN};
use crate::merkle::Hash;
use crate::reader::Archive;

/// Chiffres d'une archive. Toutes les tailles sont en octets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveSummary {
    pub format_version: u16,
    pub codec_level: i8,
    /// Version numérique de zstd à l'encodage (ex. 10507 pour 1.5.7).
    pub codec_version: u32,
    pub chunk_min: u32,
    pub chunk_avg: u32,
    pub chunk_max: u32,
    pub files: u64,
    /// Somme des tailles de fichier (contenu restitué à l'extraction).
    pub logical_bytes: u64,
    /// Références de morceaux, tous fichiers confondus (doublons compris).
    pub chunk_refs: u64,
    pub unique_chunks: u64,
    /// Octets décompressés des morceaux uniques (après déduplication).
    pub unique_bytes: u64,
    /// Octets des frames zstd stockées.
    pub compressed_bytes: u64,
    /// Index des morceaux, table des fichiers et table de hachage.
    pub metadata_bytes: u64,
    pub parity_bytes: u64,
    /// Taille de l'archive (en-tête, frames, tables, parité, trailers).
    pub archive_bytes: u64,
    pub archive_root: Hash,
}

impl ArchiveSummary {
    /// Octets économisés par la déduplication (`logical - unique`).
    pub fn dedup_saved_bytes(&self) -> u64 {
        self.logical_bytes.saturating_sub(self.unique_bytes)
    }

    /// Taille logique / taille de l'archive (`None` pour une archive vide).
    pub fn ratio(&self) -> Option<f64> {
        (self.logical_bytes > 0 && self.archive_bytes > 0)
            .then(|| self.logical_bytes as f64 / self.archive_bytes as f64)
    }

    /// Part de la parité dans l'archive, en %.
    pub fn parity_percent(&self) -> f64 {
        if self.archive_bytes == 0 {
            return 0.0;
        }
        100.0 * self.parity_bytes as f64 / self.archive_bytes as f64
    }

    /// Version zstd lisible (`1.5.7`).
    pub fn codec_version_string(&self) -> String {
        let v = self.codec_version;
        format!("{}.{}.{}", v / 10_000, v / 100 % 100, v % 100)
    }
}

impl Archive {
    /// Bilan de l'archive, sans décompression.
    pub fn summary(&self) -> ArchiveSummary {
        let h = self.header();
        let t = self.trailer();
        let files = self.files();
        let chunks = self.chunks();
        ArchiveSummary {
            format_version: h.format_version,
            codec_level: h.codec_level,
            codec_version: h.codec_version,
            chunk_min: h.chunk_min,
            chunk_avg: h.chunk_avg,
            chunk_max: h.chunk_max,
            files: files.len() as u64,
            logical_bytes: files.iter().map(|f| f.size).sum(),
            chunk_refs: files.iter().map(|f| f.chunks.len() as u64).sum(),
            unique_chunks: chunks.len() as u64,
            unique_bytes: chunks.iter().map(|c| c.len as u64).sum(),
            compressed_bytes: chunks.iter().map(|c| c.compressed_len as u64).sum(),
            metadata_bytes: t.chunk_index_len + t.file_table_len + t.hash_table_len,
            parity_bytes: t.parity_len,
            archive_bytes: t.parity_off() + t.parity_len + (TRAILER_LEN * TRAILER_COPIES) as u64,
            archive_root: t.archive_root,
        }
    }
}
