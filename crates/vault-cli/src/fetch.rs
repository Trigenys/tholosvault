//! `tholos fetch` : téléchargement conscient du format (ADR-0005).
//!
//! Source HTTP(S), ou locale (`file://…` ou chemin : disque externe, partage
//! monté) avec la même vérification par morceau et la même reprise.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use vault_core::merkle::{to_hex, Hash};
use vault_fetch::{
    local_source, parse_rate, Event, FetchOptions, Fetcher, FileTransport, Outcome, Transport,
    UnitKind,
};
use vault_fetch_http::HttpTransportBuilder;

pub struct FetchArgs {
    pub url: String,
    pub out: Option<PathBuf>,
    pub root: Option<String>,
    pub connections: usize,
    pub limit: Option<String>,
    pub no_parity: bool,
    pub plan: bool,
    pub restart: bool,
    pub quiet: bool,
}

fn parse_root(s: &str) -> Result<Hash> {
    let s = s.trim();
    if s.len() != 64 {
        bail!("racine attendue : 64 caractères hexadécimaux");
    }
    let mut h = [0u8; 32];
    for (i, b) in h.iter_mut().enumerate() {
        *b =
            u8::from_str_radix(&s[2 * i..2 * i + 2], 16).context("racine hexadécimale invalide")?;
    }
    Ok(h)
}

fn default_out(url: &str) -> PathBuf {
    if let Some(local) = local_source(url) {
        return local
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("archive.vault"));
    }
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("archive.vault");
    PathBuf::from(name)
}

fn human(n: u64) -> String {
    const UNITS: [&str; 5] = ["o", "Kio", "Mio", "Gio", "Tio"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} o")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

pub fn run(a: FetchArgs) -> Result<()> {
    let dest = a.out.clone().unwrap_or_else(|| default_out(&a.url));
    let expected_root = a.root.as_deref().map(parse_root).transpose()?;
    let limit = match &a.limit {
        Some(s) => Some(parse_rate(s).with_context(|| format!("limite de débit invalide : {s}"))?),
        None => None,
    };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("démarrage de l'exécuteur")?;
    match local_source(&a.url) {
        Some(src) => {
            let transport = FileTransport::new(&src, &dest)
                .with_context(|| format!("source locale {}", src.display()))?;
            rt.block_on(drive(transport, a, dest, expected_root, limit))
        }
        None => {
            let transport = HttpTransportBuilder::new(a.url.clone())
                .build()
                .context("client HTTP")?;
            rt.block_on(drive(transport, a, dest, expected_root, limit))
        }
    }
}

async fn drive<T: Transport + 'static>(
    transport: T,
    a: FetchArgs,
    dest: PathBuf,
    expected_root: Option<Hash>,
    limit: Option<u64>,
) -> Result<()> {
    let mut opts = FetchOptions::new(&dest, &a.url);
    opts.expected_root = expected_root;
    opts.connections = a.connections.max(1);
    opts.limit_bytes_per_sec = limit;
    opts.skip_parity = a.no_parity;
    opts.restart = a.restart;

    if a.plan {
        let report = Fetcher::new(transport, opts).plan().await?;
        print_plan(&dest, &report);
        return Ok(());
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    opts.events = Some(tx);
    let quiet = a.quiet;
    let progress = tokio::spawn(async move {
        let mut total = 0u64;
        let mut last = std::time::Instant::now() - Duration::from_secs(1);
        while let Some(ev) = rx.recv().await {
            if quiet {
                continue;
            }
            match ev {
                Event::Probed {
                    len,
                    accepts_ranges,
                } => {
                    total = len;
                    if !accepts_ranges {
                        eprintln!(
                            "la source n'accepte pas les plages : flux unique, vérification en fin"
                        );
                    }
                }
                Event::Planned {
                    units,
                    verified_units,
                    missing_bytes,
                    total_bytes,
                } => {
                    total = total_bytes;
                    eprintln!(
                        "{units} unités, {verified_units} déjà vérifiées, {} à recevoir sur {}",
                        human(missing_bytes),
                        human(total_bytes)
                    );
                }
                Event::UnitVerified {
                    verified_bytes,
                    kind,
                    ..
                } => {
                    if last.elapsed() >= Duration::from_millis(200)
                        || verified_bytes == total
                        || matches!(kind, UnitKind::Trailer)
                    {
                        last = std::time::Instant::now();
                        let pct = (verified_bytes * 100).checked_div(total).unwrap_or(0);
                        eprint!("\r{pct:3}% {} / {}   ", human(verified_bytes), human(total));
                    }
                }
                Event::ChunkRejected { index, attempt } => {
                    eprintln!("\nmorceau {index} rejeté (BLAKE3), nouvelle demande n° {attempt}");
                }
                Event::SegmentCut { resume_at, .. } => {
                    eprintln!("\ncoupure, reprise à l'octet {resume_at}");
                }
                Event::SegmentRetry {
                    attempt, reason, ..
                } => {
                    eprintln!("\nrelance n° {attempt} : {reason}");
                }
            }
        }
    });
    let summary = Fetcher::new(transport, opts).run().await;
    let _ = progress.await;
    let s = summary?;
    if !quiet {
        eprintln!();
    }
    let status = match s.outcome {
        Outcome::AlreadyComplete => "déjà complète",
        Outcome::Complete => "complète",
        Outcome::CompleteWithoutParity => "complète sans parité (état .part conservé)",
    };
    println!(
        "{} : {} ({}), racine {}",
        dest.display(),
        status,
        human(s.len),
        to_hex(&s.archive_root)
    );
    if !quiet {
        eprintln!(
            "{} requêtes, {} reçus, {} morceau(x) rejeté(s) ({}), {} coupure(s), {:.1} s",
            s.requests,
            human(s.bytes_received),
            s.chunks_rejected,
            human(s.bytes_rejected),
            s.cuts,
            s.elapsed.as_secs_f64()
        );
    }
    Ok(())
}

fn print_plan(dest: &Path, r: &vault_fetch::PlanReport) {
    println!("destination : {}", dest.display());
    println!("taille      : {}", human(r.len));
    println!("racine      : {}", to_hex(&r.archive_root));
    println!(
        "plages      : {}",
        if r.accepts_ranges {
            "oui"
        } else {
            "non (flux unique)"
        }
    );
    if r.already_complete {
        println!("état        : destination déjà complète, rien à télécharger");
        return;
    }
    println!(
        "unités      : {} ({} déjà vérifiées{})",
        r.units,
        r.verified_units,
        if r.resumed { ", reprise" } else { "" }
    );
    println!("contenu     : {}", human(r.content_bytes));
    println!(
        "à recevoir  : {} en {} requête(s){}",
        human(r.missing_bytes),
        r.segments,
        if r.skip_parity {
            ", parité ignorée"
        } else {
            ""
        }
    );
}
