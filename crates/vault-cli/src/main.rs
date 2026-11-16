//! CLI `tholos` : adaptateur mince au-dessus de `vault-core`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use vault_core::{merkle, pack_dir, Archive, PackOptions};

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
    },
    /// Liste les fichiers d'une archive sans rien décompresser.
    List { archive: PathBuf },
    /// Vérifie chaque morceau, chaque racine de fichier et la racine d'archive.
    Verify { archive: PathBuf },
    /// Extrait toute l'archive dans un répertoire.
    Extract { archive: PathBuf, out_dir: PathBuf },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Pack {
            src_dir,
            archive,
            level,
        } => {
            if !(1..=22).contains(&level) {
                bail!("niveau zstd {level} hors plage (1 à 22)");
            }
            let opts = PackOptions {
                level,
                ..PackOptions::default()
            };
            let s = pack_dir(&src_dir, &archive, &opts)
                .with_context(|| format!("archivage de {}", src_dir.display()))?;
            println!(
                "{} fichiers, {} octets → {} octets ({} morceaux uniques sur {} références)",
                s.files, s.input_bytes, s.archive_bytes, s.unique_chunks, s.total_chunk_refs
            );
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
            let ar = open(&archive)?;
            let r = ar.verify();
            if r.is_ok() {
                println!(
                    "OK : {} morceaux, {} fichiers",
                    r.chunks_checked,
                    ar.files().len()
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
                bail!("vérification échouée");
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
    }
    Ok(())
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
