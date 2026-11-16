//! CLI `tholos` : adaptateur mince au-dessus de `vault-core`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use vault_core::{
    merkle, pack_dir, repair_file, scan_file, Archive, Damage, PackOptions, ParityOptions,
};

#[derive(Parser)]
#[command(
    name = "tholos",
    version,
    about = "tholosvault — archives montables, vérifiables, dédupliquées"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Archive un répertoire.
    Pack {
        src_dir: PathBuf,
        archive: PathBuf,
        /// Niveau zstd (1 à 22).
        #[arg(long, default_value_t = vault_core::codec::DEFAULT_LEVEL)]
        level: i32,
        /// Budget de parité Reed-Solomon en % de l'archive (0 désactive).
        #[arg(long, default_value_t = vault_core::parity::DEFAULT_PERCENT)]
        parity: u8,
    },
    /// Liste les fichiers d'une archive sans rien décompresser.
    List { archive: PathBuf },
    /// Vérifie chaque morceau, chaque racine de fichier et la racine d'archive,
    /// puis indique si les dégâts éventuels sont réparables.
    Verify { archive: PathBuf },
    /// Reconstruit les zones corrompues à partir de la parité (copie puis renommage).
    Repair { archive: PathBuf },
    /// Extrait toute l'archive dans un répertoire.
    Extract { archive: PathBuf, out_dir: PathBuf },
    /// Écrit un fichier (ou une plage) sur la sortie standard en ne décompressant
    /// que les morceaux couvrants.
    Cat {
        archive: PathBuf,
        path: String,
        /// Octet de départ dans le fichier.
        #[arg(long, default_value_t = 0)]
        offset: u64,
        /// Nombre d'octets à lire (défaut : jusqu'à la fin du fichier).
        #[arg(long)]
        len: Option<u64>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Pack {
            src_dir,
            archive,
            level,
            parity,
        } => {
            if !(1..=22).contains(&level) {
                bail!("niveau zstd {level} hors plage (1 à 22)");
            }
            let opts = PackOptions {
                level,
                parity: ParityOptions {
                    percent: parity,
                    ..ParityOptions::default()
                },
                ..PackOptions::default()
            };
            let s = pack_dir(&src_dir, &archive, &opts)
                .with_context(|| format!("archivage de {}", src_dir.display()))?;
            println!(
                "{} fichiers, {} octets → {} octets ({} morceaux uniques sur {} références)",
                s.files, s.input_bytes, s.archive_bytes, s.unique_chunks, s.total_chunk_refs
            );
            if s.parity_bytes > 0 {
                println!("parité {} octets ({parity} %)", s.parity_bytes);
            }
            println!("racine {}", merkle::to_hex(&s.archive_root));
        }
        Cmd::List { archive } => {
            let ar = open(&archive)?;
            for f in ar.files() {
                println!("{:>12}  {}  {}", f.size, merkle::to_hex(&f.root), f.path);
            }
            println!("racine {}", merkle::to_hex(ar.archive_root()));
        }
        Cmd::Verify { archive } => {
            let ar = match Archive::open(&archive) {
                Ok(ar) => ar,
                Err(e) => {
                    // Métadonnées illisibles : le bilan par shards reste possible.
                    eprintln!("ouverture impossible : {e}");
                    let d = scan_file(&archive)
                        .with_context(|| format!("analyse de {}", archive.display()))?;
                    print_damage(&d);
                    bail!("vérification échouée ({})", repairability(&d));
                }
            };
            let r = ar.verify();
            let d = r.damage.as_ref().expect("bilan établi par verify");
            if r.is_ok() && !d.needs_repair() {
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
            } else {
                for i in &r.bad_chunks {
                    eprintln!("morceau {i} corrompu");
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
                } else {
                    bail!("vérification échouée ({})", repairability(d));
                }
            }
        }
        Cmd::Repair { archive } => {
            let r = repair_file(&archive)
                .with_context(|| format!("réparation de {}", archive.display()))?;
            if !r.rewritten {
                println!("rien à réparer");
            } else {
                println!(
                    "{} shards reconstruits, {} copie(s) du trailer restaurée(s)",
                    r.patched_shards, r.trailer_copies_restored
                );
                if !r.verify_after.is_ok() {
                    bail!("l'archive reste incohérente après réparation");
                }
                println!("vérification après réparation : OK");
            }
        }
        Cmd::Extract { archive, out_dir } => {
            let ar = open(&archive)?;
            for f in ar.files() {
                let data = ar.read_file(&f.path).with_context(|| f.path.clone())?;
                let dest = safe_join(&out_dir, &f.path)?;
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&dest, data).with_context(|| dest.display().to_string())?;
            }
            println!(
                "{} fichiers extraits dans {}",
                ar.files().len(),
                out_dir.display()
            );
        }
        Cmd::Cat {
            archive,
            path,
            offset,
            len,
        } => {
            let ar = open(&archive)?;
            let idx = ar
                .find_index(&path)
                .ok_or_else(|| anyhow::anyhow!("fichier absent de l'archive : {path}"))?;
            let size = ar.files()[idx].size;
            let mut remaining = len.unwrap_or(u64::MAX).min(size.saturating_sub(offset));
            let mut pos = offset;
            let mut buf = vec![0u8; 1 << 20];
            let stdout = io::stdout();
            let mut out = stdout.lock();
            while remaining > 0 {
                let want = remaining.min(buf.len() as u64) as usize;
                let n = ar
                    .read_range_into(idx, pos, &mut buf[..want])
                    .with_context(|| format!("lecture de {path} à {pos}"))?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                pos += n as u64;
                remaining -= n as u64;
            }
            out.flush()?;
        }
    }
    Ok(())
}

fn repairability(d: &Damage) -> &'static str {
    if d.repairable {
        "réparable"
    } else {
        "non réparable"
    }
}

fn print_damage(d: &Damage) {
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

fn open(p: &Path) -> Result<Archive> {
    Archive::open(p).with_context(|| format!("ouverture de {}", p.display()))
}

/// Refuse les chemins qui sortiraient du répertoire cible.
fn safe_join(root: &Path, rel: &str) -> Result<PathBuf> {
    let mut out = root.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
            bail!("chemin d'archive refusé : {rel}");
        }
        out.push(part);
    }
    Ok(out)
}
