//! H1 — Perte de compression du découpage (blocs fixes de 1 MiB, puis CDC
//! tholosvault) par rapport à zstd solide au même niveau.

use anyhow::Result;
use vault_core::ParityOptions;

use super::{pack_opts, pct_loss, solid_zstd_bytes, Ctx, Produced};
use crate::corpus::{self, Files};
use crate::report::Measured;

pub const SEED_CODE: u64 = 11;
pub const SEED_BINARY: u64 = 12;
pub const LEVEL: i32 = 15;

/// Bandes de validité du corpus (ratio solide) : elles qualifient le
/// générateur, jamais les seuils.
fn band(cat: &str) -> (f64, f64) {
    match cat {
        "code" => (3.0, 10.0),
        _ => (1.05, 1.6),
    }
}

fn block_bytes(files: &Files, block: usize, level: i32) -> Result<u64> {
    let mut all = Vec::with_capacity(corpus::total_bytes(files) as usize);
    for (_, d) in files {
        all.extend_from_slice(d);
    }
    let mut total = 0u64;
    for b in all.chunks(block) {
        total += zstd::bulk::compress(b, level)?.len() as u64;
    }
    Ok(total)
}

pub fn run(ctx: &mut Ctx) -> Result<Produced> {
    let mut out = Vec::new();
    let cats = [
        ("code", SEED_CODE, ctx.profile.code_bytes),
        ("binary", SEED_BINARY, ctx.profile.binary_bytes),
    ];
    for (cat, seed, bytes) in cats {
        let files = if cat == "code" {
            corpus::gen_code(seed, bytes)
        } else {
            corpus::gen_binary(seed, bytes)
        };
        let dir = ctx.materialize(cat, seed, &files)?;
        let raw = corpus::total_bytes(&files);
        let solid = solid_zstd_bytes(&files, LEVEL)?;
        let ratio = raw as f64 / solid as f64;
        let (lo, hi) = band(cat);
        let valid = (lo..=hi).contains(&ratio);
        ctx.set_corpus_validity(cat, ratio, valid);
        if !valid {
            anyhow::bail!(
                "corpus {cat} hors bande de validité : ratio solide {ratio:.2} (attendu {lo}–{hi})"
            );
        }
        out.push(Measured::scalar(&format!("h1.ratio_solid.{cat}"), ratio));

        let block1m = block_bytes(&files, 1 << 20, LEVEL)?;
        out.push(Measured::scalar(
            &format!("h1.loss_pct.{cat}.block1m"),
            pct_loss(block1m, solid),
        ));

        let (_, summary) = ctx.archive(cat, &dir, &pack_opts(LEVEL, ParityOptions::disabled()))?;
        out.push(Measured::scalar(
            &format!("h1.loss_pct.{cat}.cdc"),
            pct_loss(summary.compressed_bytes, solid),
        ));
        out.push(Measured::scalar(
            &format!("h1.overhead_pct.{cat}.index"),
            pct_loss(summary.archive_bytes, summary.compressed_bytes),
        ));

        if ctx.profile.level19 {
            let solid19 = solid_zstd_bytes(&files, 19)?;
            let (_, s19) = ctx.archive(
                &format!("{cat}-l19"),
                &dir,
                &pack_opts(19, ParityOptions::disabled()),
            )?;
            out.push(Measured::scalar(
                &format!("h1.loss_pct.{cat}.cdc19"),
                pct_loss(s19.compressed_bytes, solid19),
            ));
        }
    }
    Ok(out)
}
