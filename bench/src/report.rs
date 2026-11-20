//! Configuration déclarative (`bench/config.json`), métriques, verdicts et
//! écriture des résultats (JSON, CSV, échantillons bruts, résumé markdown).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "tholos-bench/1";
pub const CONFIG_SCHEMA: &str = "tholos-bench-config/1";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub schema: String,
    pub profiles: BTreeMap<String, Profile>,
    pub theses: BTreeMap<String, ThesisSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub code_bytes: u64,
    pub binary_bytes: u64,
    pub small_counts: [u64; 2],
    pub dup_base_bytes: u64,
    pub dup_rates_pct: Vec<u64>,
    pub parity_bytes: u64,
    pub parity_seeds: u64,
    pub level19: bool,
    pub ldm_baseline: bool,
    pub lookup_samples: usize,
    /// Taille du contenu binaire de l'archive téléchargée par H9.
    #[serde(default = "default_fetch_bytes")]
    pub fetch_bytes: u64,
}

fn default_fetch_bytes() -> u64 {
    32 * 1024 * 1024
}

#[derive(Debug, Deserialize)]
pub struct ThesisSpec {
    pub title: String,
    pub status: Status,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub prerequisite: Option<String>,
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricSpec>,
}

#[derive(Debug, Deserialize)]
pub struct MetricSpec {
    pub unit: String,
    #[serde(default)]
    pub threshold: Option<Threshold>,
    #[serde(default)]
    pub blocking: bool,
    #[serde(default)]
    pub source: Option<String>,
    /// Profils où la métrique est produite (vide = tous).
    #[serde(default)]
    pub profiles: Vec<String>,
    /// ADR justifiant un seuil rendu non bloquant après réfutation.
    #[serde(default)]
    pub adr: Option<String>,
    /// Capacité de l'hôte nécessaire pour produire la métrique (voir
    /// [`CAPABILITIES`]). Absente de l'hôte : la métrique est sautée et la
    /// thèse devient `incomplete`, jamais `supported`.
    #[serde(default)]
    pub requires: Option<String>,
    /// Systèmes (`std::env::consts::OS`) où le seuil reste déclaré et évalué
    /// mais ne bloque pas, `adr` obligatoire : une réfutation propre à un hôte
    /// se consigne sans assouplir le seuil ailleurs.
    #[serde(default)]
    pub non_blocking_on: Vec<String>,
}

impl MetricSpec {
    /// Le seuil bloque-t-il sur cet hôte ?
    pub fn blocking_on(&self, os: &str) -> bool {
        self.blocking && !self.non_blocking_on.iter().any(|o| o == os)
    }
}

/// Capacités d'hôte qu'une métrique peut exiger.
pub const CAPABILITIES: [&str; 4] = ["mount", "fuse", "winfsp", "curl"];

/// Capacités détectées sur l'hôte, avec la raison de chaque absence.
#[derive(Debug, Default, Clone)]
pub struct Environment {
    /// `std::env::consts::OS` de l'hôte (vide dans les tests).
    pub os: String,
    pub available: BTreeSet<String>,
    pub missing: BTreeMap<String, String>,
}

impl Environment {
    pub fn has(&self, cap: &str) -> bool {
        self.available.contains(cap)
    }

    pub fn missing_reason(&self, cap: &str) -> String {
        self.missing
            .get(cap)
            .cloned()
            .unwrap_or_else(|| format!("capacité `{cap}` absente de l'hôte"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Measured,
    Partial,
    NotMeasured,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Measured => "measured",
            Status::Partial => "partial",
            Status::NotMeasured => "not_measured",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threshold {
    pub op: String,
    pub value: f64,
}

impl Threshold {
    pub fn check(&self, v: f64) -> Result<bool> {
        Ok(match self.op.as_str() {
            "<=" => v <= self.value,
            "<" => v < self.value,
            ">=" => v >= self.value,
            ">" => v > self.value,
            "==" => (v - self.value).abs() < f64::EPSILON,
            other => bail!("opérateur de seuil inconnu : {other}"),
        })
    }
}

/// Valeur produite par un module de thèse.
#[derive(Debug, Clone)]
pub struct Measured {
    pub id: String,
    pub value: f64,
    pub p50: Option<f64>,
    pub p99: Option<f64>,
    pub samples: u64,
}

impl Measured {
    pub fn scalar(id: &str, value: f64) -> Self {
        Self {
            id: id.to_string(),
            value,
            p50: None,
            p99: None,
            samples: 1,
        }
    }

    /// Métrique issue de latences (ns) ; `value` est la grandeur confrontée au
    /// seuil (souvent le p99 ou un ratio dérivé).
    pub fn from_samples(id: &str, ns: &[u64], value: f64) -> Self {
        Self {
            id: id.to_string(),
            value,
            p50: Some(crate::measure::percentile(ns, 50.0) as f64),
            p99: Some(crate::measure::percentile(ns, 99.0) as f64),
            samples: ns.len() as u64,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MetricOut {
    pub id: String,
    pub value: f64,
    pub unit: String,
    pub p50: Option<f64>,
    pub p99: Option<f64>,
    pub samples: u64,
    pub threshold: Option<Threshold>,
    pub blocking: bool,
    pub pass: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Raison pour laquelle la métrique n'a pas été produite sur cet hôte.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ThesisOut {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub reason: Option<String>,
    pub prerequisite: Option<String>,
    pub verdict: String,
    pub metrics: Vec<MetricOut>,
}

#[derive(Debug, Serialize)]
pub struct CorpusOut {
    pub id: String,
    pub seed: u64,
    pub files: u64,
    pub bytes: u64,
    pub digest: String,
    pub ratio_solid: Option<f64>,
    pub valid: bool,
}

#[derive(Debug, Serialize)]
pub struct Tool {
    pub vault_core: String,
    pub zstd: u32,
    pub git_rev: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Host {
    pub os: String,
    pub arch: String,
    pub cpus: usize,
    pub calib_blake3_ns_per_mib: u64,
    /// Capacités détectées (`mount`, `fuse`, `winfsp`).
    pub capabilities: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema: String,
    pub profile: String,
    pub tool: Tool,
    pub host: Host,
    pub corpus: Vec<CorpusOut>,
    pub theses: Vec<ThesisOut>,
    pub pass: bool,
}

pub fn load_config(path: &Path) -> Result<Config> {
    let text = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    let cfg: Config = serde_json::from_str(&text).context("config.json invalide")?;
    validate(cfg)
}

/// Contrôle le contrat déclaré : schéma, thèses présentes, cohérence
/// seuil / bloquant / ADR / capacités.
pub fn validate(cfg: Config) -> Result<Config> {
    if cfg.schema != CONFIG_SCHEMA {
        bail!("schéma de configuration inattendu : {}", cfg.schema);
    }
    for id in ["H1", "H2", "H3", "H4", "H5", "H6", "H7", "H8"] {
        if !cfg.theses.contains_key(id) {
            bail!("thèse {id} absente de la configuration");
        }
    }
    for (tid, t) in &cfg.theses {
        for (mid, m) in &t.metrics {
            if m.blocking && m.threshold.is_none() {
                bail!("{tid}/{mid} : métrique bloquante sans seuil");
            }
            if !m.blocking && m.threshold.is_some() && m.adr.is_none() {
                bail!("{tid}/{mid} : seuil non bloquant sans champ `adr` (ADR-0003)");
            }
            if !m.non_blocking_on.is_empty() && m.adr.is_none() {
                bail!("{tid}/{mid} : `non_blocking_on` sans champ `adr`");
            }
            if let Some(req) = &m.requires {
                if !CAPABILITIES.contains(&req.as_str()) {
                    bail!("{tid}/{mid} : capacité requise inconnue `{req}`");
                }
            }
        }
    }
    Ok(cfg)
}

/// Confronte les valeurs produites au contrat déclaré. Toute métrique déclarée
/// pour ce profil et non produite, ou produite et non déclarée, est une erreur
/// de configuration (code 2), pas un échec de thèse. Seule exception : une
/// métrique dont la capacité `requires` manque à l'hôte est sautée et
/// rapportée comme telle ; produite malgré l'absence, c'est une erreur.
pub fn evaluate(
    cfg: &Config,
    profile: &str,
    produced: &BTreeMap<String, Vec<Measured>>,
    only: &[String],
    env: &Environment,
) -> Result<Vec<ThesisOut>> {
    let mut out = Vec::new();
    for (id, spec) in &cfg.theses {
        if !only.is_empty() && !only.iter().any(|o| o == id) {
            continue;
        }
        let values = produced.get(id).cloned().unwrap_or_default();
        let mut by_id: BTreeMap<String, Measured> =
            values.into_iter().map(|m| (m.id.clone(), m)).collect();
        let mut metrics = Vec::new();
        for (mid, ms) in &spec.metrics {
            let expected_here = ms.profiles.is_empty() || ms.profiles.iter().any(|p| p == profile);
            let missing_cap = ms.requires.as_deref().filter(|c| !env.has(c));
            let m = match (by_id.remove(mid), expected_here, missing_cap) {
                (Some(_), _, Some(cap)) => {
                    bail!("métrique {mid} produite alors que la capacité `{cap}` est absente")
                }
                (None, true, Some(cap)) => {
                    metrics.push(MetricOut {
                        id: mid.clone(),
                        value: f64::NAN,
                        unit: ms.unit.clone(),
                        p50: None,
                        p99: None,
                        samples: 0,
                        threshold: ms.threshold.clone(),
                        blocking: ms.blocking_on(&env.os),
                        pass: None,
                        adr: ms.adr.clone(),
                        source: ms.source.clone(),
                        skipped: Some(env.missing_reason(cap)),
                    });
                    continue;
                }
                (Some(m), true, None) => m,
                (None, false, _) => continue,
                (Some(_), false, None) => {
                    bail!("métrique {mid} produite hors de ses profils déclarés")
                }
                (None, true, None) => bail!("métrique {mid} déclarée pour {id} mais non produite"),
            };
            let pass = match &ms.threshold {
                Some(t) => Some(t.check(m.value)?),
                None => None,
            };
            metrics.push(MetricOut {
                id: mid.clone(),
                value: m.value,
                unit: ms.unit.clone(),
                p50: m.p50,
                p99: m.p99,
                samples: m.samples,
                threshold: ms.threshold.clone(),
                blocking: ms.blocking_on(&env.os),
                pass,
                adr: ms.adr.clone(),
                source: ms.source.clone(),
                skipped: None,
            });
        }
        if let Some(extra) = by_id.keys().next() {
            bail!("métrique {extra} produite pour {id} mais absente de la configuration");
        }
        let verdict = match spec.status {
            Status::NotMeasured => "n/a",
            _ if metrics.iter().any(|m| m.pass == Some(false)) => "refuted",
            _ if metrics.iter().any(|m| m.skipped.is_some()) => "incomplete",
            _ if metrics.iter().any(|m| m.pass == Some(true)) => "supported",
            _ => "inconclusive",
        };
        out.push(ThesisOut {
            id: id.clone(),
            title: spec.title.clone(),
            status: spec.status,
            reason: spec.reason.clone(),
            prerequisite: spec.prerequisite.clone(),
            verdict: verdict.to_string(),
            metrics,
        });
    }
    Ok(out)
}

pub fn blocking_failures(theses: &[ThesisOut]) -> Vec<String> {
    theses
        .iter()
        .flat_map(|t| {
            t.metrics
                .iter()
                .filter(|m| m.blocking && m.pass == Some(false))
        })
        .map(|m| {
            let t = m.threshold.as_ref().expect("bloquant => seuil");
            format!(
                "{} = {:.4} {} (attendu {} {})",
                m.id, m.value, m.unit, t.op, t.value
            )
        })
        .collect()
}

pub type Sample = (String, u64, u64);

pub fn write_outputs(dir: &Path, report: &Report, samples: &[Sample]) -> Result<()> {
    fs::create_dir_all(dir)?;
    let base = dir.join(&report.profile);
    fs::write(
        base.with_extension("json"),
        serde_json::to_string_pretty(report)? + "\n",
    )?;
    let mut csv =
        String::from("profile,thesis,metric,value,unit,p50,p99,op,threshold,blocking,pass\n");
    for t in &report.theses {
        for m in &t.metrics {
            let (op, thr) = m
                .threshold
                .as_ref()
                .map(|t| (t.op.clone(), t.value.to_string()))
                .unwrap_or_default();
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{}\n",
                report.profile,
                t.id,
                m.id,
                if m.skipped.is_some() {
                    String::new()
                } else {
                    m.value.to_string()
                },
                m.unit,
                m.p50.map(|v| v.to_string()).unwrap_or_default(),
                m.p99.map(|v| v.to_string()).unwrap_or_default(),
                op,
                thr,
                m.blocking,
                m.pass.map(|v| v.to_string()).unwrap_or_default(),
            ));
        }
    }
    fs::write(base.with_extension("csv"), csv)?;
    let mut raw = String::from("metric,iteration,ns\n");
    for (m, i, ns) in samples {
        raw.push_str(&format!("{m},{i},{ns}\n"));
    }
    fs::write(dir.join(format!("{}.samples.raw", report.profile)), raw)?;
    Ok(())
}

pub fn summary_markdown(report: &Report) -> String {
    let mut md = format!(
        "## tholos-bench — profil `{}` : {}\n\nHôte : {} {} ({} cœurs), BLAKE3 {} ns/MiB.\n\n| Thèse | Statut | Verdict | Métrique | Valeur | Seuil | Bloquant | OK |\n|---|---|---|---|---|---|---|---|\n",
        report.profile,
        if report.pass { "OK" } else { "ÉCHEC" },
        report.host.os,
        report.host.arch,
        report.host.cpus,
        report.host.calib_blake3_ns_per_mib,
    );
    for t in &report.theses {
        if t.metrics.is_empty() {
            md.push_str(&format!(
                "| {} | {} | {} | — | — | — | — | {} |\n",
                t.id,
                t.status.as_str(),
                t.verdict,
                t.reason.clone().unwrap_or_default()
            ));
        }
        for m in &t.metrics {
            let thr = m
                .threshold
                .as_ref()
                .map(|x| format!("{} {}", x.op, x.value))
                .unwrap_or_else(|| "—".into());
            let ok = match (&m.skipped, m.pass) {
                (Some(why), _) => format!("sauté : {why}"),
                (None, Some(true)) => "✓".into(),
                (None, Some(false)) => "✗".into(),
                (None, None) => "·".into(),
            };
            let value = if m.skipped.is_some() {
                "—".to_string()
            } else {
                format!("{:.4} {}", m.value, m.unit)
            };
            md.push_str(&format!(
                "| {} | {} | {} | `{}` | {} | {} | {} | {} |\n",
                t.id,
                t.status.as_str(),
                t.verdict,
                m.id,
                value,
                thr,
                m.blocking,
                ok
            ));
        }
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Config {
        load_config(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/config.json"
        )))
        .unwrap()
    }

    #[test]
    fn thresholds() {
        let t = Threshold {
            op: "<=".into(),
            value: 20.0,
        };
        assert!(t.check(20.0).unwrap());
        assert!(!t.check(20.1).unwrap());
        assert!(Threshold {
            op: "?".into(),
            value: 0.0
        }
        .check(1.0)
        .is_err());
    }

    #[test]
    fn shipped_config_is_valid_and_has_blocking_metrics() {
        let cfg = shipped();
        assert!(cfg.profiles.contains_key("ci") && cfg.profiles.contains_key("full"));
        let blocking = cfg
            .theses
            .values()
            .flat_map(|t| t.metrics.values())
            .filter(|m| m.blocking)
            .count();
        assert!(blocking >= 10, "{blocking}");
    }

    #[test]
    fn contract_rejects_unknown_or_missing_metrics() {
        let cfg = shipped();
        let env = Environment::default();
        let mut produced = BTreeMap::new();
        produced.insert("H6".to_string(), vec![Measured::scalar("h6.inconnue", 1.0)]);
        assert!(evaluate(&cfg, "ci", &produced, &[], &env).is_err());
        assert!(evaluate(&cfg, "ci", &BTreeMap::new(), &[], &env).is_err());
        let only = ["H6".to_string()];
        let ok = evaluate(&cfg, "ci", &BTreeMap::new(), &only, &env).unwrap();
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].verdict, "n/a");
    }

    fn with_mount() -> Environment {
        let mut env = Environment::default();
        env.available.insert("mount".into());
        env.available.insert("fuse".into());
        env
    }

    fn h4_core_only() -> BTreeMap<String, Vec<Measured>> {
        let mut p = BTreeMap::new();
        p.insert(
            "H4".to_string(),
            vec![
                Measured::scalar("h4.core_read_ratio", 0.95),
                Measured::scalar("h4.core_concurrent_read_ratio", 0.9),
                Measured::scalar("h4.core_concurrent_decode_ratio", 1.0),
                Measured::scalar("h4.threads", 4.0),
            ],
        );
        p
    }

    #[test]
    fn missing_capability_skips_metrics_and_marks_incomplete() {
        let cfg = shipped();
        let only = ["H4".to_string()];
        let out = evaluate(&cfg, "ci", &h4_core_only(), &only, &Environment::default()).unwrap();
        assert_eq!(out[0].verdict, "incomplete");
        let skipped: Vec<_> = out[0]
            .metrics
            .iter()
            .filter(|m| m.skipped.is_some())
            .collect();
        assert!(skipped.len() >= 2, "{}", skipped.len());
        assert!(skipped.iter().all(|m| m.pass.is_none()));
        assert!(blocking_failures(&out).is_empty());
        let md = summary_markdown(&Report {
            schema: SCHEMA.into(),
            profile: "ci".into(),
            tool: Tool {
                vault_core: "0".into(),
                zstd: 0,
                git_rev: None,
            },
            host: Host {
                os: "test".into(),
                arch: "x".into(),
                cpus: 1,
                calib_blake3_ns_per_mib: 1,
                capabilities: vec![],
            },
            corpus: vec![],
            theses: out,
            pass: true,
        });
        assert!(md.contains("sauté : "), "{md}");
    }

    #[test]
    fn required_metric_produced_without_capability_is_an_error() {
        let cfg = shipped();
        let mut produced = h4_core_only();
        produced
            .get_mut("H4")
            .unwrap()
            .push(Measured::scalar("h4.mount_content_ok", 1.0));
        let only = ["H4".to_string()];
        assert!(evaluate(&cfg, "ci", &produced, &only, &Environment::default()).is_err());
    }

    fn h4_with_mount(open_ratio: f64) -> BTreeMap<String, Vec<Measured>> {
        let mut p = h4_core_only();
        p.get_mut("H4").unwrap().extend([
            Measured::scalar("h4.mount_content_ok", 1.0),
            Measured::scalar("h4.mount_read_ratio", 0.9),
            Measured::scalar("h4.mount_open_ns", 1.0),
            Measured::scalar("h4.mount_open_p99_over_blake3_mib", open_ratio),
        ]);
        p
    }

    #[test]
    fn os_exemption_keeps_threshold_and_blocks_elsewhere() {
        // ADR-0007 : `h4.mount_open_p99_over_blake3_mib` ne bloque pas sous
        // Windows, mais le seuil reste évalué (✗ visible) et bloque ailleurs.
        let cfg = shipped();
        let only = ["H4".to_string()];
        let exempt = cfg.theses["H4"].metrics["h4.mount_open_p99_over_blake3_mib"]
            .non_blocking_on
            .clone();
        assert_eq!(exempt, vec!["windows".to_string()]);
        let mut env = with_mount();
        env.os = "windows".into();
        let out = evaluate(&cfg, "ci", &h4_with_mount(4.0), &only, &env).unwrap();
        let m = out[0]
            .metrics
            .iter()
            .find(|m| m.id == "h4.mount_open_p99_over_blake3_mib")
            .unwrap();
        assert_eq!(m.pass, Some(false));
        assert!(!m.blocking && m.adr.is_some());
        assert!(blocking_failures(&out).is_empty());
        assert_eq!(out[0].verdict, "refuted");
        env.os = "linux".into();
        let out = evaluate(&cfg, "ci", &h4_with_mount(4.0), &only, &env).unwrap();
        assert_eq!(blocking_failures(&out).len(), 1, "{out:?}");
    }

    #[test]
    fn os_exemption_requires_an_adr() {
        let mut cfg = shipped();
        let m = cfg
            .theses
            .get_mut("H4")
            .unwrap()
            .metrics
            .get_mut("h4.mount_open_p99_over_blake3_mib")
            .unwrap();
        m.adr = None;
        assert!(validate(cfg).is_err());
    }

    #[test]
    fn capability_present_requires_the_metrics() {
        let cfg = shipped();
        let only = ["H4".to_string()];
        assert!(evaluate(&cfg, "ci", &h4_core_only(), &only, &with_mount()).is_err());
    }

    #[test]
    fn unknown_capability_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.json");
        let text = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/config.json")).unwrap();
        let bad = text.replacen("\"requires\": \"mount\"", "\"requires\": \"nfs\"", 1);
        assert_ne!(bad, text);
        fs::write(&p, bad).unwrap();
        let err = load_config(&p).unwrap_err().to_string();
        assert!(err.contains("capacité requise inconnue"), "{err}");
    }
}
