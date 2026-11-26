//! `tholos verify` : vérification complète ou rapide, sortie texte ou JSON
//! (schéma `tholos-verify/1`).

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use vault_core::{merkle, scan_file, Archive, Damage, VerifyReport};

pub fn run(archive: &Path, quick: bool, sample: usize, as_json: bool) -> Result<()> {
    let ar = match Archive::open(archive) {
        Ok(ar) => ar,
        Err(e) => {
            // Métadonnées illisibles : le bilan par shards reste possible.
            let d =
                scan_file(archive).with_context(|| format!("analyse de {}", archive.display()))?;
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&unreadable_json(&e.to_string(), &d))?
                );
            } else {
                eprintln!("ouverture impossible : {e}");
                print_damage(&d);
            }
            bail!("vérification échouée ({})", repairability(&d));
        }
    };
    let total = ar.chunks().len() as u64;
    let r = if quick {
        ar.verify_quick(sample)
    } else {
        ar.verify()
    };
    let d = r.damage.as_ref().expect("bilan établi par verify");
    let escalated = quick && r.chunks_checked == total && sample < ar.chunks().len();
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&to_json(&ar, &r, quick, escalated))?
        );
        if !r.is_ok() {
            bail!("vérification échouée ({})", repairability(d));
        }
        return Ok(());
    }
    if escalated {
        eprintln!("dégât détecté par CRC : vérification complète effectuée");
    }
    if r.is_ok() && !d.needs_repair() {
        if quick && r.chunks_checked < total {
            println!(
                "OK (rapide) : {} morceaux décompressés sur {total}, {} fichiers, {}",
                r.chunks_checked,
                ar.files().len(),
                if ar.has_parity() {
                    "CRC de tous les shards conforme"
                } else {
                    "sans parité : contenu hors échantillon non contrôlé (`tholos verify` sans --quick pour tout vérifier)"
                }
            );
        } else {
            println!(
                "OK : {} morceaux, {} fichiers{}",
                r.chunks_checked,
                ar.files().len(),
                if ar.has_parity() {
                    ", parité présente"
                } else {
                    ", sans parité"
                }
            );
        }
        return Ok(());
    }
    for i in &r.bad_chunks {
        eprintln!("morceau {i} corrompu");
    }
    for p in ar.files_using_chunks(&r.bad_chunks) {
        eprintln!("fichier atteint : {p}");
    }
    for p in &r.bad_files {
        eprintln!("fichier incohérent : {p}");
    }
    if !r.archive_root_ok {
        eprintln!("racine d'archive incohérente");
    }
    print_damage(d);
    if r.is_ok() {
        println!(
            "contenu intègre, redondance à restaurer ({})",
            repairability(d)
        );
        Ok(())
    } else {
        bail!("vérification échouée ({})", repairability(d))
    }
}

/// Schéma stable `tholos-verify/1` : clés ajoutées seulement, jamais renommées.
pub fn to_json(ar: &Archive, r: &VerifyReport, quick: bool, escalated: bool) -> Value {
    let d = r.damage.as_ref().expect("bilan établi par verify");
    json!({
        "schema": "tholos-verify/1",
        "readable": true,
        "mode": if quick && !escalated { "quick" } else { "full" },
        "escalated": escalated,
        "ok": r.is_ok(),
        "needs_repair": d.needs_repair(),
        "repairable": r.repairable(),
        "chunks_total": ar.chunks().len(),
        "chunks_checked": r.chunks_checked,
        "files": ar.files().len(),
        "bad_chunks": r.bad_chunks,
        "damaged_files": ar.files_using_chunks(&r.bad_chunks),
        "inconsistent_files": r.bad_files,
        "archive_root_ok": r.archive_root_ok,
        "parity": ar.has_parity(),
        "damage": damage_json(d),
        "archive_root": merkle::to_hex(ar.archive_root()),
    })
}

/// Archive dont l'en-tête ou les tables sont illisibles : seul le bilan par shards existe.
pub fn unreadable_json(error: &str, d: &Damage) -> Value {
    json!({
        "schema": "tholos-verify/1",
        "readable": false,
        "error": error,
        "ok": false,
        "needs_repair": true,
        "repairable": d.repairable,
        "damage": damage_json(d),
    })
}

fn damage_json(d: &Damage) -> Value {
    json!({
        "bad_data_shards": d.bad_data_shards,
        "bad_parity_shards": d.bad_parity_shards,
        "bad_trailer_copies": d.bad_trailer_copies,
        "parity_usable": d.geometry.is_some(),
        "overflow": d.overflow.map(|(group, erased, max)| {
            json!({ "group": group, "erased": erased, "parity": max })
        }),
    })
}

pub fn repairability(d: &Damage) -> &'static str {
    if d.repairable {
        "réparable"
    } else {
        "non réparable"
    }
}

pub fn print_damage(d: &Damage) {
    if !d.bad_data_shards.is_empty() {
        eprintln!("{} shard(s) de données effacé(s)", d.bad_data_shards.len());
    }
    if !d.bad_parity_shards.is_empty() {
        eprintln!("{} shard(s) de parité effacé(s)", d.bad_parity_shards.len());
    }
    if d.bad_trailer_copies > 0 {
        eprintln!("{} copie(s) du trailer illisible(s)", d.bad_trailer_copies);
    }
    if let Some((group, erased, max)) = d.overflow {
        eprintln!("groupe {group} : {erased} effacés pour {max} de parité");
    }
    if d.geometry.is_none() && !d.repairable {
        eprintln!("aucune parité exploitable");
    }
}
