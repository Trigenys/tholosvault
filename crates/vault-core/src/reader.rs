//! Lecture d'une archive par mmap. Les métadonnées sont lues sans décompresser
//! un seul morceau (L-002) ; la décompression n'a lieu qu'à la lecture d'un
//! fichier ou à la vérification.

use std::fs;
use std::path::Path;

use memmap2::Mmap;

use crate::codec;
use crate::error::{Error, Result};
use crate::format::{self, Header, Trailer, HEADER_LEN, TRAILER_COPIES, TRAILER_LEN};
use crate::merkle::{self, Hash};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkEntry {
    pub offset: u64,
    pub compressed_len: u32,
    pub len: u32,
    pub hash: Hash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub root: Hash,
    pub chunks: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerifyReport {
    pub chunks_checked: u64,
    /// Indices des morceaux dont le contenu ne correspond pas à leur hachage.
    pub bad_chunks: Vec<u32>,
    /// Chemins des fichiers dont la racine de Merkle ne correspond pas.
    pub bad_files: Vec<String>,
    pub archive_root_ok: bool,
}

impl VerifyReport {
    pub fn is_ok(&self) -> bool {
        self.bad_chunks.is_empty() && self.bad_files.is_empty() && self.archive_root_ok
    }
}

pub struct Archive {
    map: Mmap,
    header: Header,
    trailer: Trailer,
    chunks: Vec<ChunkEntry>,
    files: Vec<FileEntry>,
    /// (hash, index) trié par hash.
    hash_table: Vec<(Hash, u32)>,
}

impl Archive {
    pub fn open(path: &Path) -> Result<Self> {
        let file = fs::File::open(path).map_err(|e| Error::io(path, e))?;
        // Sûreté : lecture seule ; une modification concurrente du fichier est
        // hors contrat (archive immuable par conception).
        let map = unsafe { Mmap::map(&file) }.map_err(|e| Error::io(path, e))?;
        Self::from_mmap(map)
    }

    fn from_mmap(map: Mmap) -> Result<Self> {
        let len = map.len() as u64;
        let min = (HEADER_LEN + TRAILER_LEN * TRAILER_COPIES) as u64;
        if len < min {
            return Err(Error::TooShort { len });
        }
        let header = Header::decode(&map[..HEADER_LEN])?;
        let trailer = read_trailer(&map).ok_or(Error::NoValidTrailer)?;

        let chunk_bytes = section(
            &map,
            "index morceaux",
            trailer.chunk_index_off,
            trailer.chunk_index_len,
        )?;
        let chunks = parse_chunks(chunk_bytes, trailer.chunk_count)?;

        let file_bytes = section(
            &map,
            "table fichiers",
            trailer.file_table_off,
            trailer.file_table_len,
        )?;
        let files = parse_files(file_bytes, trailer.file_count, chunks.len())?;

        let hash_bytes = section(
            &map,
            "table hachage",
            trailer.hash_table_off,
            trailer.hash_table_len,
        )?;
        let hash_table = parse_hash_table(hash_bytes, chunks.len())?;

        for (i, c) in chunks.iter().enumerate() {
            let end = c.offset.saturating_add(c.compressed_len as u64);
            if c.offset < HEADER_LEN as u64 || end > trailer.chunk_index_off {
                return Err(Error::MalformedSection {
                    section: "index morceaux",
                    detail: format!("morceau {i} hors de la zone de données"),
                });
            }
        }

        Ok(Self {
            map,
            header,
            trailer,
            chunks,
            files,
            hash_table,
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn archive_root(&self) -> &Hash {
        &self.trailer.archive_root
    }

    pub fn files(&self) -> &[FileEntry] {
        &self.files
    }

    pub fn chunks(&self) -> &[ChunkEntry] {
        &self.chunks
    }

    pub fn find(&self, path: &str) -> Option<&FileEntry> {
        self.files
            .binary_search_by(|f| f.path.as_bytes().cmp(path.as_bytes()))
            .ok()
            .map(|i| &self.files[i])
    }

    pub fn chunk_index_by_hash(&self, hash: &Hash) -> Option<u32> {
        self.hash_table
            .binary_search_by(|(h, _)| h.cmp(hash))
            .ok()
            .map(|i| self.hash_table[i].1)
    }

    /// Décompresse un morceau et vérifie son hachage.
    pub fn read_chunk(&self, index: u32) -> Result<Vec<u8>> {
        let c = self
            .chunks
            .get(index as usize)
            .ok_or_else(|| Error::MalformedSection {
                section: "index morceaux",
                detail: format!("indice {index} hors table"),
            })?;
        let start = c.offset as usize;
        let frame = &self.map[start..start + c.compressed_len as usize];
        let data = codec::decompress(frame, c.len as usize, index)?;
        let actual = merkle::chunk_id(&data);
        if actual != c.hash {
            return Err(Error::ChunkMismatch {
                index,
                expected: merkle::to_hex(&c.hash),
                actual: merkle::to_hex(&actual),
            });
        }
        Ok(data)
    }

    /// Contenu complet d'un fichier, vérifié morceau par morceau.
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let f = self
            .find(path)
            .ok_or_else(|| Error::NotFound(path.to_string()))?;
        let mut out = Vec::with_capacity(f.size as usize);
        for &idx in &f.chunks {
            out.extend_from_slice(&self.read_chunk(idx)?);
        }
        Ok(out)
    }

    /// Vérifie tous les morceaux, toutes les racines de fichier et la racine d'archive.
    pub fn verify(&self) -> VerifyReport {
        let mut report = VerifyReport::default();
        for i in 0..self.chunks.len() as u32 {
            report.chunks_checked += 1;
            if self.read_chunk(i).is_err() {
                report.bad_chunks.push(i);
            }
        }
        let mut leaves = Vec::with_capacity(self.files.len());
        for f in &self.files {
            let hashes: Vec<Hash> = f
                .chunks
                .iter()
                .map(|&i| self.chunks[i as usize].hash)
                .collect();
            let root = merkle::merkle_root(&hashes);
            let size: u64 = f
                .chunks
                .iter()
                .map(|&i| self.chunks[i as usize].len as u64)
                .sum();
            if root != f.root || size != f.size {
                report.bad_files.push(f.path.clone());
            }
            leaves.push(merkle::file_leaf(&f.path, f.size, &f.root));
        }
        report.archive_root_ok = merkle::merkle_root(&leaves) == self.trailer.archive_root;
        report
    }
}

fn read_trailer(map: &[u8]) -> Option<Trailer> {
    let len = map.len();
    (1..=TRAILER_COPIES).find_map(|k| {
        let end = len - TRAILER_LEN * (k - 1);
        Trailer::decode(&map[end - TRAILER_LEN..end])
    })
}

fn section<'a>(map: &'a [u8], name: &'static str, off: u64, len: u64) -> Result<&'a [u8]> {
    let file_len = map.len() as u64;
    let end = off.checked_add(len);
    match end {
        Some(end) if off >= HEADER_LEN as u64 && end <= file_len => {
            Ok(&map[off as usize..end as usize])
        }
        _ => Err(Error::SectionOutOfBounds {
            section: name,
            offset: off,
            len,
            file_len,
        }),
    }
}

fn parse_chunks(b: &[u8], count: u64) -> Result<Vec<ChunkEntry>> {
    let expected = count.checked_mul(format::CHUNK_ENTRY_LEN as u64);
    if expected != Some(b.len() as u64) {
        return Err(Error::MalformedSection {
            section: "index morceaux",
            detail: format!("{} octets pour {count} entrées", b.len()),
        });
    }
    let mut out = Vec::with_capacity(count as usize);
    for e in b.chunks_exact(format::CHUNK_ENTRY_LEN) {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&e[16..48]);
        out.push(ChunkEntry {
            offset: format::u64_at(e, 0),
            compressed_len: format::u32_at(e, 8),
            len: format::u32_at(e, 12),
            hash,
        });
    }
    Ok(out)
}

fn parse_files(b: &[u8], count: u64, chunk_count: usize) -> Result<Vec<FileEntry>> {
    let malformed = |detail: String| Error::MalformedSection {
        section: "table fichiers",
        detail,
    };
    let mut out = Vec::with_capacity(count as usize);
    let mut pos = 0usize;
    for n in 0..count {
        let need = |p: usize, k: usize| -> Result<()> {
            if p + k > b.len() {
                Err(malformed(format!("entrée {n} tronquée")))
            } else {
                Ok(())
            }
        };
        need(pos, 2)?;
        let plen = format::u16_at(b, pos) as usize;
        pos += 2;
        need(pos, plen + 8 + 32 + 4)?;
        let path = std::str::from_utf8(&b[pos..pos + plen])
            .map_err(|_| malformed(format!("entrée {n} : chemin non UTF-8")))?
            .to_string();
        pos += plen;
        let size = format::u64_at(b, pos);
        pos += 8;
        let mut root = [0u8; 32];
        root.copy_from_slice(&b[pos..pos + 32]);
        pos += 32;
        let nchunks = format::u32_at(b, pos) as usize;
        pos += 4;
        need(pos, nchunks * 4)?;
        let mut chunks = Vec::with_capacity(nchunks);
        for i in 0..nchunks {
            let idx = format::u32_at(b, pos + i * 4);
            if idx as usize >= chunk_count {
                return Err(malformed(format!(
                    "{path} : indice de morceau {idx} hors table"
                )));
            }
            chunks.push(idx);
        }
        pos += nchunks * 4;
        if let Some(prev) = out.last() {
            let prev: &FileEntry = prev;
            if prev.path.as_bytes() >= path.as_bytes() {
                return Err(malformed(format!("chemins non triés autour de {path}")));
            }
        }
        out.push(FileEntry {
            path,
            size,
            root,
            chunks,
        });
    }
    if pos != b.len() {
        return Err(malformed(format!("{} octets excédentaires", b.len() - pos)));
    }
    Ok(out)
}

fn parse_hash_table(b: &[u8], chunk_count: usize) -> Result<Vec<(Hash, u32)>> {
    let malformed = |detail: String| Error::MalformedSection {
        section: "table hachage",
        detail,
    };
    if b.len() != chunk_count * format::HASH_ENTRY_LEN {
        return Err(malformed(format!(
            "{} octets pour {chunk_count} morceaux",
            b.len()
        )));
    }
    let mut out = Vec::with_capacity(chunk_count);
    for e in b.chunks_exact(format::HASH_ENTRY_LEN) {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&e[0..32]);
        let idx = format::u32_at(e, 32);
        if idx as usize >= chunk_count {
            return Err(malformed(format!("indice {idx} hors table")));
        }
        if let Some((prev, _)) = out.last() {
            if prev >= &hash {
                return Err(malformed("hachages non triés".to_string()));
            }
        }
        out.push((hash, idx));
    }
    Ok(out)
}
