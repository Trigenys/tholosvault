//! Cœur du format d'archive tholosvault (nom de travail interne : VAULT).
//!
//! Format v1 : découpage content-defined (FastCDC), identifiant de morceau
//! BLAKE3, compression zstd par morceau, arbre de Merkle par fichier et racine
//! d'archive, index et table de hachage en clair en fin de fichier, section de
//! parité Reed-Solomon entrelacée (L-004) puis trailer répliqué. Le lecteur
//! accepte les archives v0 (sans parité). Spécifications :
//! `docs/format/spec-v0.md`, `docs/format/spec-v1.md`.
//!
//! Hors périmètre : tier froid solide, dictionnaires, chiffrement. Chaque ajout
//! passe par une version de format.

pub mod cache;
pub mod chunk;
pub mod codec;
pub mod diff;
pub mod error;
pub mod format;
pub mod merkle;
pub mod parity;
pub mod path;
pub mod reader;
pub mod summary;
pub mod writer;

pub use cache::{CacheStats, ChunkCache};
pub use chunk::ChunkParams;
pub use diff::{diff, ArchiveDiff, FileChange, FileDiff};
pub use error::{Error, Result};
pub use merkle::Hash;
pub use parity::{Damage, Geometry, ParityOptions};
pub use reader::{
    parse_archive_tables, read_trailer_tail, repair_file, scan_file, verify_chunk_frame, Archive,
    ArchiveTables, ChunkEntry, FileEntry, RepairReport, VerifyReport,
};
pub use summary::ArchiveSummary;
pub use writer::{pack_dir, PackOptions, PackSummary};
