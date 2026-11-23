//! Contexte partagé par les modules de thèse : profil, corpus matérialisés,
//! archives empaquetées (mises en cache par identifiant), échantillons bruts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use vault_core::{pack_dir, PackOptions, PackSummary, ParityOptions};

use crate::corpus::{self, Files};
use crate::report::{CorpusOut, Environment, Measured, Profile, Sample};

pub mod h1;
pub mod h2;
pub mod h3;
pub mod h4;
pub mod h5;
pub mod h9;

pub struct Ctx<'a> {
    pub profile: &'a Profile,
    pub corpus_root: &'a Path,
    pub work: &'a Path,
    pub calib_ns_per_mib: u64,
    /// Capacités de l'hôte : une thèse ne mesure que ce que l'hôte permet.
    pub env: &'a Environment,
    pub samples: Vec<Sample>,
    pub corpus_out: Vec<CorpusOut>,
    archives: BTreeMap<String, (PathBuf, PackSummary)>,
}

impl<'a> Ctx<'a> {
    pub fn new(
        profile: &'a Profile,
        corpus_root: &'a Path,
        work: &'a Path,
        calib_ns_per_mib: u64,
        env: &'a Environment,
    ) -> Self {
        Self {
            profile,
            corpus_root,
            work,
            calib_ns_per_mib,
            env,
            samples: Vec::new(),
            corpus_out: Vec::new(),
            archives: BTreeMap::new(),
        }
    }

    /// Matérialise une catégorie sous `corpus_root/id` et l'enregistre dans le
    /// rapport. `valid` et `ratio_solid` sont complétés par l'appelant.
    pub fn materialize(&mut self, id: &str, seed: u64, files: &Files) -> Result<PathBuf> {
        let (dir, digest) = corpus::materialize(self.corpus_root, id, files)
            .with_context(|| format!("corpus {id}"))?;
        if self.corpus_out.iter().any(|c| c.id == id) {
            return Ok(dir);
        }
        self.corpus_out.push(CorpusOut {
            id: id.to_string(),
            seed,
            files: files.len() as u64,
            bytes: corpus::total_bytes(files),
            digest,
            ratio_solid: None,
            valid: true,
        });
        Ok(dir)
    }

    pub fn set_corpus_validity(&mut self, id: &str, ratio_solid: f64, valid: bool) {
        if let Some(c) = self.corpus_out.iter_mut().find(|c| c.id == id) {
            c.ratio_solid = Some(ratio_solid);
            c.valid = valid;
        }
    }

    /// Empaquette `src` dans `work/<key>.vault` (une seule fois par clé).
    pub fn archive(
        &mut self,
        key: &str,
        src: &Path,
        opts: &PackOptions,
    ) -> Result<(PathBuf, PackSummary)> {
        if let Some(hit) = self.archives.get(key) {
            return Ok(hit.clone());
        }
        let dst = self.work.join(format!("{key}.vault"));
        let summary = pack_dir(src, &dst, opts).with_context(|| format!("pack {key}"))?;
        self.archives
            .insert(key.to_string(), (dst.clone(), summary.clone()));
        Ok((dst, summary))
    }

    pub fn record_samples(&mut self, metric: &str, ns: &[u64]) {
        self.samples.extend(
            ns.iter()
                .enumerate()
                .map(|(i, &v)| (metric.to_string(), i as u64, v)),
        );
    }
}

pub fn pack_opts(level: i32, parity: ParityOptions) -> PackOptions {
    PackOptions {
        level,
        parity,
        ..PackOptions::default()
    }
}

/// Octets du corpus concaténé puis compressés en un seul flux zstd (référence
/// « solide », sans découpage).
pub fn solid_zstd_bytes(files: &Files, level: i32) -> Result<u64> {
    let mut all = Vec::with_capacity(corpus::total_bytes(files) as usize);
    for (_, d) in files {
        all.extend_from_slice(d);
    }
    Ok(zstd::bulk::compress(&all, level)?.len() as u64)
}

pub fn pct_loss(candidate: u64, reference: u64) -> f64 {
    (candidate as f64 - reference as f64) / reference as f64 * 100.0
}

pub type Produced = Vec<Measured>;
