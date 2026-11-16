//! Cœur du format d'archive tholosvault (nom de travail interne : VAULT).
//!
//! Version 0 du format : découpage content-defined (FastCDC), identifiant de
//! morceau BLAKE3, compression zstd par morceau, arbre de Merkle par fichier et
//! racine d'archive, index et table de hachage en clair en fin de fichier,
//! trailer répliqué. Spécification : `docs/format/spec-v0.md`.
//!
//! Hors périmètre de cette version : parité Reed-Solomon, tier froid solide,
//! dictionnaires, chiffrement. Chaque ajout passe par une version de format.

pub mod chunk;
pub mod codec;
pub mod error;
pub mod format;
pub mod merkle;
pub mod reader;
pub mod writer;

pub use chunk::ChunkParams;
pub use error::{Error, Result};
pub use merkle::Hash;
pub use reader::{Archive, ChunkEntry, FileEntry, VerifyReport};
pub use writer::{pack_dir, PackOptions, PackSummary};
