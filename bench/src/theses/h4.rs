//! H4 — Le cœur d'un montage lit à la vitesse du décodeur ; à travers le
//! noyau, le débit est borné par la latence par requête (ADR-0004). Référence : lecture
//! séquentielle directe par `vault-core` (requêtes d'1 MiB). Confrontés :
//! le cœur de `vault-mount` servi en requêtes de 128 KiB comme le noyau
//! (toujours mesuré, en un fil puis sous la concurrence du noyau), puis, si
//! l'hôte a un pilote, le montage réel lu par `std::fs` (contenu vérifié par
//! BLAKE3, débit au parallélisme disponible et en un fil, latence d'ouverture).
//!
//! La référence est mesurée **en alternance** avec chaque candidat (une
//! lecture directe juste avant chaque passe) : une mesure séparée dérive avec
//! la charge de l'hôte (0,53 à 0,85 observés pour le même cœur sur un poste
//! chargé). Le ratio rapporte la médiane des références alternées à la médiane
//! du candidat.
//!
//! Sans pilote, les métriques `requires: mount` sont sautées et la thèse est
//! `incomplete` : le banc ne conclut jamais sur un montage qu'il n'a pas fait.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{bail, Context as _, Result};
use vault_core::{Archive, ParityOptions};
use vault_mount::{mount, MountOptions, ReadStats, VaultFs};

use super::{pack_opts, Ctx, Produced};
use crate::corpus::{self, Files};
use crate::measure::{median, median_f64, percentile, samples_ns, time_ns};
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

/// Même parcours par le cœur, sous la concurrence du démon : `threads` fils
/// persistants consomment la file des requêtes de 128 KiB, fichier après
/// fichier. Reproduit le profil d'accès du pilote (requêtes voisines qui
/// manquent le même morceau au même instant) sans pilote. Renvoie
/// (ns, décodages ÷ morceaux de l'archive).
fn core_concurrent_ns(vault: &Path, threads: usize) -> (u64, f64) {
    let fs = VaultFs::open(vault).expect("VaultFs");
    // Toutes les requêtes de KERNEL_REQ octets, dans l'ordre des fichiers,
    // consommées par `threads` fils persistants : c'est le motif du démon FUSE
    // (fils de service fixes, requêtes du noyau en file).
    let reqs: Vec<(u64, u64)> = fs
        .archive()
        .files()
        .iter()
        .flat_map(|f| {
            let ino = fs.resolve(&f.path).expect("chemin présent").ino;
            (0..f.size).step_by(KERNEL_REQ).map(move |off| (ino, off))
        })
        .collect();
    let chunks = fs.archive().chunks().len() as f64;
    let next = AtomicUsize::new(0);
    let (ns, ()) = time_ns(|| {
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    let mut buf = vec![0u8; KERNEL_REQ];
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= reqs.len() {
                            break;
                        }
                        let (ino, off) = reqs[i];
                        fs.read(ino, off, &mut buf)
                            .expect("lecture cœur concurrente");
                    }
                });
            }
        });
    });
    (ns, fs.read_stats().cache.decodes as f64 / chunks)
}

fn parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Monte `vault` sur `mnt`, exécute `f`, démonte même si `f` échoue. Renvoie
/// le résultat de `f` et les compteurs de lecture du montage.
fn with_mount<T>(
    vault: &Path,
    mnt: &Path,
    opts: &MountOptions,
    f: impl FnOnce(&Path) -> Result<T>,
) -> Result<(T, ReadStats)> {
    std::fs::create_dir_all(mnt)?;
    let fs = VaultFs::open(vault)?;
    let handle = mount(fs, mnt, opts)
        .with_context(|| format!("montage de {} sur {}", vault.display(), mnt.display()))?;
    let r = f(mnt);
    let stats = handle.fs().read_stats();
    let un = handle.unmount().context("démontage");
    un.and(r).map(|t| (t, stats))
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

    // Référence D mesurée en alternance avec chaque candidat : REPS fois
    // (direct, cœur un fil, cœur concurrent), médianes.
    let archives: Vec<Archive> = corpora
        .iter()
        .map(|c| Archive::open(&c.vault))
        .collect::<Result<_, _>>()?;
    let direct_all = |archives: &[Archive]| -> u64 { archives.iter().map(direct_ns).sum() };
    let threads = parallelism();
    let mut direct = Vec::with_capacity(REPS);
    let mut core = Vec::with_capacity(REPS);
    let mut core_cc = Vec::with_capacity(REPS);
    let mut decode_ratio = Vec::with_capacity(REPS);
    for _ in 0..REPS {
        direct.push(direct_all(&archives));
        core.push(corpora.iter().map(|c| core_ns(&c.vault)).sum::<u64>());
        let (ns, dr): (Vec<u64>, Vec<f64>) = corpora
            .iter()
            .map(|c| core_concurrent_ns(&c.vault, threads))
            .unzip();
        core_cc.push(ns.iter().sum::<u64>());
        decode_ratio.push(dr.iter().sum::<f64>() / dr.len() as f64);
    }
    let d = median(&direct);
    let mut out = vec![
        Measured::scalar("h4.core_read_ratio", d as f64 / median(&core) as f64),
        Measured::scalar(
            "h4.core_concurrent_read_ratio",
            d as f64 / median(&core_cc) as f64,
        ),
        Measured::scalar("h4.core_concurrent_decode_ratio", median_f64(&decode_ratio)),
        Measured::scalar("h4.threads", threads as f64),
    ];

    if !ctx.env.has("mount") {
        eprintln!(
            "H4 : pas de montage réel ({})",
            ctx.env.missing_reason("mount")
        );
        return Ok(out);
    }

    // Montage réel : lecture par le noyau, contenu vérifié, MOUNT_PASSES passes,
    // chacune précédée d'une lecture directe (référence alternée) et doublée
    // d'un montage à un seul fil de service.
    let expected: Vec<Vec<blake3::Hash>> = corpora
        .iter()
        .map(|c| c.files.iter().map(|(_, d)| blake3::hash(d)).collect())
        .collect();
    let one_thread = MountOptions {
        threads: Some(1),
        ..MountOptions::default()
    };
    let mut direct_m = Vec::with_capacity(MOUNT_PASSES);
    let mut mount_ns = Vec::with_capacity(MOUNT_PASSES);
    // Par corpus : (directe, montage) pour situer la perte (gros fichiers en
    // flux contre petits fichiers dominés par les allers-retours par fichier).
    let mut per_corpus: Vec<(Vec<u64>, Vec<u64>)> = vec![(Vec::new(), Vec::new()); corpora.len()];
    let mut mount1_ns = Vec::with_capacity(MOUNT_PASSES);
    let mut req_kib = Vec::with_capacity(MOUNT_PASSES);
    let mut mount_decode = Vec::with_capacity(MOUNT_PASSES);
    let mut content_ok = true;
    for pass in 0..MOUNT_PASSES {
        let directs: Vec<u64> = archives.iter().map(direct_ns).collect();
        direct_m.push(directs.iter().sum());
        let (mut total, mut total1) = (0, 0);
        let mut stats = ReadStats::default();
        let mut chunks = 0u64;
        for (i, (c, exp)) in corpora.iter().zip(&expected).enumerate() {
            let mnt = ctx.work.join(format!("mnt-{}-{pass}", c.key));
            let ((ns, ok), st) = with_mount(&c.vault, &mnt, &MountOptions::default(), |m| {
                kernel_read(m, &c.files, exp)
            })?;
            total += ns;
            per_corpus[i].0.push(directs[i]);
            per_corpus[i].1.push(ns);
            content_ok &= ok;
            stats.calls += st.calls;
            stats.bytes += st.bytes;
            stats.cache.decodes += st.cache.decodes;
            chunks += Archive::open(&c.vault)?.chunks().len() as u64;
            let mnt1 = ctx.work.join(format!("mnt1-{}-{pass}", c.key));
            let ((ns1, ok1), _) = with_mount(&c.vault, &mnt1, &one_thread, |m| {
                kernel_read(m, &c.files, exp)
            })?;
            total1 += ns1;
            content_ok &= ok1;
        }
        mount_ns.push(total);
        mount1_ns.push(total1);
        req_kib.push(stats.bytes as f64 / stats.calls.max(1) as f64 / 1024.0);
        mount_decode.push(stats.cache.decodes as f64 / chunks.max(1) as f64);
    }
    drop(archives);
    let dm = median(&direct_m);
    out.push(Measured::scalar(
        "h4.mount_content_ok",
        if content_ok { 1.0 } else { 0.0 },
    ));
    out.push(Measured::scalar(
        "h4.mount_read_ratio",
        dm as f64 / median(&mount_ns) as f64,
    ));
    out.push(Measured::scalar(
        "h4.mount_read_ratio_1thread",
        dm as f64 / median(&mount1_ns) as f64,
    ));
    for (c, (d, m)) in corpora.iter().zip(&per_corpus) {
        out.push(Measured::scalar(
            &format!("h4.mount_read_ratio_{}", c.key),
            median(d) as f64 / median(m) as f64,
        ));
    }
    out.push(Measured::scalar("h4.mount_req_kib", median_f64(&req_kib)));
    out.push(Measured::scalar(
        "h4.mount_decode_ratio",
        median_f64(&mount_decode),
    ));

    // Latence d'ouverture (lookup + open par le noyau) sur le corpus small1.
    let small1 = corpus::gen_small(SEED_SMALL, ctx.profile.small_counts[0]);
    let dir1 = ctx.materialize("small1", SEED_SMALL, &small1)?;
    let (arc1, _) = ctx.archive("small1", &dir1, &opts)?;
    let n = (ctx.profile.lookup_samples / 10).max(100);
    let mnt = ctx.work.join("mnt-small1");
    let (open_ns, _) = with_mount(&arc1, &mnt, &MountOptions::default(), |m| {
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
