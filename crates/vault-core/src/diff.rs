//! Aperçu du delta entre deux archives (thèse H6, écran « Mise à jour delta ») :
//! fichiers ajoutés, supprimés, modifiés, et morceaux de la nouvelle archive
//! absents de l'ancienne, c'est-à-dire ce qu'une mise à jour devrait transférer.
//!
//! Calcul lu dans les tables seules, sans décompresser un morceau (L-002) : un
//! morceau est identifié par le BLAKE3 de son contenu décompressé, donc deux
//! archives compressées à des niveaux zstd différents partagent leurs morceaux
//! tant que leurs paramètres de découpe sont identiques.

use std::cmp::Ordering;

use crate::merkle::Hash;
use crate::reader::{Archive, FileEntry};

/// Nature du changement d'un fichier entre l'ancienne et la nouvelle archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileChange {
    Added,
    Removed,
    Modified,
    Unchanged,
}

impl FileChange {
    /// Mot-clé stable (sortie JSON, scripts).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Modified => "modified",
            Self::Unchanged => "unchanged",
        }
    }
}

/// Changement d'un fichier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub change: FileChange,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
    /// Références de morceaux du nouveau fichier absentes de l'ancienne archive
    /// (0 pour un fichier supprimé ou inchangé).
    pub new_chunk_refs: u64,
    /// Octets décompressés couverts par ces références.
    pub new_chunk_bytes: u64,
}

/// Bilan du delta entre deux archives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveDiff {
    pub old_root: Hash,
    pub new_root: Hash,
    /// Paramètres de découpe identiques : condition pour que les morceaux des
    /// deux archives coïncident. Faux, le delta reste exact mais proche de la
    /// nouvelle archive entière.
    pub same_chunking: bool,
    /// Tous les fichiers des deux archives, triés par chemin (octets UTF-8).
    pub files: Vec<FileDiff>,
    /// Morceaux uniques de la nouvelle archive absents de l'ancienne.
    pub transfer_chunks: u64,
    /// Octets décompressés de ces morceaux.
    pub transfer_logical_bytes: u64,
    /// Octets des frames zstd de ces morceaux dans la nouvelle archive : ce
    /// qu'une mise à jour transfère, hors métadonnées et parité.
    pub transfer_compressed_bytes: u64,
    /// Morceaux uniques de la nouvelle archive déjà présents dans l'ancienne.
    pub reused_chunks: u64,
    /// Octets des frames zstd réutilisables (dans la nouvelle archive).
    pub reused_compressed_bytes: u64,
    /// Octets des frames zstd de toute la nouvelle archive.
    pub new_compressed_bytes: u64,
    /// Index, table des fichiers et table de hachage de la nouvelle archive,
    /// toujours à transférer pour reconstruire l'archive.
    pub new_metadata_bytes: u64,
}

impl ArchiveDiff {
    /// Mêmes racines : rien à transférer.
    pub fn is_identical(&self) -> bool {
        self.old_root == self.new_root
    }

    pub fn count(&self, change: FileChange) -> usize {
        self.files.iter().filter(|f| f.change == change).count()
    }

    /// Fichiers ajoutés, supprimés ou modifiés.
    pub fn changed(&self) -> impl Iterator<Item = &FileDiff> {
        self.files
            .iter()
            .filter(|f| f.change != FileChange::Unchanged)
    }

    /// Part des frames de la nouvelle archive à transférer, en % (0 si vide).
    pub fn transfer_percent(&self) -> f64 {
        if self.new_compressed_bytes == 0 {
            return 0.0;
        }
        100.0 * self.transfer_compressed_bytes as f64 / self.new_compressed_bytes as f64
    }
}

/// Delta de `old` vers `new`, depuis les tables seules.
pub fn diff(old: &Archive, new: &Archive) -> ArchiveDiff {
    let in_old: Vec<bool> = new
        .chunks()
        .iter()
        .map(|c| old.chunk_index_by_hash(&c.hash).is_some())
        .collect();

    let mut d = ArchiveDiff {
        old_root: *old.archive_root(),
        new_root: *new.archive_root(),
        same_chunking: same_chunking(old, new),
        files: Vec::with_capacity(old.files().len().max(new.files().len())),
        transfer_chunks: 0,
        transfer_logical_bytes: 0,
        transfer_compressed_bytes: 0,
        reused_chunks: 0,
        reused_compressed_bytes: 0,
        new_compressed_bytes: 0,
        new_metadata_bytes: {
            let t = new.trailer();
            t.chunk_index_len + t.file_table_len + t.hash_table_len
        },
    };
    for (c, &known) in new.chunks().iter().zip(&in_old) {
        d.new_compressed_bytes += c.compressed_len as u64;
        if known {
            d.reused_chunks += 1;
            d.reused_compressed_bytes += c.compressed_len as u64;
        } else {
            d.transfer_chunks += 1;
            d.transfer_logical_bytes += c.len as u64;
            d.transfer_compressed_bytes += c.compressed_len as u64;
        }
    }

    let fresh = |f: &FileEntry| -> (u64, u64) {
        f.chunks
            .iter()
            .filter(|&&i| !in_old[i as usize])
            .fold((0, 0), |(n, b), &i| {
                (n + 1, b + new.chunks()[i as usize].len as u64)
            })
    };

    // Les deux tables de fichiers sont triées par chemin (octets) : fusion linéaire.
    let (of, nf) = (old.files(), new.files());
    let (mut i, mut j) = (0, 0);
    while i < of.len() || j < nf.len() {
        let ord = match (of.get(i), nf.get(j)) {
            (Some(a), Some(b)) => a.path.as_bytes().cmp(b.path.as_bytes()),
            (Some(_), None) => Ordering::Less,
            _ => Ordering::Greater,
        };
        let entry = match ord {
            Ordering::Less => {
                let a = &of[i];
                i += 1;
                FileDiff {
                    path: a.path.clone(),
                    change: FileChange::Removed,
                    old_size: Some(a.size),
                    new_size: None,
                    new_chunk_refs: 0,
                    new_chunk_bytes: 0,
                }
            }
            Ordering::Greater => {
                let b = &nf[j];
                j += 1;
                let (n, bytes) = fresh(b);
                FileDiff {
                    path: b.path.clone(),
                    change: FileChange::Added,
                    old_size: None,
                    new_size: Some(b.size),
                    new_chunk_refs: n,
                    new_chunk_bytes: bytes,
                }
            }
            Ordering::Equal => {
                let (a, b) = (&of[i], &nf[j]);
                i += 1;
                j += 1;
                let same = a.root == b.root && a.size == b.size;
                let (n, bytes) = if same { (0, 0) } else { fresh(b) };
                FileDiff {
                    path: b.path.clone(),
                    change: if same {
                        FileChange::Unchanged
                    } else {
                        FileChange::Modified
                    },
                    old_size: Some(a.size),
                    new_size: Some(b.size),
                    new_chunk_refs: n,
                    new_chunk_bytes: bytes,
                }
            }
        };
        d.files.push(entry);
    }
    d
}

fn same_chunking(a: &Archive, b: &Archive) -> bool {
    let (x, y) = (a.header(), b.header());
    (x.chunk_min, x.chunk_avg, x.chunk_max) == (y.chunk_min, y.chunk_avg, y.chunk_max)
}

#[cfg(test)]
mod tests {
    use super::FileChange;

    #[test]
    fn change_keywords_are_stable() {
        assert_eq!(FileChange::Added.as_str(), "added");
        assert_eq!(FileChange::Removed.as_str(), "removed");
        assert_eq!(FileChange::Modified.as_str(), "modified");
        assert_eq!(FileChange::Unchanged.as_str(), "unchanged");
    }
}
