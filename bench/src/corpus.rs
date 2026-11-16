//! Corpus synthétique déterministe. Chaque catégorie est engendrée depuis une
//! seed fixe ; la matérialisation sur disque est idempotente (empreinte BLAKE3
//! posée à côté du répertoire, régénération seulement si elle diffère).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::prng::Xorshift;

pub type Files = Vec<(String, Vec<u8>)>;

const POOL: usize = 20_000;
const SYLLABLES: &[&str] = &[
    "ba", "co", "de", "fi", "ga", "ho", "in", "ju", "ka", "lo", "me", "nu", "or", "pa", "qu", "re",
    "si", "to", "ur", "ve", "wa", "xe", "yo", "zu", "ch", "st", "tr", "pl",
];
const KEYWORDS: &[&str] = &["let", "let mut", "const", "static"];
const TYPES: &[&str] = &[
    "u32",
    "u64",
    "i64",
    "f64",
    "bool",
    "String",
    "Vec<u8>",
    "Option<u32>",
];

fn identifier(rng: &mut Xorshift) -> String {
    let n = 2 + rng.below(3) as usize;
    let mut s = String::new();
    for i in 0..n {
        if i > 0 && rng.below(3) == 0 {
            s.push('_');
        }
        s.push_str(SYLLABLES[rng.below(SYLLABLES.len() as u64) as usize]);
    }
    s
}

/// Réservoir global d'identifiants, indépendant de la catégorie.
fn pool() -> Vec<String> {
    let mut rng = Xorshift::new(0xC0DE);
    (0..POOL).map(|_| identifier(&mut rng)).collect()
}

fn pick<'a>(rng: &mut Xorshift, items: &[&'a str]) -> &'a str {
    items[rng.below(items.len() as u64) as usize]
}

/// Un fichier « code » : vocabulaire local (~150 identifiants tirés du réservoir
/// par loi de Zipf), mots-clés, indentation, littéraux, commentaires.
pub fn code_file(rng: &mut Xorshift, pool: &[String], len: usize) -> Vec<u8> {
    let local: Vec<&str> = (0..150)
        .map(|_| pool[rng.zipf(pool.len())].as_str())
        .collect();
    let mut out = String::with_capacity(len + 128);
    let mut depth = 0usize;
    while out.len() < len {
        for _ in 0..depth {
            out.push_str("    ");
        }
        let id = |rng: &mut Xorshift| local[rng.zipf(local.len())];
        match rng.below(10) {
            0 => {
                out.push_str("fn ");
                out.push_str(id(rng));
                out.push('(');
                out.push_str(id(rng));
                out.push_str(": ");
                out.push_str(pick(rng, TYPES));
                out.push_str(") -> ");
                out.push_str(pick(rng, TYPES));
                out.push_str(" {");
                depth += 1;
            }
            1 if depth > 0 => {
                depth -= 1;
                out.truncate(out.len() - 4);
                out.push('}');
            }
            2 => {
                out.push_str("// ");
                for _ in 0..rng.below(8) + 1 {
                    out.push_str(id(rng));
                    out.push(' ');
                }
            }
            3 => {
                out.push_str("if ");
                out.push_str(id(rng));
                out.push_str(" > ");
                out.push_str(&rng.below(10_000).to_string());
                out.push_str(" {");
                depth += 1;
            }
            4 => {
                out.push_str("let ");
                out.push_str(id(rng));
                out.push_str(" = \"");
                out.push_str(id(rng));
                out.push_str("\".to_string();");
            }
            _ => {
                out.push_str(pick(rng, KEYWORDS));
                out.push(' ');
                out.push_str(id(rng));
                out.push_str(" = ");
                out.push_str(id(rng));
                out.push('(');
                out.push_str(id(rng));
                out.push_str(", ");
                out.push_str(&rng.below(1 << 16).to_string());
                out.push_str(");");
            }
        }
        out.push('\n');
    }
    out.truncate(len);
    out.into_bytes()
}

/// Un fichier « binaire » : 60 % de bruit, 25 % d'enregistrements à petits
/// deltas, 15 % de motif répété légèrement altéré.
pub fn binary_file(rng: &mut Xorshift, len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    let noise = len * 60 / 100;
    let records = len * 25 / 100;
    rng.fill(&mut out[..noise]);
    let mut rec = [0u64; 4];
    for row in out[noise..noise + records].chunks_mut(32) {
        for (i, f) in rec.iter_mut().enumerate() {
            *f = f.wrapping_add(rng.below(16) + i as u64);
        }
        for (i, f) in rec.iter().enumerate() {
            let b = f.to_le_bytes();
            let n = row.len().saturating_sub(i * 8).min(8);
            if n > 0 {
                row[i * 8..i * 8 + n].copy_from_slice(&b[..n]);
            }
        }
    }
    let motif_len = 1024 + rng.below(3072) as usize;
    let mut motif = vec![0u8; motif_len];
    rng.fill(&mut motif);
    for block in out[noise + records..].chunks_mut(motif_len) {
        block.copy_from_slice(&motif[..block.len()]);
        if rng.below(4) == 0 && !block.is_empty() {
            let i = rng.below(block.len() as u64) as usize;
            block[i] ^= 0x5a;
        }
    }
    out
}

fn sized_files(
    seed: u64,
    total: u64,
    lo: u64,
    hi: u64,
    ext: &str,
    mut gen: impl FnMut(&mut Xorshift, usize) -> Vec<u8>,
) -> Files {
    let mut rng = Xorshift::new(seed);
    let mut files = Vec::new();
    let mut acc = 0u64;
    let mut i = 0;
    while acc < total {
        let len = rng.log_uniform(lo, hi).min(total - acc) as usize;
        let data = gen(&mut rng, len);
        files.push((format!("d{:02}/f{i:06}.{ext}", i % 64), data));
        acc += len as u64;
        i += 1;
    }
    files
}

pub fn gen_code(seed: u64, total: u64) -> Files {
    let pool = pool();
    sized_files(seed, total, 1024, 64 * 1024, "rs", |rng, len| {
        code_file(rng, &pool, len)
    })
}

pub fn gen_binary(seed: u64, total: u64) -> Files {
    gen_binary_range(seed, total, 4 * 1024, 2 * 1024 * 1024)
}

/// Fichiers binaires dont la taille est tirée log-uniformément dans `[lo, hi]`.
pub fn gen_binary_range(seed: u64, total: u64, lo: u64, hi: u64) -> Files {
    sized_files(seed, total, lo, hi, "bin", binary_file)
}

/// `count` petits fichiers (128 o à 4 KiB, contenu code) dans un arbre à 32 branches.
pub fn gen_small(seed: u64, count: u64) -> Files {
    let pool = pool();
    let mut rng = Xorshift::new(seed);
    (0..count)
        .map(|i| {
            let len = rng.log_uniform(128, 4096) as usize;
            let path = format!("b{:02}/c{:02}/s{i:06}.rs", i % 32, (i / 32) % 32);
            (path, code_file(&mut rng, &pool, len))
        })
        .collect()
}

/// Renvoie la copie et le nombre d'éditions appliquées.
///
/// Copie de `data` portant au moins une insertion ou suppression de 1–64 octets
/// (la première dans le premier MiB), puis une toutes les 512 KiB–1,5 MiB : le
/// découpage défini par le contenu doit se resynchroniser après chaque édition.
pub fn edited_copy(rng: &mut Xorshift, data: &[u8]) -> (Vec<u8>, u64) {
    if data.is_empty() {
        return (Vec::new(), 0);
    }
    let mut out = Vec::with_capacity(data.len() + 256);
    let mut edits = 0u64;
    let mut pos = 0usize;
    let mut step = rng.below(data.len().min(1 << 20) as u64) as usize;
    loop {
        let take = step.min(data.len() - pos);
        out.extend_from_slice(&data[pos..pos + take]);
        pos += take;
        if pos >= data.len() {
            break;
        }
        let n = 1 + rng.below(64) as usize;
        if rng.below(2) == 0 {
            let mut ins = vec![0u8; n];
            rng.fill(&mut ins);
            out.extend_from_slice(&ins);
        } else {
            pos = (pos + n).min(data.len());
        }
        edits += 1;
        step = 512 * 1024 + rng.below(1024 * 1024) as usize;
    }
    (out, edits)
}

#[allow(dead_code)] // `base_bytes` documente la composition ; lu par les tests.
pub struct DupCorpus {
    pub files: Files,
    pub base_bytes: u64,
    pub exact_bytes: u64,
    pub edited_bytes: u64,
    /// Nombre total d'éditions appliquées aux copies éditées.
    pub edits: u64,
}

/// Base mixte (code + binaires de 1 à 8 MiB) plus copies exactes (de tout
/// fichier) et copies éditées (des binaires seulement : une édition dans un
/// fichier plus petit qu'un morceau invalide tout le fichier, ce que la thèse
/// ne prétend pas éviter) jusqu'à ce que `rate_pct` % des octets du corpus
/// soient des doublons (moitié exactes, moitié éditées).
pub fn gen_dup(seed: u64, base_bytes: u64, rate_pct: u64) -> DupCorpus {
    let mut files = gen_code(seed, base_bytes / 2);
    let n_code = files.len();
    // Binaires de taille proportionnelle à la base (bornés 256 KiB – 8 MiB) :
    // assez gros pour porter plusieurs morceaux, assez petits pour que le budget
    // de doublons soit respecté à un fichier de code près.
    let hi = (base_bytes / 8).clamp(256 << 10, 8 << 20);
    let lo = (hi / 8).max(64 << 10);
    files.extend(
        gen_binary_range(seed + 1, base_bytes / 2, lo, hi)
            .into_iter()
            .map(|(p, d)| (format!("bin/{p}"), d)),
    );
    let base_total: u64 = files.iter().map(|f| f.1.len() as u64).sum();
    // d = dup / (base + dup)  =>  dup = base * d / (100 - d)
    let target_dup = if rate_pct == 0 {
        0
    } else {
        base_total * rate_pct / (100 - rate_pct)
    };
    let mut rng = Xorshift::new(seed + 2);
    let (mut exact, mut edited, mut edits) = (0u64, 0u64, 0u64);
    let mut extra = Vec::new();
    let mut k = 0usize;
    while exact + edited < target_dup {
        let remaining = target_dup - exact - edited;
        // k pair : copie exacte de n'importe quel fichier ; k impair : copie
        // éditée d'un binaire. Seuls les fichiers qui tiennent dans le budget
        // restant sont candidats ; sinon un fichier de code (≤ 64 KiB) complète.
        let from = if k % 2 == 0 { 0 } else { n_code };
        let fitting: Vec<usize> = (from..files.len())
            .filter(|&i| files[i].1.len() as u64 <= remaining)
            .collect();
        let (i, edit) = if fitting.is_empty() {
            (rng.below(n_code as u64) as usize, false)
        } else {
            (
                fitting[rng.below(fitting.len() as u64) as usize],
                k % 2 == 1,
            )
        };
        let (p, d) = &files[i];
        let flat = p.replace('/', "_");
        if edit {
            let (e, n) = edited_copy(&mut rng, d);
            edited += e.len() as u64;
            edits += n;
            extra.push((format!("dup/edited/{k:05}-{flat}"), e));
        } else {
            exact += d.len() as u64;
            extra.push((format!("dup/exact/{k:05}-{flat}"), d.clone()));
        }
        k += 1;
    }
    files.extend(extra);
    DupCorpus {
        files,
        base_bytes: base_total,
        exact_bytes: exact,
        edited_bytes: edited,
        edits,
    }
}

/// Un seul fichier mixte pour la parité.
pub fn gen_parity(seed: u64, bytes: u64) -> Files {
    let pool = pool();
    let mut rng = Xorshift::new(seed);
    let mut data = binary_file(&mut rng, (bytes / 2) as usize);
    data.extend(code_file(&mut rng, &pool, (bytes - bytes / 2) as usize));
    vec![("mixte.bin".to_string(), data)]
}

pub fn digest(files: &Files) -> String {
    let mut h = blake3::Hasher::new();
    for (p, d) in files {
        h.update(p.as_bytes());
        h.update(&(d.len() as u64).to_le_bytes());
        h.update(d);
    }
    h.finalize().to_hex().to_string()
}

pub fn total_bytes(files: &Files) -> u64 {
    files.iter().map(|f| f.1.len() as u64).sum()
}

/// Écrit la catégorie sous `root/id` si son empreinte diffère de celle posée
/// dans `root/id.digest`. Retourne le répertoire et l'empreinte.
pub fn materialize(root: &Path, id: &str, files: &Files) -> Result<(PathBuf, String)> {
    let dir = root.join(id);
    let digest_path = root.join(format!("{id}.digest"));
    let d = digest(files);
    let same = fs::read_to_string(&digest_path)
        .map(|s| s.trim() == d)
        .unwrap_or(false);
    if same && dir.is_dir() {
        return Ok((dir, d));
    }
    if dir.exists() {
        fs::remove_dir_all(&dir).with_context(|| dir.display().to_string())?;
    }
    for (rel, data) in files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&p, data).with_context(|| p.display().to_string())?;
    }
    fs::create_dir_all(root)?;
    fs::write(&digest_path, format!("{d}\n"))?;
    Ok((dir, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_sized() {
        let a = gen_code(1, 256 * 1024);
        let b = gen_code(1, 256 * 1024);
        assert_eq!(digest(&a), digest(&b));
        assert_eq!(total_bytes(&a), 256 * 1024);
        assert_ne!(digest(&a), digest(&gen_code(2, 256 * 1024)));
    }

    #[test]
    fn dup_rate_is_respected() {
        for base in [8u64 << 20, 32 << 20] {
            let c = gen_dup(1, base, 20);
            let total = total_bytes(&c.files);
            let dup = c.exact_bytes + c.edited_bytes;
            let rate = dup as f64 / total as f64 * 100.0;
            assert!((18.0..22.0).contains(&rate), "{base}: {rate}");
            assert!(c.edited_bytes > 0, "{base}: aucune copie éditée");
            assert!(c.files.iter().any(|(p, _)| p.starts_with("dup/edited/")));
        }
    }

    #[test]
    fn edited_copy_resyncs() {
        let mut rng = Xorshift::new(9);
        let data = binary_file(&mut rng, 4 << 20);
        let (e, n) = edited_copy(&mut rng, &data);
        assert!(n >= 1);
        assert_ne!(e, data);
        assert!((e.len() as i64 - data.len() as i64).abs() < 2048);
    }

    #[test]
    fn materialize_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let files = gen_small(3, 20);
        let (dir, d1) = materialize(tmp.path(), "small", &files).unwrap();
        let mtime = fs::metadata(dir.join(&files[0].0))
            .unwrap()
            .modified()
            .unwrap();
        let (_, d2) = materialize(tmp.path(), "small", &files).unwrap();
        assert_eq!(d1, d2);
        assert_eq!(
            mtime,
            fs::metadata(dir.join(&files[0].0))
                .unwrap()
                .modified()
                .unwrap()
        );
    }
}
