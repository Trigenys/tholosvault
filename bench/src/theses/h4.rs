//! H4 — Un montage lit à la vitesse du décodeur. Référence : lecture
//! séquentielle directe par `vault-core` (requêtes d'1 MiB). Confrontés :
//! le cœur de `vault-mount` servi en requêtes de 128 KiB comme le noyau
//! (toujours mesuré), puis, si l'hôte a un pilote, le montage réel lu par
//! `std::fs` (contenu vérifié par BLAKE3, latence d'ouverture).
//!
//! Sans pilote, les métriques `requires: mount` sont sautées et la thèse est
//! `incomplete` : le banc ne conclut jamais sur un montage qu'il n'a pas fait.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context as _, Result};
use vault_core::{Archive, ParityOptions};
use vault_mount::{mount, MountOptions, VaultFs};

use super::{pack_opts, Ctx, Produced};
use crate::corpus::{self, Files};
use crate::measure::{median, percentile, samples_ns, time_ns};
use crate::prng::Xorshift;
use crate::report::Measured;
use crate::theses::h1::{LEVEL, SEED_BINARY, SEED_CODE};
use crate::theses::h3::SEED_SMALL;

/// Taille des requêtes de la lecture directe de référence.
const DIRECT: usize = 1 << 20;
/// Taille des requêtes du noyau Linux par défaut (max_read FUSE).
const KERNEL_REQ: usize = 128 << 10;
/// Répétitions pour la lecture directe et la lecture par le cœur (médiane).
const REPS: usize = 5;
/// Montages successifs pour la lecture à travers le noyau (médiane).
const MOUNT_PASSES: usize = 3;

struct Corpus {
    key: &'static str,
    vault: PathBuf,
    files: Files,
}

/// Lecture séquentielle de tous les fichiers par `vault-core`, ns.
fn direct_ns(ar: &Archive) -> u64 {
    let mut buf = vec![0u8; DIRECT];
    let (ns, ()) = time_ns(|| {
        for (i, f) in ar.files().iter().enumerate() {
            let mut off = 0;
            while off < f.size {
                let n = ar
                    .read_range_into(i, off, &mut buf)
                    .expect("lecture directe");
                if n == 0 {
                    break;
                }
                off += n as u64;
            }
        }
    });
    ns
}

/// Même parcours par le cœur de `vault-mount`, cache de morceaux froid, en
/// requêtes de 128 KiB comme le noyau les découpe. L'ouverture (arbre) est
/// hors mesure : elle a lieu une fois au montage, pas à chaque lecture.
fn core_ns(vault: &Path) -> u64 {
    let mut buf = vec![0u8; KERNEL_REQ];
    let fs = VaultFs::open(vault).expect("VaultFs");
    let (ns, ()) = time_ns(|| {
        for f in fs.archive().files() {
            let attr = fs.resolve(&f.path).expect("chemin présent");
            let mut off = 0;
            while off < attr.size {
                let n = fs.read(attr.ino, off, &mut buf).expect("lecture cœur");
                if n == 0 {
                    break;
                }
                off += n as u64;
            }
        }
    });
    ns
}

/// Monte `vault` sur `mnt`, exécute `f`, démonte même si `f` échoue.
fn with_mount<T>(vault: &Path, mnt: &Path, f: impl FnOnce(&Path) -> Result<T>) -> Result<T> {
    std::fs::create_dir_all(mnt)?;
    let fs = VaultFs::open(vault)?;
    let handle = mount(fs, mnt, &MountOptions::default())
        .with_context(|| format!("montage de {} sur {}", vault.display(), mnt.display()))?;
    let r = f(mnt);
    let un = handle.unmount().context("démontage");
    un.and(r)
}

/// Lit chaque fichier du montage par `std::fs` (1 MiB par requête) et compare
/// son BLAKE3 au corpus. Renvoie (ns, tout le contenu est exact).
fn kernel_read(mnt: &Path, files: &Files, expected: &[blake3::Hash]) -> Result<(u64, bool)> {
    let mut buf = vec![0u8; DIRECT];
    let mut ok = true;
    let (ns, r) = time_ns(|| -> Result<()> {
        for ((rel, _), want) in files.iter().zip(expected) {
            let mut f = File::open(mnt.join(rel)).with_context(|| rel.clone())?;
            let mut h = blake3::Hasher::new();
            loop {
                let n = f.read(&mut buf).with_context(|| rel.clone())?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
            ok &= h.finalize() == *want;
        }
        Ok(())
    });
    r?;
    Ok((ns, ok))
}

pub fn run(ctx: &mut Ctx) -> Result<Produced> {
    let opts = pack_opts(LEVEL, ParityOptions::disabled());
    let mut corpora = Vec::new();
    for (key, seed, gen) in [
        ("code", SEED_CODE, corpus::gen_code as fn(u64, u64) -> Files),
        ("binary", SEED_BINARY, corpus::gen_binary),
    ] {
        let bytes = match key {
            "code" => ctx.profile.code_bytes,
            _ => ctx.profile.binary_bytes,
        };
        let files = gen(seed, bytes);
        let dir = ctx.materialize(key, seed, &files)?;
        let (vault, _) = ctx.archive(key, &dir, &opts)?;
        corpora.push(Corpus { key, vault, files });
    }

    // Référence D et cœur, médiane de REPS parcours complets (les deux corpus).
    let archives: Vec<Archive> = corpora
        .iter()
        .map(|c| Archive::open(&c.vault))
        .collect::<Result<_, _>>()?;
    let direct: Vec<u64> = (0..REPS)
        .map(|_| archives.iter().map(direct_ns).sum())
        .collect();
    let core: Vec<u64> = (0..REPS)
        .map(|_| corpora.iter().map(|c| core_ns(&c.vault)).sum())
        .collect();
    drop(archives);
    let d = median(&direct);
    let mut out = vec![Measured::scalar(
        "h4.core_read_ratio",
        d as f64 / median(&core) as f64,
    )];

    if !ctx.env.has("mount") {
        eprintln!(
            "H4 : pas de montage réel ({})",
            ctx.env.missing_reason("mount")
        );
        return Ok(out);
    }

    // Montage réel : lecture par le noyau, contenu vérifié, plusieurs passes.
    let expected: Vec<Vec<blake3::Hash>> = corpora
        .iter()
        .map(|c| c.files.iter().map(|(_, d)| blake3::hash(d)).collect())
        .collect();
    let mut mount_ns = Vec::with_capacity(MOUNT_PASSES);
    let mut content_ok = true;
    for pass in 0..MOUNT_PASSES {
        let mut total = 0;
        for (c, exp) in corpora.iter().zip(&expected) {
            let mnt = ctx.work.join(format!("mnt-{}-{pass}", c.key));
            let (ns, ok) = with_mount(&c.vault, &mnt, |m| kernel_read(m, &c.files, exp))?;
            total += ns;
            content_ok &= ok;
        }
        mount_ns.push(total);
    }
    out.push(Measured::scalar(
        "h4.mount_content_ok",
        if content_ok { 1.0 } else { 0.0 },
    ));
    out.push(Measured::scalar(
        "h4.mount_read_ratio",
        d as f64 / median(&mount_ns) as f64,
    ));

    // Latence d'ouverture (lookup + open par le noyau) sur le corpus small1.
    let small1 = corpus::gen_small(SEED_SMALL, ctx.profile.small_counts[0]);
    let dir1 = ctx.materialize("small1", SEED_SMALL, &small1)?;
    let (arc1, _) = ctx.archive("small1", &dir1, &opts)?;
    let n = (ctx.profile.lookup_samples / 10).max(100);
    let mnt = ctx.work.join("mnt-small1");
    let open_ns = with_mount(&arc1, &mnt, |m| {
        let mut rng = Xorshift::new(SEED_SMALL + 4);
        Ok(samples_ns(n, || {
            let rel = &small1[rng.below(small1.len() as u64) as usize].0;
            File::open(m.join(rel)).expect("ouverture par le noyau")
        }))
    })?;
    if open_ns.is_empty() {
        bail!("aucun échantillon d'ouverture");
    }
    ctx.record_samples("h4.mount_open_ns", &open_ns);
    let p99 = percentile(&open_ns, 99.0);
    out.push(Measured::from_samples(
        "h4.mount_open_ns",
        &open_ns,
        p99 as f64,
    ));
    out.push(Measured::scalar(
        "h4.mount_open_p99_over_blake3_mib",
        p99 as f64 / ctx.calib_ns_per_mib as f64,
    ));
    Ok(out)
}
