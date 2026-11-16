//! H5 — Parité Reed-Solomon : réparation de dégâts aléatoires et contigus dans
//! le budget déclaré, coût de construction rapporté à BLAKE3.

use std::collections::BTreeSet;
use std::fs;

use anyhow::{Context as _, Result};
use vault_core::{parity, repair_file, Archive, ParityOptions};

use super::{pack_opts, Ctx, Produced};
use crate::corpus;
use crate::measure::{median, s_per_gb, time_ns};
use crate::prng::Xorshift;
use crate::report::Measured;
use crate::theses::h1::LEVEL;

pub const SEED: u64 = 51;

#[derive(Clone, Copy)]
enum Kind {
    Random,
    Contig,
}

/// (identifiant, budget de parité %, forme du dégât, part du protégé en ‰)
const SCENARIOS: [(&str, u8, Kind, u64); 4] = [
    ("random5@p10", 10, Kind::Random, 50),
    ("contig10@p10", 10, Kind::Contig, 100),
    ("random2@p5", 5, Kind::Random, 20),
    ("contig5@p5", 5, Kind::Contig, 50),
];

fn damage(bytes: &mut [u8], protected_len: u64, kind: Kind, permille: u64, rng: &mut Xorshift) {
    match kind {
        Kind::Contig => {
            let len = (protected_len * permille / 1000) as usize;
            let start = rng.below(protected_len - len as u64) as usize;
            bytes[start..start + len].fill(0xff);
        }
        Kind::Random => {
            let shards = protected_len.div_ceil(4096);
            let n = (shards * permille / 1000) as usize;
            let mut hit = BTreeSet::new();
            while hit.len() < n {
                let i = rng.below(shards);
                if hit.insert(i) {
                    let s = (i * 4096) as usize;
                    let e = (s + 4096).min(protected_len as usize);
                    for b in &mut bytes[s..e] {
                        *b ^= 0xa5;
                    }
                }
            }
        }
    }
}

pub fn run(ctx: &mut Ctx) -> Result<Produced> {
    let seeds = ctx.profile.parity_seeds;
    let bytes = ctx.profile.parity_bytes;
    let mut repaired = vec![0u64; SCENARIOS.len()];
    let mut repair_ns = Vec::new();
    let mut build_ns_per_mib = Vec::new();
    let dmg = ctx.work.join("parity-dmg.vault");

    for seed in 0..seeds {
        let files = corpus::gen_parity(SEED + seed, bytes);
        let (src, _) = corpus::materialize(ctx.work, "parity-src", &files)?;
        for percent in [10u8, 5] {
            let opts = pack_opts(
                LEVEL,
                ParityOptions {
                    percent,
                    ..ParityOptions::default()
                },
            );
            let dst = ctx.work.join(format!("parity-p{percent}.vault"));
            vault_core::pack_dir(&src, &dst, &opts).context("pack parité")?;
            let original = fs::read(&dst)?;
            let protected_len = Archive::open(&dst)?.trailer().parity_off();
            if percent == 10 {
                let protected = &original[..protected_len as usize];
                let (ns, section) = time_ns(|| parity::build_section(protected, &opts.parity));
                section?;
                build_ns_per_mib.push(ns * (1 << 20) / protected_len.max(1));
            }
            for (i, (name, p, kind, permille)) in SCENARIOS.iter().enumerate() {
                if *p != percent {
                    continue;
                }
                let mut rng = Xorshift::new(SEED ^ (seed << 8) ^ i as u64);
                let mut corrupted = original.clone();
                damage(&mut corrupted, protected_len, *kind, *permille, &mut rng);
                fs::write(&dmg, &corrupted)?;
                let (ns, report) = time_ns(|| repair_file(&dmg));
                let ok = match report {
                    Ok(r) => r.verify_after.is_ok() && fs::read(&dmg)? == original,
                    Err(_) => false,
                };
                if ok {
                    repaired[i] += 1;
                }
                if *name == "contig10@p10" {
                    repair_ns.push(ns);
                }
            }
        }
    }

    let mut out = Vec::new();
    for (i, (name, ..)) in SCENARIOS.iter().enumerate() {
        out.push(Measured::scalar(
            &format!("h5.repair_rate.{name}"),
            repaired[i] as f64 / seeds as f64,
        ));
    }
    let build_med = median(&build_ns_per_mib);
    out.push(Measured::scalar(
        "h5.build_vs_blake3",
        build_med as f64 / ctx.calib_ns_per_mib as f64,
    ));
    out.push(Measured::scalar(
        "h5.build_s_per_gb",
        s_per_gb(build_med, 1 << 20),
    ));
    out.push(Measured::scalar(
        "h5.repair_s_per_gb",
        s_per_gb(median(&repair_ns), bytes),
    ));
    Ok(out)
}
