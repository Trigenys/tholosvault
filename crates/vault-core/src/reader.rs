//! Lecture d'une archive par mmap. Les métadonnées sont lues sans décompresser
//! un seul morceau (L-002) ; la décompression n'a lieu qu'à la lecture d'un
//! fichier ou à la vérification.
//!
//! Réparation (v1) : [`scan_file`] et [`repair_file`] travaillent sur le chemin,
//! sans exiger que l'en-tête ou les tables soient lisibles. La réparation écrit
//! une copie temporaire puis la renomme : jamais de réécriture d'un fichier
//! potentiellement mappé (I-001).

use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use memmap2::Mmap;

use crate::cache::ChunkCache;
use crate::codec;
use crate::error::{Error, Result};
use crate::format::{self, Header, Trailer, HEADER_LEN, TRAILER_COPIES, TRAILER_LEN};
use crate::merkle::{self, Hash};
use crate::parity::{self, Damage, ParitySection, Patch};

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
    /// Bilan à la granularité des shards (parité, trailer). `None` tant que
    /// `verify` n'a pas été appelé.
    pub damage: Option<Damage>,
}

impl VerifyReport {
    /// Contenu intègre (indépendamment de l'état de la parité ou des copies du trailer).
    pub fn is_ok(&self) -> bool {
        self.bad_chunks.is_empty() && self.bad_files.is_empty() && self.archive_root_ok
    }

    /// Vrai si l'archive est saine ou si tout dégât constaté est reconstructible.
    pub fn repairable(&self) -> bool {
        match &self.damage {
            Some(d) => d.repairable,
            None => self.is_ok(),
        }
    }
}

/// Résultat d'une réparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairReport {
    pub damage: Damage,
    /// Shards (données + parité) réécrits.
    pub patched_shards: u64,
    pub trailer_copies_restored: u8,
    /// Faux si rien n'était à réparer (aucune écriture).
    pub rewritten: bool,
    pub verify_after: VerifyReport,
}

/// Offsets de début (dans le fichier décompressé) de chaque référence de
/// morceau, tous fichiers concaténés ; `base[f]..base[f + 1]` délimite le
/// fichier `f`. Construit paresseusement à la première lecture par plage :
/// `open()`, `list` et `verify` ne paient rien (L-002).
struct ChunkStarts {
    base: Vec<usize>,
    starts: Vec<u64>,
}

impl ChunkStarts {
    fn build(files: &[FileEntry], chunks: &[ChunkEntry]) -> Self {
        let mut base = Vec::with_capacity(files.len() + 1);
        let mut starts = Vec::with_capacity(files.iter().map(|f| f.chunks.len()).sum());
        for f in files {
            base.push(starts.len());
            let mut pos = 0u64;
            for &idx in &f.chunks {
                starts.push(pos);
                pos += chunks[idx as usize].len as u64;
            }
        }
        base.push(starts.len());
        Self { base, starts }
    }

    fn file_starts(&self, file: usize) -> &[u64] {
        &self.starts[self.base[file]..self.base[file + 1]]
    }

    /// Positions (dans `FileEntry::chunks`) des morceaux couvrant `offset..offset + len`.
    fn span(&self, file: usize, size: u64, offset: u64, len: usize) -> Range<usize> {
        let starts = self.file_starts(file);
        if len == 0 || offset >= size || starts.is_empty() {
            return 0..0;
        }
        let end = offset.saturating_add(len as u64).min(size);
        let first = starts.partition_point(|&s| s <= offset) - 1;
        let last = starts.partition_point(|&s| s < end);
        first..last
    }
}

struct Tables {
    chunks: Vec<ChunkEntry>,
    files: Vec<FileEntry>,
    /// (hash, index) trié par hash.
    hash_table: Vec<(Hash, u32)>,
}

pub struct Archive {
    path: PathBuf,
    map: Mmap,
    header: Header,
    trailer: Trailer,
    bad_trailer_copies: u8,
    chunks: Vec<ChunkEntry>,
    files: Vec<FileEntry>,
    hash_table: Vec<(Hash, u32)>,
    chunk_starts: OnceLock<ChunkStarts>,
}

impl Archive {
    pub fn open(path: &Path) -> Result<Self> {
        let map = map_file(path)?;
        Self::from_mmap(path.to_path_buf(), map)
    }

    fn from_mmap(path: PathBuf, map: Mmap) -> Result<Self> {
        check_min_len(&map)?;
        let header = Header::decode(&map[..HEADER_LEN])?;
        let (trailer, bad_trailer_copies) = locate_trailer(&map)?;
        let tables = parse_tables(&map, &trailer)?;
        Ok(Self {
            path,
            map,
            header,
            trailer,
            bad_trailer_copies,
            chunks: tables.chunks,
            files: tables.files,
            hash_table: tables.hash_table,
            chunk_starts: OnceLock::new(),
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn trailer(&self) -> &Trailer {
        &self.trailer
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

    /// Vrai si l'archive porte une section de parité.
    pub fn has_parity(&self) -> bool {
        self.trailer.parity_len > 0
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
        read_chunk_raw(&self.map, c, index)
    }

    /// Indice d'un fichier dans [`Archive::files`] : identifiant stable pour
    /// enchaîner des lectures par plage sans refaire la recherche.
    pub fn find_index(&self, path: &str) -> Option<usize> {
        self.files
            .binary_search_by(|f| f.path.as_bytes().cmp(path.as_bytes()))
            .ok()
    }

    fn chunk_starts(&self) -> &ChunkStarts {
        self.chunk_starts
            .get_or_init(|| ChunkStarts::build(&self.files, &self.chunks))
    }

    fn file_at(&self, file: usize) -> Result<&FileEntry> {
        self.files.get(file).ok_or_else(|| Error::MalformedSection {
            section: "table fichiers",
            detail: format!("indice de fichier {file} hors table"),
        })
    }

    /// Positions, dans `files()[file].chunks`, des morceaux qu'une lecture de
    /// `len` octets à `offset` doit décompresser. Ne lit aucune donnée.
    pub fn chunk_span(&self, file: usize, offset: u64, len: usize) -> Result<Range<usize>> {
        let f = self.file_at(file)?;
        Ok(self.chunk_starts().span(file, f.size, offset, len))
    }

    /// Lecture partielle, sémantique `pread` : copie dans `buf` les octets du
    /// fichier `file` à partir de `offset`, tronquée en fin de fichier, et renvoie
    /// le nombre d'octets écrits (0 si `offset >= size`). Seuls les morceaux
    /// couvrant la plage sont décompressés, chacun vérifié par son hachage ; la
    /// racine Merkle du fichier n'est pas recalculée (voir [`Archive::verify`]).
    pub fn read_range_into(&self, file: usize, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.read_range_with(file, offset, buf, |idx| self.read_chunk(idx).map(Arc::from))
    }

    /// Comme [`Archive::read_range_into`], mais les morceaux décompressés et
    /// vérifiés passent par `cache` : une suite de lectures partielles
    /// (montage, flux) ne décompresse chaque morceau qu'une fois, et des
    /// lecteurs concurrents d'un même morceau manquant attendent un seul
    /// décodage (vol unique). Un morceau corrompu renvoie l'erreur et n'entre
    /// jamais dans le cache.
    pub fn read_range_into_cached(
        &self,
        file: usize,
        offset: u64,
        buf: &mut [u8],
        cache: &ChunkCache,
    ) -> Result<usize> {
        self.read_range_with(file, offset, buf, |idx| {
            cache.get_or_decode(idx, || self.read_chunk(idx).map(Arc::from))
        })
    }

    fn read_range_with(
        &self,
        file: usize,
        offset: u64,
        buf: &mut [u8],
        mut chunk: impl FnMut(u32) -> Result<Arc<[u8]>>,
    ) -> Result<usize> {
        let f = self.file_at(file)?;
        if offset >= f.size || buf.is_empty() {
            return Ok(0);
        }
        let want = ((f.size - offset) as usize).min(buf.len());
        let cs = self.chunk_starts();
        let starts = cs.file_starts(file);
        let mut written = 0usize;
        for pos in cs.span(file, f.size, offset, want) {
            let data = chunk(f.chunks[pos])?;
            let skip = offset.saturating_sub(starts[pos]) as usize;
            let take = (data.len() - skip).min(want - written);
            buf[written..written + take].copy_from_slice(&data[skip..skip + take]);
            written += take;
            if written == want {
                break;
            }
        }
        Ok(written)
    }

    /// Variante allouante de [`Archive::read_range_into`] par chemin ; `len` est
    /// borné à la taille restante avant toute allocation.
    pub fn read_range(&self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>> {
        let file = self
            .find_index(path)
            .ok_or_else(|| Error::NotFound(path.to_string()))?;
        let size = self.files[file].size;
        let n = size.saturating_sub(offset).min(len as u64) as usize;
        let mut out = vec![0u8; n];
        let got = self.read_range_into(file, offset, &mut out)?;
        out.truncate(got);
        Ok(out)
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

    /// Vérifie tous les morceaux, toutes les racines de fichier, la racine
    /// d'archive, puis établit le bilan des dégâts et sa réparabilité.
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

        let ranges = chunk_ranges(&self.chunks, &report.bad_chunks);
        let mut damage = scan_map(&self.map, &self.trailer, &ranges, self.bad_trailer_copies);
        if damage.geometry.is_none() {
            damage.repairable = report.is_ok();
        }
        report.damage = Some(damage);
        report
    }

    /// Répare l'archive sur disque (ferme la vue mmap, cf. I-001) et renvoie le bilan.
    pub fn repair(self) -> Result<RepairReport> {
        let path = self.path.clone();
        drop(self);
        repair_file(&path)
    }
}

/// Bilan des dégâts d'une archive, même quand l'en-tête ou les tables sont illisibles.
pub fn scan_file(path: &Path) -> Result<Damage> {
    let map = map_file(path)?;
    check_min_len(&map)?;
    let (trailer, bad_copies) = locate_trailer(&map)?;
    let ranges = bad_chunk_ranges(&map, &trailer);
    Ok(scan_map(&map, &trailer, &ranges, bad_copies))
}

/// Reconstruit les shards effacés et les copies du trailer détruites, par
/// copie temporaire puis renommage. Sans écriture si rien n'est à réparer ou
/// si les dégâts dépassent la parité disponible.
pub fn repair_file(path: &Path) -> Result<RepairReport> {
    let (damage, patches) = {
        let map = map_file(path)?;
        check_min_len(&map)?;
        let (trailer, bad_copies) = locate_trailer(&map)?;
        let ranges = bad_chunk_ranges(&map, &trailer);
        let damage = scan_map(&map, &trailer, &ranges, bad_copies);
        if !damage.repairable {
            return Err(damage.unrepairable_error());
        }
        let mut patches = Vec::new();
        if !damage.bad_data_shards.is_empty() || !damage.bad_parity_shards.is_empty() {
            let (protected, section_bytes) = split_protected(&map, &trailer)?;
            let sec = ParitySection::parse(section_bytes, protected.len() as u64)?;
            patches = parity::reconstruct(protected, &sec, trailer.parity_off(), &damage)?;
        }
        let encoded = trailer.encode();
        let len = map.len();
        for k in 1..=TRAILER_COPIES {
            let end = len - TRAILER_LEN * (k - 1);
            let slot = &map[end - TRAILER_LEN..end];
            if slot != encoded {
                patches.push(Patch {
                    offset: (end - TRAILER_LEN) as u64,
                    bytes: encoded.to_vec(),
                });
            }
        }
        (damage, patches)
    };

    let patched_shards = (damage.bad_data_shards.len() + damage.bad_parity_shards.len()) as u64;
    let trailer_copies_restored = (patches.len() as u64 - patched_shards) as u8;
    let rewritten = !patches.is_empty();
    if rewritten {
        apply_patches(path, &patches)?;
    }
    let verify_after = Archive::open(path)?.verify();
    Ok(RepairReport {
        damage,
        patched_shards,
        trailer_copies_restored,
        rewritten,
        verify_after,
    })
}

fn apply_patches(path: &Path, patches: &[Patch]) -> Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let io_tmp = |e| Error::io(&tmp, e);
    fs::copy(path, &tmp).map_err(io_tmp)?;
    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .open(&tmp)
            .map_err(io_tmp)?;
        for p in patches {
            f.seek(SeekFrom::Start(p.offset)).map_err(io_tmp)?;
            f.write_all(&p.bytes).map_err(io_tmp)?;
        }
        f.sync_all().map_err(io_tmp)?;
    }
    fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
}

fn map_file(path: &Path) -> Result<Mmap> {
    let file = fs::File::open(path).map_err(|e| Error::io(path, e))?;
    // Sûreté : lecture seule ; une modification concurrente du fichier est
    // hors contrat (archive immuable par conception, verrouillée tant qu'une
    // vue existe, cf. I-001).
    unsafe { Mmap::map(&file) }.map_err(|e| Error::io(path, e))
}

fn check_min_len(map: &[u8]) -> Result<()> {
    let len = map.len() as u64;
    let min = (HEADER_LEN + TRAILER_LEN * TRAILER_COPIES) as u64;
    if len < min {
        return Err(Error::TooShort { len });
    }
    Ok(())
}

/// Zone protégée et octets de la section de parité (vide en v0).
fn split_protected<'a>(map: &'a [u8], trailer: &Trailer) -> Result<(&'a [u8], &'a [u8])> {
    let off = trailer.parity_off();
    let section_bytes = section(map, "parité", off, trailer.parity_len)?;
    let max = (map.len() - TRAILER_LEN * TRAILER_COPIES) as u64;
    if off + trailer.parity_len > max {
        return Err(Error::SectionOutOfBounds {
            section: "parité",
            offset: off,
            len: trailer.parity_len,
            file_len: map.len() as u64,
        });
    }
    Ok((&map[..off as usize], section_bytes))
}

fn scan_map(map: &[u8], trailer: &Trailer, ranges: &[Range<u64>], bad_copies: u8) -> Damage {
    let Ok((protected, section_bytes)) = split_protected(map, trailer) else {
        return parity::scan(&[], None, ranges, bad_copies);
    };
    if trailer.parity_len == 0 {
        return parity::scan(protected, None, ranges, bad_copies);
    }
    match ParitySection::parse(section_bytes, protected.len() as u64) {
        Ok(sec) => parity::scan(protected, Some(&sec), ranges, bad_copies),
        Err(_) => {
            let mut d = parity::scan(protected, None, ranges, bad_copies);
            // Section présente mais illisible : rien n'est reconstructible.
            d.repairable = false;
            d
        }
    }
}

/// Intervalles d'octets des morceaux dont le hachage ne correspond pas, si les
/// tables sont lisibles ; vide sinon (les CRC par shard restent la référence).
fn bad_chunk_ranges(map: &[u8], trailer: &Trailer) -> Vec<Range<u64>> {
    let Ok(tables) = parse_tables(map, trailer) else {
        return Vec::new();
    };
    let bad: Vec<u32> = (0..tables.chunks.len() as u32)
        .filter(|&i| read_chunk_raw(map, &tables.chunks[i as usize], i).is_err())
        .collect();
    chunk_ranges(&tables.chunks, &bad)
}

fn chunk_ranges(chunks: &[ChunkEntry], bad: &[u32]) -> Vec<Range<u64>> {
    bad.iter()
        .map(|&i| {
            let c = &chunks[i as usize];
            c.offset..c.offset + c.compressed_len as u64
        })
        .collect()
}

fn read_chunk_raw(map: &[u8], c: &ChunkEntry, index: u32) -> Result<Vec<u8>> {
    let start = c.offset as usize;
    let frame = &map[start..start + c.compressed_len as usize];
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

fn parse_tables(map: &[u8], trailer: &Trailer) -> Result<Tables> {
    let chunk_bytes = section(
        map,
        "index morceaux",
        trailer.chunk_index_off,
        trailer.chunk_index_len,
    )?;
    let chunks = parse_chunks(chunk_bytes, trailer.chunk_count)?;

    let file_bytes = section(
        map,
        "table fichiers",
        trailer.file_table_off,
        trailer.file_table_len,
    )?;
    let files = parse_files(file_bytes, trailer.file_count, chunks.len())?;

    let hash_bytes = section(
        map,
        "table hachage",
        trailer.hash_table_off,
        trailer.hash_table_len,
    )?;
    let hash_table = parse_hash_table(hash_bytes, chunks.len())?;

    for (i, f) in files.iter().enumerate() {
        let total: u64 = f
            .chunks
            .iter()
            .map(|&c| chunks[c as usize].len as u64)
            .sum();
        if total != f.size {
            return Err(Error::MalformedSection {
                section: "table fichiers",
                detail: format!("fichier {i} : taille {} mais morceaux {total}", f.size),
            });
        }
    }
    for (i, c) in chunks.iter().enumerate() {
        let end = c.offset.saturating_add(c.compressed_len as u64);
        if c.offset < HEADER_LEN as u64 || end > trailer.chunk_index_off {
            return Err(Error::MalformedSection {
                section: "index morceaux",
                detail: format!("morceau {i} hors de la zone de données"),
            });
        }
    }
    Ok(Tables {
        chunks,
        files,
        hash_table,
    })
}

/// Trailer valide (dernière copie d'abord) et nombre de copies illisibles.
/// Trailer de l'archive et nombre de copies de queue illisibles. Quand les
/// deux copies sont détruites, balaie la fin du fichier à la recherche de la
/// section de parité et prend la copie qu'elle embarque (disposition 1).
fn locate_trailer(map: &[u8]) -> Result<(Trailer, u8)> {
    match read_trailers(map) {
        (Some(t), bad) => Ok((t, bad)),
        (None, bad) => recover_trailer_from_parity(map)
            .map(|t| (t, bad))
            .ok_or(Error::NoValidTrailer),
    }
}

/// Balayage de secours : cherche en remontant depuis la fin un en-tête de
/// section de parité intact dont la copie de trailer désigne exactement cette
/// position et cette longueur de fichier. Coût proportionnel à la taille de la
/// section pour une archive saine ; pas de faux positif sans collision CRC.
fn recover_trailer_from_parity(map: &[u8]) -> Option<Trailer> {
    let len = map.len();
    let body = &map[..len - TRAILER_LEN * TRAILER_COPIES];
    let mut search_end = body.len();
    while let Some(off) = body[..search_end]
        .windows(parity::PARITY_MAGIC.len())
        .rposition(|w| w == parity::PARITY_MAGIC)
    {
        search_end = off + parity::PARITY_MAGIC.len() - 1;
        let Some(copy) = parity::embedded_trailer_copy(&body[off..]) else {
            continue;
        };
        let Some(t) = Trailer::decode(copy) else {
            continue;
        };
        let expected_end = t.parity_off().checked_add(t.parity_len);
        if t.parity_off() == off as u64 && expected_end == Some(body.len() as u64) {
            return Some(t);
        }
    }
    None
}

fn read_trailers(map: &[u8]) -> (Option<Trailer>, u8) {
    let len = map.len();
    let mut found = None;
    let mut bad = 0u8;
    for k in 1..=TRAILER_COPIES {
        let end = len - TRAILER_LEN * (k - 1);
        match Trailer::decode(&map[end - TRAILER_LEN..end]) {
            Some(t) => {
                if found.is_none() {
                    found = Some(t);
                }
            }
            None => bad += 1,
        }
    }
    (found, bad)
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
