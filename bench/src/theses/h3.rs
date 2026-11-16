//! H3 — Montage et accès aléatoire : métadonnées lisibles sans décompression,
//! ouverture quasi linéaire, recherche de chemin et lecture de 4 KiB bornées.

use std::fs;

use anyhow::{Context as _, Result};
use vault_core::{Archive, ParityOptions};

use super::{pack_opts, Ctx, Produced};
use crate::corpus;
use crate::measure::{median_of, percentile, samples_ns};
use crate::prng::Xorshift;
use crate::report::Measured;
use crate::theses::h1::{LEVEL, SEED_BINARY};

pub const SEED_SMALL: u64 = 31;
const READ: usize = 4096;

/// Copie de l'archive dont toutes les frames de morceaux sont mises à zéro :
/// seules les tables doivent suffire à lister et retrouver les fichiers.
fn metadata_plaintext(
    ctx: &Ctx,
    archive: &std::path::Path,
    expect_files: usize,
    probe: &str,
) -> Result<f64> {
    let mut bytes = fs::read(archive)?;
    {
        let a = Archive::open(archive)?;
        for c in a.chunks() {
            let s = c.offset as usize;
            bytes[s..s + c.compressed_len as usize].fill(0);
        }
    }
    let zeroed = ctx.work.join("small1-zeroed.vault");
    fs::write(&zeroed, &bytes)?;
    let ok = match Archive::open(&zeroed) {
        Ok(a) => a.files().len() == expect_files && a.find_index(probe).is_some(),
        Err(_) => false,
    };
    Ok(if ok { 1.0 } else { 0.0 })
}

pub fn run(ctx: &mut Ctx) -> Result<Produced> {
    let mut out = Vec::new();
    let opts = pack_opts(LEVEL, ParityOptions::disabled());
    let [n1, n10] = ctx.profile.small_counts;

    let small1 = corpus::gen_small(SEED_SMALL, n1);
    let small10 = corpus::gen_small(SEED_SMALL + 1, n10);
    let dir1 = ctx.materialize("small1", SEED_SMALL, &small1)?;
    let dir10 = ctx.materialize("small10", SEED_SMALL + 1, &small10)?;
    let (arc1, _) = ctx.archive("small1", &dir1, &opts)?;
    let (arc10, _) = ctx.archive("small10", &dir10, &opts)?;

    out.push(Measured::scalar(
        "h3.metadata_plaintext",
        metadata_plaintext(ctx, &arc1, small1.len(), &small1[0].0)?,
    ));

    let open1 = median_of(5, || Archive::open(&arc1).expect("open small1"));
    let open10 = median_of(5, || Archive::open(&arc10).expect("open small10"));
    out.push(Measured::scalar(
        "h3.open_ns_per_file",
        open1 as f64 / n1 as f64,
    ));
    out.push(Measured::scalar(
        "h3.open_scaling",
        open10 as f64 / open1 as f64,
    ));

    let a10 = Archive::open(&arc10)?;
    let mut rng = Xorshift::new(SEED_SMALL + 7);
    let n = ctx.profile.lookup_samples;
    let find_ns = samples_ns(n, || {
        let p = &small10[rng.below(small10.len() as u64) as usize].0;
        a10.find_index(p).expect("chemin présent")
    });
    ctx.record_samples("h3.find_ns", &find_ns);
    let p99 = percentile(&find_ns, 99.0);
    out.push(Measured::from_samples("h3.find_ns", &find_ns, p99 as f64));
    out.push(Measured::scalar(
        "h3.find_p99_over_blake3_mib",
        p99 as f64 / ctx.calib_ns_per_mib as f64,
    ));

    let binary = corpus::gen_binary(SEED_BINARY, ctx.profile.binary_bytes);
    let dirb = ctx.materialize("binary", SEED_BINARY, &binary)?;
    let (arcb, _) = ctx.archive("binary", &dirb, &opts)?;
    let ab = Archive::open(&arcb)?;
    let files = ab.files();
    let mut buf = vec![0u8; READ];
    let mut max_span = 0usize;
    let mut rng = Xorshift::new(SEED_BINARY + 7);
    let read_ns = samples_ns(n, || {
        let f = rng.below(files.len() as u64) as usize;
        let size = files[f].size;
        let off = if size > READ as u64 {
            (rng.below(size - READ as u64) / READ as u64) * READ as u64
        } else {
            0
        };
        let span = ab.chunk_span(f, off, READ).expect("span");
        max_span = max_span.max(span.len());
        ab.read_range_into(f, off, &mut buf)
            .context("read_range_into")
            .expect("lecture")
    });
    ctx.record_samples("h3.read4k_ns", &read_ns);
    let p99r = percentile(&read_ns, 99.0);
    out.push(Measured::from_samples(
        "h3.read4k_ns",
        &read_ns,
        p99r as f64,
    ));
    out.push(Measured::scalar(
        "h3.chunks_per_read4k.max",
        max_span as f64,
    ));
    Ok(out)
}
