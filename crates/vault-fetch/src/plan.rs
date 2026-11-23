//! Plan de téléchargement : découpe de l'archive en unités alignées sur sa
//! structure (en-tête, morceaux, tables, parité, trailer), puis regroupement
//! des unités manquantes en segments contigus.

use std::ops::Range;

use vault_core::format::{Trailer, HEADER_LEN, TRAILER_COPIES, TRAILER_LEN};
use vault_core::ArchiveTables;

use crate::error::{Error, Result};

pub const TRAILER_TAIL_LEN: u64 = (TRAILER_LEN * TRAILER_COPIES) as u64;
/// Taille cible d'un segment (une requête).
pub const DEFAULT_SEGMENT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    Header,
    /// Frame zstd du morceau d'index donné ; vérifiée par BLAKE3 à l'arrivée.
    Chunk(u32),
    /// Index des morceaux + table des fichiers + table de hachage (contiguës).
    Tables,
    /// Tranche de la section de parité ; octets bruts.
    Parity,
    Trailer,
    /// Octets non décrits par les tables (jamais produits par le writer v1,
    /// tolérés pour rester lisible si le format gagne des sections).
    Gap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub kind: UnitKind,
    pub range: Range<u64>,
}

impl Unit {
    pub fn len(&self) -> u64 {
        self.range.end - self.range.start
    }

    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    /// Unité dont les octets sont du contenu de l'archive (hors métadonnées).
    pub fn is_content(&self) -> bool {
        matches!(
            self.kind,
            UnitKind::Header | UnitKind::Chunk(_) | UnitKind::Parity | UnitKind::Gap
        )
    }
}

/// Plage d'octets couverte par les trois tables, dans l'ordre du fichier.
pub fn tables_range(t: &Trailer) -> Range<u64> {
    t.chunk_index_off..t.hash_table_off + t.hash_table_len
}

pub fn trailer_range(len: u64) -> Range<u64> {
    len - TRAILER_TAIL_LEN..len
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub len: u64,
    /// Unités triées par offset, sans chevauchement, couvrant `0..len`.
    pub units: Vec<Unit>,
}

impl Plan {
    /// Construit le plan à partir du trailer et des tables reçus ; `len` est
    /// la longueur annoncée par la source.
    pub fn build(
        trailer: &Trailer,
        tables: &ArchiveTables,
        len: u64,
        segment_bytes: u64,
    ) -> Result<Plan> {
        if len < TRAILER_TAIL_LEN + HEADER_LEN as u64 {
            return Err(Error::Layout(format!(
                "document de {len} octets, trop court"
            )));
        }
        let mut units = Vec::with_capacity(tables.chunks.len() + 8);
        units.push(Unit {
            kind: UnitKind::Header,
            range: 0..HEADER_LEN as u64,
        });
        for (i, c) in tables.chunks.iter().enumerate() {
            units.push(Unit {
                kind: UnitKind::Chunk(i as u32),
                range: c.offset..c.offset + c.compressed_len as u64,
            });
        }
        units.push(Unit {
            kind: UnitKind::Tables,
            range: tables_range(trailer),
        });
        let parity = trailer.parity_off()..trailer.parity_off() + trailer.parity_len;
        let mut off = parity.start;
        while off < parity.end {
            let end = (off + segment_bytes).min(parity.end);
            units.push(Unit {
                kind: UnitKind::Parity,
                range: off..end,
            });
            off = end;
        }
        units.push(Unit {
            kind: UnitKind::Trailer,
            range: trailer_range(len),
        });
        units.retain(|u| !u.is_empty());
        units.sort_by_key(|u| u.range.start);

        // Couverture exacte de 0..len ; les trous deviennent des unités brutes.
        let mut covered = Vec::with_capacity(units.len() + 2);
        let mut cursor = 0u64;
        for u in units {
            if u.range.start < cursor {
                return Err(Error::Layout(format!(
                    "unités en chevauchement autour de l'octet {}",
                    u.range.start
                )));
            }
            if u.range.start > cursor {
                covered.push(Unit {
                    kind: UnitKind::Gap,
                    range: cursor..u.range.start,
                });
            }
            cursor = u.range.end;
            covered.push(u);
        }
        if cursor != len {
            return Err(Error::Layout(format!(
                "les tables décrivent {cursor} octets, la source en annonce {len}"
            )));
        }
        Ok(Plan {
            len,
            units: covered,
        })
    }

    pub fn content_bytes(&self) -> u64 {
        self.units
            .iter()
            .filter(|u| u.is_content())
            .map(Unit::len)
            .sum()
    }

    /// Regroupe les unités dont `missing(i)` est vrai en segments contigus
    /// d'au plus `segment_bytes` (une unité plus grande forme son propre segment).
    pub fn segments(&self, segment_bytes: u64, missing: impl Fn(usize) -> bool) -> Vec<Segment> {
        let mut out: Vec<Segment> = Vec::new();
        for (i, u) in self.units.iter().enumerate() {
            if !missing(i) {
                continue;
            }
            if let Some(last) = out.last_mut() {
                if last.range.end == u.range.start && last.len() + u.len() <= segment_bytes {
                    last.range.end = u.range.end;
                    last.units.push(i);
                    continue;
                }
            }
            out.push(Segment {
                range: u.range.clone(),
                units: vec![i],
            });
        }
        out
    }
}

/// Une requête : plage d'octets et indices (dans `Plan::units`) des unités
/// qu'elle couvre, dans l'ordre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub range: Range<u64>,
    pub units: Vec<usize>,
}

impl Segment {
    pub fn len(&self) -> u64 {
        self.range.end - self.range.start
    }

    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_core::merkle::Hash;
    use vault_core::{ChunkEntry, FileEntry};

    fn fixture(chunk_lens: &[u32], parity_len: u64) -> (Trailer, ArchiveTables, u64) {
        let mut off = HEADER_LEN as u64;
        let mut chunks = Vec::new();
        for &l in chunk_lens {
            chunks.push(ChunkEntry {
                offset: off,
                compressed_len: l,
                len: l * 2,
                hash: [0u8; 32],
            });
            off += l as u64;
        }
        let tables_len = 300u64;
        let trailer = Trailer {
            chunk_index_off: off,
            chunk_index_len: 100,
            file_table_off: off + 100,
            file_table_len: 100,
            hash_table_off: off + 200,
            hash_table_len: 100,
            chunk_count: chunks.len() as u64,
            file_count: 1,
            archive_root: [0u8; 32],
            data_len: off,
            parity_len,
        };
        let len = off + tables_len + parity_len + TRAILER_TAIL_LEN;
        let tables = ArchiveTables {
            chunks,
            files: vec![FileEntry {
                path: "a".into(),
                size: 1,
                root: Hash::default(),
                chunks: vec![0],
            }],
            hash_table: vec![],
        };
        (trailer, tables, len)
    }

    #[test]
    fn covers_whole_document_in_order() {
        let (t, tb, len) = fixture(&[10, 20, 30], 50);
        let p = Plan::build(&t, &tb, len, 16).unwrap();
        let kinds: Vec<_> = p.units.iter().map(|u| u.kind).collect();
        assert_eq!(
            kinds,
            vec![
                UnitKind::Header,
                UnitKind::Chunk(0),
                UnitKind::Chunk(1),
                UnitKind::Chunk(2),
                UnitKind::Tables,
                UnitKind::Parity,
                UnitKind::Parity,
                UnitKind::Parity,
                UnitKind::Parity,
                UnitKind::Trailer,
            ]
        );
        assert_eq!(p.units.first().unwrap().range.start, 0);
        assert_eq!(p.units.last().unwrap().range.end, len);
        assert_eq!(p.content_bytes(), 64 + 60 + 50);
    }

    #[test]
    fn rejects_overlap_and_tolerates_gap() {
        let (t, tb, len) = fixture(&[10], 0);
        // Source plus courte : le trailer chevauche les tables.
        assert!(matches!(
            Plan::build(&t, &tb, len - 1, 1024),
            Err(Error::Layout(_))
        ));
        // Source plus longue : l'octet non décrit devient une unité brute.
        let p = Plan::build(&t, &tb, len + 1, 1024).unwrap();
        assert!(p
            .units
            .iter()
            .any(|u| u.kind == UnitKind::Gap && u.len() == 1));
    }

    #[test]
    fn segments_group_contiguous_missing_units() {
        let (t, tb, len) = fixture(&[10, 20, 30, 40], 0);
        let p = Plan::build(&t, &tb, len, 1024).unwrap();
        // Unité 2 (Chunk(1)) déjà vérifiée : deux segments de part et d'autre.
        let segs = p.segments(1024, |i| i != 2);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].units, vec![0, 1]);
        assert_eq!(segs[1].units, vec![3, 4, 5, 6]);
        // Cible de 45 octets : Header+Chunk0 (74) dépasse déjà → séparés.
        let segs = p.segments(45, |_| true);
        assert_eq!(segs[0].units, vec![0]);
        assert_eq!(segs[1].units, vec![1, 2]);
    }
}
