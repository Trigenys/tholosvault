//! Construction d'une archive à partir d'un répertoire.
//!
//! Déterminisme (L-001) : fichiers parcourus en ordre bytewise de chemin relatif,
//! aucun horodatage ni métadonnée hôte, zstd mono-thread à niveau fixe. Deux
//! constructions du même arbre produisent des octets identiques.
//!
//! Atomicité : l'archive est écrite dans `<dst>.tholos-tmp` puis renommée sur
//! `dst` une fois complète. Un échec ou une interruption laisse l'archive
//! précédente intacte, jamais un fichier tronqué sous le nom final. La cible et
//! son fichier temporaire sont exclus de la source quand ils s'y trouvent :
//! relancer `pack` sur le même arbre redonne la même racine.

use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::chunk::{self, ChunkParams};
use crate::codec;
use crate::error::{Error, Result};
use crate::format::{self, Header, Trailer, HEADER_LEN, MAX_PATH_LEN};
use crate::merkle::{self, Hash};
use crate::parity::{self, ParityOptions};

#[derive(Debug, Clone)]
pub struct PackOptions {
    pub level: i32,
    pub chunk: ChunkParams,
    /// Parité Reed-Solomon (v1) ; `percent: 0` n'écrit aucune section.
    pub parity: ParityOptions,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            level: codec::DEFAULT_LEVEL,
            chunk: ChunkParams::default(),
            parity: ParityOptions::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackSummary {
    pub files: u64,
    pub input_bytes: u64,
    pub unique_chunks: u64,
    pub total_chunk_refs: u64,
    pub compressed_bytes: u64,
    pub archive_bytes: u64,
    /// Taille de la section de parité (0 si désactivée).
    pub parity_bytes: u64,
    pub archive_root: Hash,
}

struct ChunkRecord {
    offset: u64,
    clen: u32,
    ulen: u32,
    hash: Hash,
}

struct FileRecord {
    path: String,
    size: u64,
    root: Hash,
    chunks: Vec<u32>,
}

/// Suffixe du fichier temporaire écrit à côté de la cible.
pub const TMP_SUFFIX: &str = ".tholos-tmp";

/// Chemin du fichier temporaire de construction de `dst`.
pub fn tmp_path(dst: &Path) -> PathBuf {
    let mut name = dst.file_name().unwrap_or_default().to_os_string();
    name.push(TMP_SUFFIX);
    dst.with_file_name(name)
}

/// Archive le contenu de `src_dir` (fichiers réguliers uniquement, récursif)
/// dans le fichier `dst`, remplacé atomiquement s'il existe (voir l'en-tête
/// du module). `dst` et son temporaire ne sont jamais archivés eux-mêmes.
pub fn pack_dir(src_dir: &Path, dst: &Path, opts: &PackOptions) -> Result<PackSummary> {
    opts.chunk.validate()?;
    opts.parity.validate()?;
    let tmp = tmp_path(dst);
    let mut paths = collect_files(src_dir)?;
    exclude_outputs(&mut paths, &[dst, &tmp]);

    // Lecture + écriture : la zone protégée est relue par mmap pour la parité.
    let out = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)
        .map_err(|e| Error::io(&tmp, e))?;
    match write_archive(&paths, out, &tmp, opts) {
        Ok(summary) => {
            // Le temporaire est fermé (et sa vue mmap libérée, I-001) avant le
            // renommage, qui remplace `dst` d'un seul coup.
            fs::rename(&tmp, dst).map_err(|e| {
                let _ = fs::remove_file(&tmp);
                Error::io(dst, e)
            })?;
            Ok(summary)
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Retire de la liste les fichiers qui sont l'une des sorties (même fichier,
/// quel que soit le chemin employé pour le désigner).
fn exclude_outputs(paths: &mut Vec<(String, PathBuf)>, outputs: &[&Path]) {
    let targets: Vec<(std::ffi::OsString, PathBuf)> = outputs
        .iter()
        .filter_map(|o| Some((o.file_name()?.to_os_string(), canonical_target(o)?)))
        .collect();
    if targets.is_empty() {
        return;
    }
    paths.retain(|(_, abs)| {
        let Some(name) = abs.file_name() else {
            return true;
        };
        // Canonicalisation réservée aux homonymes : coût nul sur un arbre courant.
        !targets.iter().any(|(tname, tcanon)| {
            tname == name && fs::canonicalize(abs).is_ok_and(|c| &c == tcanon)
        })
    });
}

/// Chemin canonique d'une sortie qui n'existe pas forcément encore : parent
/// canonique + nom.
fn canonical_target(p: &Path) -> Option<PathBuf> {
    let name = p.file_name()?;
    let parent = match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    Some(fs::canonicalize(parent).ok()?.join(name))
}

fn write_archive(
    paths: &[(String, PathBuf)],
    out: fs::File,
    dst: &Path,
    opts: &PackOptions,
) -> Result<PackSummary> {
    let mut w = BufWriter::new(out);

    let header = Header {
        format_version: format::FORMAT_VERSION,
        flags: 0,
        codec_id: codec::CODEC_ZSTD,
        codec_level: opts.level as i8,
        codec_version: codec::zstd_version(),
        chunk_min: opts.chunk.min,
        chunk_avg: opts.chunk.avg,
        chunk_max: opts.chunk.max,
        dict_id: [0u8; 32],
    };
    w.write_all(&header.encode())
        .map_err(|e| Error::io(dst, e))?;

    let mut chunks: Vec<ChunkRecord> = Vec::new();
    let mut by_hash: HashMap<Hash, u32> = HashMap::new();
    let mut files: Vec<FileRecord> = Vec::with_capacity(paths.len());
    let mut data_len: u64 = 0;
    let mut input_bytes: u64 = 0;
    let mut total_refs: u64 = 0;

    for (rel, abs) in paths {
        let data = fs::read(abs).map_err(|e| Error::io(abs, e))?;
        input_bytes += data.len() as u64;
        let mut refs = Vec::new();
        let mut leaves = Vec::new();
        for range in chunk::split(&data, opts.chunk) {
            let slice = &data[range];
            let hash = merkle::chunk_id(slice);
            let idx = match by_hash.get(&hash) {
                Some(&i) => i,
                None => {
                    let frame = codec::compress(slice, opts.level)?;
                    w.write_all(&frame).map_err(|e| Error::io(dst, e))?;
                    let idx = chunks.len() as u32;
                    chunks.push(ChunkRecord {
                        offset: HEADER_LEN as u64 + data_len,
                        clen: frame.len() as u32,
                        ulen: slice.len() as u32,
                        hash,
                    });
                    data_len += frame.len() as u64;
                    by_hash.insert(hash, idx);
                    idx
                }
            };
            refs.push(idx);
            leaves.push(hash);
        }
        total_refs += refs.len() as u64;
        files.push(FileRecord {
            path: rel.clone(),
            size: data.len() as u64,
            root: merkle::merkle_root(&leaves),
            chunks: refs,
        });
    }

    // Index des morceaux (ordre d'écriture = ordre d'offset).
    let chunk_index_off = HEADER_LEN as u64 + data_len;
    let mut buf = Vec::with_capacity(chunks.len() * format::CHUNK_ENTRY_LEN);
    for c in &chunks {
        buf.extend_from_slice(&c.offset.to_le_bytes());
        buf.extend_from_slice(&c.clen.to_le_bytes());
        buf.extend_from_slice(&c.ulen.to_le_bytes());
        buf.extend_from_slice(&c.hash);
    }
    w.write_all(&buf).map_err(|e| Error::io(dst, e))?;
    let chunk_index_len = buf.len() as u64;

    // Table des fichiers.
    let file_table_off = chunk_index_off + chunk_index_len;
    buf.clear();
    let mut file_leaves = Vec::with_capacity(files.len());
    for f in &files {
        buf.extend_from_slice(&(f.path.len() as u16).to_le_bytes());
        buf.extend_from_slice(f.path.as_bytes());
        buf.extend_from_slice(&f.size.to_le_bytes());
        buf.extend_from_slice(&f.root);
        buf.extend_from_slice(&(f.chunks.len() as u32).to_le_bytes());
        for idx in &f.chunks {
            buf.extend_from_slice(&idx.to_le_bytes());
        }
        file_leaves.push(merkle::file_leaf(&f.path, f.size, &f.root));
    }
    w.write_all(&buf).map_err(|e| Error::io(dst, e))?;
    let file_table_len = buf.len() as u64;

    // Table de hachage triée (hash → index de morceau).
    let hash_table_off = file_table_off + file_table_len;
    let mut sorted: Vec<(Hash, u32)> = chunks.iter().map(|c| c.hash).zip(0u32..).collect();
    sorted.sort_unstable_by_key(|a| a.0);
    buf.clear();
    for (hash, idx) in &sorted {
        buf.extend_from_slice(hash);
        buf.extend_from_slice(&idx.to_le_bytes());
        buf.extend_from_slice(&[0u8; 4]);
    }
    w.write_all(&buf).map_err(|e| Error::io(dst, e))?;
    let hash_table_len = buf.len() as u64;

    // Parité (v1) : calculée sur tout ce qui précède, octets tels qu'écrits.
    // La longueur de la section ne dépend que de la géométrie, ce qui permet
    // d'encoder le trailer avant de la construire et d'en embarquer une copie.
    let protected_len = hash_table_off + hash_table_len;
    let parity_len = if opts.parity.is_enabled() {
        parity::Geometry::new(protected_len, &opts.parity).section_len()
    } else {
        0
    };

    let archive_root = merkle::merkle_root(&file_leaves);
    let trailer = Trailer {
        chunk_index_off,
        chunk_index_len,
        file_table_off,
        file_table_len,
        hash_table_off,
        hash_table_len,
        chunk_count: chunks.len() as u64,
        file_count: files.len() as u64,
        archive_root,
        data_len,
        parity_len,
    }
    .encode();

    if opts.parity.is_enabled() {
        w.flush().map_err(|e| Error::io(dst, e))?;
        let section = {
            // Sûreté : fichier ouvert par nous seuls, aucune écriture pendant la vue.
            let map = unsafe { memmap2::Mmap::map(w.get_ref()) }.map_err(|e| Error::io(dst, e))?;
            parity::build_section(&map[..protected_len as usize], &opts.parity, Some(&trailer))?
        };
        debug_assert_eq!(section.len() as u64, parity_len);
        w.write_all(&section).map_err(|e| Error::io(dst, e))?;
    }

    for _ in 0..format::TRAILER_COPIES {
        w.write_all(&trailer).map_err(|e| Error::io(dst, e))?;
    }
    w.flush().map_err(|e| Error::io(dst, e))?;

    let archive_bytes =
        protected_len + parity_len + (format::TRAILER_LEN * format::TRAILER_COPIES) as u64;
    Ok(PackSummary {
        files: files.len() as u64,
        input_bytes,
        unique_chunks: chunks.len() as u64,
        total_chunk_refs: total_refs,
        compressed_bytes: data_len,
        archive_bytes,
        parity_bytes: parity_len,
        archive_root,
    })
}

/// Liste récursive des fichiers réguliers, chemins relatifs normalisés avec `/`,
/// triés bytewise. Les liens symboliques et autres entrées sont ignorés.
fn collect_files(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))?;
        for entry in entries {
            let entry = entry.map_err(|e| Error::io(&dir, e))?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| Error::io(&path, e))?;
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                let rel = relative_utf8(root, &path)?;
                out.push((rel, path));
            }
        }
    }
    out.sort_unstable_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Ok(out)
}

fn relative_utf8(root: &Path, path: &Path) -> Result<String> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let mut parts = Vec::new();
    for comp in rel.components() {
        let s = comp
            .as_os_str()
            .to_str()
            .ok_or_else(|| Error::NonUtf8Path(path.to_path_buf()))?;
        parts.push(s);
    }
    let joined = parts.join("/");
    if joined.len() > MAX_PATH_LEN {
        return Err(Error::PathTooLong {
            len: joined.len(),
            max: MAX_PATH_LEN,
            path: joined,
        });
    }
    Ok(joined)
}
