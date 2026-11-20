//! `tholos-bench` — banc public des thèses H1–H8 de tholosvault.
//!
//! Codes de sortie : 0 toutes les métriques bloquantes passent ; 1 au moins
//! une métrique bloquante échoue ; 2 configuration ou corpus invalide.

mod corpus;
mod measure;
mod prng;
mod report;
mod theses;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Instant;

use anyhow::{bail, Context as _, Result};
use clap::Parser;

use report::{Environment, Host, Report, Tool};
use theses::Ctx;

#[derive(Parser)]
#[command(name = "tholos-bench", about = "Banc des thèses H1–H8 de tholosvault")]
struct Args {
    /// Profil de la configuration (`ci` ou `full`).
    #[arg(long, default_value = "ci")]
    profile: String,
    /// Fichier de configuration déclarative.
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/config.json"))]
    config: PathBuf,
    /// Répertoire des résultats (JSON, CSV, échantillons bruts).
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/results"))]
    out: PathBuf,
    /// Racine où le corpus synthétique est matérialisé (régénération idempotente).
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/corpus"))]
    corpus_root: PathBuf,
    /// Ne mesurer que ces thèses (ex. `--only H1,H3`).
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
    /// Fichier markdown où ajouter le résumé (ex. `$GITHUB_STEP_SUMMARY`).
    #[arg(long)]
    summary_md: Option<PathBuf>,
    /// Capacités d'hôte exigées (ex. `--require-env mount`) : absentes, le banc
    /// s'arrête en erreur (code 2) au lieu de sauter leurs métriques.
    #[arg(long, value_delimiter = ',')]
    require_env: Vec<String>,
}

/// Détecte les capacités de l'hôte sans rien monter.
fn detect_environment() -> Environment {
    let mut env = Environment {
        os: std::env::consts::OS.to_string(),
        ..Environment::default()
    };
    match vault_mount::probe() {
        Ok(backend) => {
            env.available.insert("mount".into());
            env.available.insert(backend.to_string().to_lowercase());
        }
        Err(e) => {
            for cap in ["mount", "fuse", "winfsp"] {
                env.missing.insert(cap.into(), e.to_string());
            }
        }
    }
    for cap in report::CAPABILITIES {
        if !env.has(cap) && !env.missing.contains_key(cap) {
            env.missing
                .insert(cap.into(), format!("adaptateur `{cap}` non compilé"));
        }
    }
    env
}

fn git_rev() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `Ok(true)` si tout passe, `Ok(false)` si une métrique bloquante échoue.
fn run(args: &Args) -> Result<bool> {
    let cfg = report::load_config(&args.config)?;
    let profile = cfg
        .profiles
        .get(&args.profile)
        .with_context(|| format!("profil inconnu : {}", args.profile))?;
    let work = tempfile::Builder::new()
        .prefix("tholos-bench-")
        .tempdir()
        .context("répertoire de travail")?;
    fs::create_dir_all(&args.corpus_root)?;

    let env = detect_environment();
    for cap in &args.require_env {
        if !env.has(cap) {
            bail!(
                "capacité `{cap}` exigée par --require-env mais absente : {}",
                env.missing_reason(cap)
            );
        }
    }
    let calib = measure::calib_blake3_ns_per_mib();
    eprintln!(
        "profil {} — BLAKE3 {} ns/MiB — capacités : {}",
        args.profile,
        calib,
        if env.available.is_empty() {
            "aucune".to_string()
        } else {
            env.available.iter().cloned().collect::<Vec<_>>().join(", ")
        }
    );
    let mut ctx = Ctx::new(profile, &args.corpus_root, work.path(), calib, &env);

    let wanted = |id: &str| args.only.is_empty() || args.only.iter().any(|o| o == id);
    let mut produced: BTreeMap<String, Vec<report::Measured>> = BTreeMap::new();
    type Runner = fn(&mut Ctx) -> Result<Vec<report::Measured>>;
    let runners: [(&str, Runner); 5] = [
        ("H1", theses::h1::run),
        ("H2", theses::h2::run),
        ("H3", theses::h3::run),
        ("H4", theses::h4::run),
        ("H5", theses::h5::run),
    ];
    for (id, runner) in runners {
        if !wanted(id) {
            continue;
        }
        let t = Instant::now();
        let m = runner(&mut ctx).with_context(|| format!("thèse {id}"))?;
        eprintln!("{id} : {:.1} s", t.elapsed().as_secs_f64());
        produced.insert(id.to_string(), m);
    }
    // Exécution partielle (`--only`) : le contrat ne porte que sur les thèses demandées.
    let theses_out = report::evaluate(&cfg, &args.profile, &produced, &args.only, &env)?;
    let failures = report::blocking_failures(&theses_out);
    let rep = Report {
        schema: report::SCHEMA.to_string(),
        profile: args.profile.clone(),
        tool: Tool {
            vault_core: env!("CARGO_PKG_VERSION").to_string(),
            zstd: vault_core::codec::zstd_version(),
            git_rev: git_rev(),
        },
        host: Host {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpus: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            calib_blake3_ns_per_mib: calib,
            capabilities: env.available.iter().cloned().collect(),
        },
        corpus: std::mem::take(&mut ctx.corpus_out),
        theses: theses_out,
        pass: failures.is_empty(),
    };
    let samples = std::mem::take(&mut ctx.samples);
    report::write_outputs(&args.out, &rep, &samples)?;
    let md = report::summary_markdown(&rep);
    if let Some(p) = &args.summary_md {
        use std::io::Write;
        let mut f = fs::OpenOptions::new().append(true).create(true).open(p)?;
        f.write_all(md.as_bytes())?;
    }
    print!("{md}");
    if !failures.is_empty() {
        eprintln!("Seuils bloquants non atteints :");
        for f in &failures {
            eprintln!("  - {f}");
        }
    }
    if rep.corpus.iter().any(|c| !c.valid) {
        bail!("corpus hors bande de validité");
    }
    Ok(failures.is_empty())
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("erreur : {e:#}");
            ExitCode::from(2)
        }
    }
}
