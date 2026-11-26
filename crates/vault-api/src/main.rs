//! `tholos-api` : sert en lecture seule les archives `.vault` d'un répertoire.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Parser;
use vault_api::{router, ApiConfig};

#[derive(Parser)]
#[command(
    name = "tholos-api",
    version,
    about = "API HTTP en lecture seule sur un répertoire d'archives .vault (catalogue, bilan, \
             liste, lecture par plage vérifiée, vérification). Aucune authentification : \
             écoute locale par défaut ; exposer derrière un proxy inverse authentifiant."
)]
struct Args {
    /// Répertoire des archives servies (non récursif).
    root: PathBuf,
    /// Adresse d'écoute.
    #[arg(long, default_value = "127.0.0.1:7070")]
    listen: SocketAddr,
    /// Vérifications simultanées au plus (au-delà : 503 + Retry-After).
    #[arg(long, default_value_t = 2)]
    verify_slots: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if !args.root.is_dir() {
        bail!(
            "{} n'est pas un répertoire (attendu : le dossier contenant les archives .vault)",
            args.root.display()
        );
    }
    let listener = tokio::net::TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("écoute sur {} impossible", args.listen))?;
    let addr = listener.local_addr()?;
    let app = router(ApiConfig {
        root: args.root.clone(),
        verify_slots: args.verify_slots,
    });
    eprintln!(
        "tholos-api : {} servi sur http://{addr} ; Ctrl-C pour arrêter",
        args.root.display()
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .context("service HTTP")?;
    eprintln!("arrêté");
    Ok(())
}
