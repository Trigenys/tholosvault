//! CLI `tholos` : adaptateur mince au-dessus de `vault-core`.

mod extract;
mod fetch;
mod info;
mod verify;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use vault_core::{merkle, pack_dir, repair_file, Archive, PackOptions, ParityOptions};
use vault_mount::{MountOptions, VaultFs};

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
    Verify {
        archive: PathBuf,
        /// Vérification rapide : racines depuis les tables, CRC32C de tous les
        /// shards si l'archive porte une parité, et un échantillon de morceaux
        /// décompressés. Un dégât détecté déclenche la vérification complète.
        /// Sans parité, seul l'échantillon contrôle le contenu.
        #[arg(long)]
        quick: bool,
        /// Morceaux décompressés par la vérification rapide.
        #[arg(long, default_value_t = 64, requires = "quick")]
        sample: usize,
        /// Sortie JSON (schéma `tholos-verify/1`) : morceaux corrompus, fichiers
        /// atteints, bilan par shards, réparabilité. Code de sortie inchangé.
        #[arg(long)]
        json: bool,
    },
    /// Reconstruit les zones corrompues à partir de la parité (copie puis renommage).
    Repair { archive: PathBuf },
    /// Extrait l'archive, ou seulement les chemins donnés, dans un répertoire.
    /// Écriture en flux (mémoire bornée par un morceau), chaque morceau vérifié ;
    /// un fichier n'apparaît sous son nom qu'une fois complet.
    Extract {
        archive: PathBuf,
        out_dir: PathBuf,
        /// Fichiers ou répertoires à extraire, tels que `tholos list` les affiche
        /// (défaut : toute l'archive).
        paths: Vec<String>,
    },
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
    /// Monte l'archive en lecture seule sur un répertoire vide (FUSE sous Linux).
    /// Ctrl-C démonte.
    Mount {
        archive: PathBuf,
        mountpoint: PathBuf,
        /// Threads servant les requêtes du noyau (défaut : parallélisme disponible).
        #[arg(long)]
        threads: Option<usize>,
        /// Autoriser les autres utilisateurs à lire le montage.
        #[arg(long)]
        allow_other: bool,
        /// Morceaux décompressés gardés en cache (défaut : 64 ; 0 : aucun).
        #[arg(long)]
        cache_chunks: Option<usize>,
    },
    /// Télécharge une archive .vault en vérifiant chaque morceau à l'arrivée
    /// (plages HTTP alignées sur les morceaux, reprise sans re-téléchargement).
    Fetch {
        url: String,
        /// Fichier de destination (défaut : dernier segment de l'URL).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Racine d'archive attendue (hex) : refusée avant tout octet de contenu si différente.
        #[arg(long)]
        root: Option<String>,
        /// Connexions parallèles.
        #[arg(long, default_value_t = vault_fetch::DEFAULT_CONNECTIONS)]
        connections: usize,
        /// Débit maximal, ex. 5M, 512k (octets/s).
        #[arg(long)]
        limit: Option<String>,
        /// Ne pas télécharger la section de parité.
        #[arg(long)]
        no_parity: bool,
        /// Afficher le plan sans rien écrire.
        #[arg(long)]
        plan: bool,
        /// Ignorer destination et état de reprise existants.
        #[arg(long)]
        restart: bool,
        /// Pas de progression sur stderr.
        #[arg(long, short)]
        quiet: bool,
    },
    /// Résume une archive sans rien décompresser : format, codec, taille
    /// logique, déduplication, métadonnées, parité, ratio et racine.
    Info {
        archive: PathBuf,
        /// Sortie JSON (schéma `tholos-info/1`) pour les scripts et les apps.
        #[arg(long)]
        json: bool,
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
        Cmd::Verify {
            archive,
            quick,
            sample,
            json,
        } => verify::run(&archive, quick, sample, json)?,
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
        Cmd::Extract {
            archive,
            out_dir,
            paths,
        } => extract::run(&archive, &out_dir, &paths)?,
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
        Cmd::Mount {
            archive,
            mountpoint,
            threads,
            allow_other,
            cache_chunks,
        } => mount(&archive, &mountpoint, threads, allow_other, cache_chunks)?,
        Cmd::Fetch {
            url,
            out,
            root,
            connections,
            limit,
            no_parity,
            plan,
            restart,
            quiet,
        } => fetch::run(fetch::FetchArgs {
            url,
            out,
            root,
            connections,
            limit,
            no_parity,
            plan,
            restart,
            quiet,
        })?,
        Cmd::Info { archive, json } => info::run(&archive, json)?,
    }
    Ok(())
}

fn mount(
    archive: &Path,
    mountpoint: &Path,
    threads: Option<usize>,
    allow_other: bool,
    cache_chunks: Option<usize>,
) -> Result<()> {
    let backend = vault_mount::probe().map_err(|e| anyhow::anyhow!("{e}"))?;
    let fs =
        VaultFs::open(archive).with_context(|| format!("ouverture de {}", archive.display()))?;
    let opts = MountOptions {
        threads,
        allow_other,
        cache_chunks,
        ..MountOptions::default()
    };
    let handle = vault_mount::mount(fs, mountpoint, &opts)
        .with_context(|| format!("montage sur {}", mountpoint.display()))?;
    eprintln!(
        "{} monté sur {} ({backend}) ; Ctrl-C pour démonter",
        archive.display(),
        mountpoint.display()
    );
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })
    .context("installation du gestionnaire Ctrl-C")?;
    while handle.is_alive() {
        if rx.recv_timeout(Duration::from_millis(500)).is_ok() {
            break;
        }
    }
    handle.unmount().context("démontage")?;
    eprintln!("démonté");
    Ok(())
}

fn open(p: &Path) -> Result<Archive> {
    Archive::open(p).with_context(|| format!("ouverture de {}", p.display()))
}
