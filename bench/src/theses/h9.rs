//! H9 — Téléchargement conscient du format (ADR-0005) : une corruption coûte un
//! morceau et une requête, une reprise ne re-télécharge aucun octet vérifié, une
//! racine attendue différente est refusée avant tout octet de contenu, et le
//! débit agrégé sur N connexions se compare à `curl` parallèle sur le même
//! serveur local.

use std::fs;
use std::ops::Range;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{bail, Context as _, Result};
use vault_core::{Archive, ParityOptions};
use vault_fetch::{Error, Event, FetchOptions, Fetcher, Outcome, Summary};
use vault_fetch_http::test_server::{CorruptAt, Cut, TestServer};
use vault_fetch_http::{HttpTransportBuilder, RetryPolicy};

use super::{pack_opts, Ctx, Produced};
use crate::corpus;
use crate::measure::{median, s_per_gb};
use crate::report::Measured;
use crate::theses::h1::LEVEL;

pub const SEED: u64 = 91;
/// Connexions parallèles, côté `tholos fetch` comme côté `curl`.
const CONNECTIONS: usize = 4;
/// Passes chronométrées (médiane).
const PASSES: usize = 3;

struct Lab {
    server: TestServer,
    data: Vec<u8>,
    chunks: Vec<Range<u64>>,
    /// Début des tables : tout GET commençant avant est une requête de contenu.
    tables_start: u64,
    dest: PathBuf,
}

impl Lab {
    fn opts(&self) -> FetchOptions {
        let mut o = FetchOptions::new(&self.dest, self.server.url());
        o.connections = CONNECTIONS;
        o
    }

    async fn run(&self, opts: FetchOptions) -> vault_fetch::Result<Summary> {
        let t = HttpTransportBuilder::new(self.server.url())
            .retry(RetryPolicy {
                initial: std::time::Duration::from_millis(10),
                ..RetryPolicy::default()
            })
            .build()
            .expect("client HTTP local");
        Fetcher::new(t, opts).run().await
    }

    fn reset(&self) {
        let _ = fs::remove_file(&self.dest);
        let _ = fs::remove_file(vault_fetch::part_path(&self.dest));
        self.server.clear_log();
    }

    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn check_identical(&self) -> Result<()> {
        if fs::read(&self.dest)? != self.data {
            bail!("destination différente de la source");
        }
        Ok(())
    }

    /// Requêtes GET ayant demandé au moins un octet de contenu (hors tables et trailer).
    fn content_gets(&self) -> usize {
        let tail = self.tables_start;
        self.server
            .log()
            .iter()
            .filter(|r| r.method == "GET")
            .filter(|r| r.range.as_ref().map(|x| x.start < tail).unwrap_or(true))
            .count()
    }
}

/// H9a : un morceau corrompu coûte au plus 1 morceau et 1 requête de plus.
async fn corrupt_cost(lab: &Lab, clean_requests: u64) -> Result<f64> {
    lab.reset();
    let target = lab.chunks[lab.chunks.len() / 3].clone();
    lab.server.faults().corrupt = Some(CorruptAt {
        offset: target.start + 1,
        remaining: 1,
    });
    let s = lab.run(lab.opts()).await.context("passe corrompue")?;
    lab.server.faults().corrupt = None;
    if s.outcome != Outcome::Complete {
        bail!("passe corrompue non complète : {:?}", s.outcome);
    }
    lab.check_identical()?;
    let extra_requests = s.requests.saturating_sub(clean_requests);
    let chunk_len = target.end - target.start;
    // Coût exprimé en morceaux : le pire des trois comptes (morceaux rejetés,
    // requêtes en plus, octets rejetés rapportés à la taille du morceau).
    let cost = (s.chunks_rejected as f64)
        .max(extra_requests as f64)
        .max(s.bytes_rejected as f64 / chunk_len as f64);
    Ok(cost)
}

/// H9a : après une coupure, la reprise ne re-télécharge aucun octet vérifié.
async fn resume_rebytes(lab: &Lab) -> Result<f64> {
    lab.reset();
    let mid = lab.chunks[lab.chunks.len() / 2].clone();
    lab.server.faults().cut = Some(Cut {
        at: mid.start,
        after: 1000,
        remaining: 1,
    });
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let mut o = lab.opts();
    o.max_segment_attempts = 1;
    o.events = Some(tx);
    match lab.run(o).await {
        Err(Error::SegmentExhausted { .. }) => {}
        Err(e) => bail!("coupure : erreur inattendue {e}"),
        Ok(_) => bail!("la coupure n'a pas interrompu la passe"),
    }
    lab.server.faults().cut = None;
    let mut verified: Vec<Range<u64>> = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        if let Event::UnitVerified { start, bytes, .. } = ev {
            verified.push(start..start + bytes);
        }
    }
    if verified.is_empty() {
        bail!("aucune unité vérifiée avant la coupure");
    }
    lab.server.clear_log();
    let s = lab.run(lab.opts()).await.context("reprise")?;
    if s.outcome != Outcome::Complete {
        bail!("reprise non complète : {:?}", s.outcome);
    }
    lab.check_identical()?;
    let rebytes: u64 = verified
        .iter()
        .map(|r| lab.server.bytes_served_within(r.clone()))
        .sum();
    Ok(rebytes as f64)
}

/// H9e : racine attendue différente → refus avant tout octet de contenu.
async fn root_mismatch_rejected(lab: &Lab) -> Result<f64> {
    lab.reset();
    let mut o = lab.opts();
    o.expected_root = Some([0xab; 32]);
    let refused = matches!(lab.run(o).await, Err(Error::RootMismatch { .. }));
    let clean = lab.content_gets() == 0
        && !lab.dest.exists()
        && !vault_fetch::part_path(&lab.dest).exists();
    Ok(if refused && clean { 1.0 } else { 0.0 })
}

/// Médiane du temps de `PASSES` téléchargements complets, en ns.
async fn fetch_ns(lab: &Lab) -> Result<u64> {
    let mut ns = Vec::with_capacity(PASSES);
    for _ in 0..PASSES {
        lab.reset();
        let t = Instant::now();
        let s = lab.run(lab.opts()).await.context("passe chronométrée")?;
        ns.push(t.elapsed().as_nanos() as u64);
        if s.outcome != Outcome::Complete {
            bail!("passe chronométrée non complète : {:?}", s.outcome);
        }
    }
    lab.check_identical()?;
    Ok(median(&ns))
}

/// `curl` parallèle : `CONNECTIONS` processus, une plage contiguë chacun,
/// vers des fichiers séparés (le découpage le plus favorable à `curl`).
fn curl_parallel_ns(lab: &Lab, work: &std::path::Path) -> Result<u64> {
    let len = lab.len();
    let part = len.div_ceil(CONNECTIONS as u64);
    let mut ns = Vec::with_capacity(PASSES);
    for pass in 0..PASSES {
        let t = Instant::now();
        let mut children = Vec::with_capacity(CONNECTIONS);
        for i in 0..CONNECTIONS as u64 {
            let start = i * part;
            let end = ((i + 1) * part).min(len);
            if start >= end {
                break;
            }
            let out = work.join(format!("curl-{pass}-{i}.part"));
            let child = Command::new("curl")
                .args(["-s", "-S", "-f", "-r"])
                .arg(format!("{start}-{}", end - 1))
                .arg("-o")
                .arg(&out)
                .arg(lab.server.url())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .context("lancement de curl")?;
            children.push((child, out));
        }
        for (child, out) in children {
            let st = child.wait_with_output().context("attente de curl")?;
            if !st.status.success() {
                bail!(
                    "curl a échoué : {}",
                    String::from_utf8_lossy(&st.stderr).trim()
                );
            }
            let _ = fs::remove_file(out);
        }
        ns.push(t.elapsed().as_nanos() as u64);
    }
    Ok(median(&ns))
}

pub fn run(ctx: &mut Ctx) -> Result<Produced> {
    let bytes = ctx.profile.fetch_bytes;
    let files = corpus::gen_binary(SEED, bytes);
    let (src, _) = corpus::materialize(ctx.work, "fetch-src", &files)?;
    let (archive, _) = ctx.archive("fetch", &src, &pack_opts(LEVEL, ParityOptions::default()))?;
    let data = fs::read(&archive)?;
    let opened = Archive::open(&archive)?;
    let tables_start = opened.trailer().chunk_index_off;
    let chunks: Vec<Range<u64>> = opened
        .chunks()
        .iter()
        .map(|c| c.offset..c.offset + c.compressed_len as u64)
        .collect();
    if chunks.len() < 8 {
        bail!("archive H9 trop petite : {} morceaux", chunks.len());
    }
    let dest = ctx.work.join("fetch-out.vault");
    let has_curl = ctx.env.has("curl");
    let work = ctx.work.to_path_buf();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("exécuteur tokio")?;
    let out = rt.block_on(async move {
        let server = TestServer::start(data.clone()).await;
        let lab = Lab {
            server,
            data,
            chunks,
            tables_start,
            dest,
        };
        let mut out = Vec::new();

        lab.reset();
        let clean = lab.run(lab.opts()).await.context("passe propre")?;
        if clean.outcome != Outcome::Complete {
            bail!("passe propre non complète : {:?}", clean.outcome);
        }
        lab.check_identical()?;

        out.push(Measured::scalar(
            "h9.corrupt_chunk_cost",
            corrupt_cost(&lab, clean.requests).await?,
        ));
        out.push(Measured::scalar(
            "h9.resume_rebytes",
            resume_rebytes(&lab).await?,
        ));
        out.push(Measured::scalar(
            "h9.root_mismatch_rejected",
            root_mismatch_rejected(&lab).await?,
        ));

        if has_curl {
            let fetch = fetch_ns(&lab).await?;
            let curl = curl_parallel_ns(&lab, &work)?;
            out.push(Measured::scalar(
                "h9.fetch_s_per_gb",
                s_per_gb(fetch, lab.len()),
            ));
            out.push(Measured::scalar(
                "h9.curl_s_per_gb",
                s_per_gb(curl, lab.len()),
            ));
            out.push(Measured::scalar(
                "h9.aggregate_throughput_ratio",
                curl as f64 / fetch as f64,
            ));
        }
        lab.reset();
        lab.server.abort();
        Ok::<_, anyhow::Error>(out)
    })?;
    Ok(out)
}
