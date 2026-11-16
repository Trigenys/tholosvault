//! H2 — Déduplication par découpage défini par le contenu : rappel sur copies
//! exactes et éditées, et coût d'archive face à zstd solide (sans parité).

use std::collections::HashSet;

use anyhow::Result;
use vault_core::{chunk, ChunkParams, ParityOptions};

use super::{pack_opts, pct_loss, solid_zstd_bytes, Ctx, Produced};
use crate::corpus::{self, DupCorpus};
use crate::report::Measured;

pub const SEED: u64 = 21;
pub const LEVEL: i32 = 15;
/// Taux sur lequel le rappel est mesuré.
const RECALL_RATE: u64 = 20;

/// Octets de morceaux nouveaux introduits par `extra` par rapport à l'ensemble
/// des morceaux de `base` (même découpage que `vault-core`).
fn new_unique_bytes(base: &HashSet<blake3::Hash>, extra: &[&[u8]]) -> u64 {
    let params = ChunkParams::default();
    let mut seen = base.clone();
    let mut total = 0u64;
    for data in extra {
        for r in chunk::split(data, params) {
            if seen.insert(blake3::hash(&data[r.clone()])) {
                total += r.len() as u64;
            }
        }
    }
    total
}

/// (rappel exact, rappel édité, octets de morceaux nouveaux par édition en KiB)
fn recall(c: &DupCorpus) -> (f64, f64, f64) {
    let params = ChunkParams::default();
    let mut base = HashSet::new();
    let mut exact = Vec::new();
    let mut edited = Vec::new();
    for (p, d) in &c.files {
        if p.starts_with("dup/exact/") {
            exact.push(d.as_slice());
        } else if p.starts_with("dup/edited/") {
            edited.push(d.as_slice());
        } else {
            for r in chunk::split(d, params) {
                base.insert(blake3::hash(&d[r]));
            }
        }
    }
    let r_exact = 1.0 - new_unique_bytes(&base, &exact) as f64 / c.exact_bytes.max(1) as f64;
    let new_edited = new_unique_bytes(&base, &edited);
    let r_edited = 1.0 - new_edited as f64 / c.edited_bytes.max(1) as f64;
    let cost_kib = new_edited as f64 / c.edits.max(1) as f64 / 1024.0;
    (r_exact, r_edited, cost_kib)
}

fn ldm_bytes(files: &corpus::Files, level: i32) -> Result<u64> {
    use zstd::zstd_safe::CParameter;
    let mut all = Vec::with_capacity(corpus::total_bytes(files) as usize);
    for (_, d) in files {
        all.extend_from_slice(d);
    }
    let mut c = zstd::bulk::Compressor::new(level)?;
    c.set_parameter(CParameter::EnableLongDistanceMatching(true))?;
    c.set_parameter(CParameter::WindowLog(27))?;
    Ok(c.compress(&all)?.len() as u64)
}

pub fn run(ctx: &mut Ctx) -> Result<Produced> {
    let mut out = Vec::new();
    let base = ctx.profile.dup_base_bytes;
    for &rate in &ctx.profile.dup_rates_pct {
        let id = format!("dup{rate:02}");
        let c = corpus::gen_dup(SEED, base, rate);
        let dir = ctx.materialize(&id, SEED, &c.files)?;
        let solid = solid_zstd_bytes(&c.files, LEVEL)?;
        let (_, summary) = ctx.archive(&id, &dir, &pack_opts(LEVEL, ParityOptions::disabled()))?;
        // Bande de validité : taux de doublons réel à ±2 points, et des copies
        // éditées présentes au taux où le rappel est mesuré.
        let total = corpus::total_bytes(&c.files);
        let actual = (c.exact_bytes + c.edited_bytes) * 100 / total.max(1);
        let valid = actual.abs_diff(rate) <= 2 && (rate != RECALL_RATE || c.edited_bytes > 0);
        ctx.set_corpus_validity(&id, total as f64 / solid as f64, valid);
        out.push(Measured::scalar(
            &format!("h2.penalty_pct.d{rate}"),
            pct_loss(summary.archive_bytes, solid),
        ));
        if rate == RECALL_RATE {
            let (e, d, cost_kib) = recall(&c);
            out.push(Measured::scalar("h2.dedup_recall.exact", e));
            out.push(Measured::scalar("h2.dedup_recall.edited", d));
            out.push(Measured::scalar("h2.edit_cost_kib", cost_kib));
            if ctx.profile.ldm_baseline {
                let ldm = ldm_bytes(&c.files, LEVEL)?;
                out.push(Measured::scalar(
                    &format!("h2.penalty_pct_ldm.d{rate}"),
                    pct_loss(summary.archive_bytes, ldm),
                ));
            }
        }
    }
    Ok(out)
}
