use std::path::PathBuf;

/// Erreurs du système de fichiers logique. Les adaptateurs les traduisent en
/// codes natifs (errno, NTSTATUS) ; le cœur ne connaît aucun code d'OS.
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("nœud inconnu")]
    NotFound,
    #[error("le nœud n'est pas un répertoire")]
    NotDir,
    #[error("le nœud est un répertoire")]
    IsDir,
    #[error("archive corrompue : {0}")]
    Corrupt(#[from] vault_core::Error),
}

/// Erreurs d'ouverture ou de montage. Chaque variante porte un conseil
/// actionnable quand l'utilisateur peut y remédier.
#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("pilote {backend} indisponible : {detail} ({hint})")]
    DriverUnavailable {
        backend: Backend,
        detail: String,
        hint: &'static str,
    },
    #[error("aucun adaptateur de montage compilé pour cette cible ({hint})")]
    NotCompiled { hint: &'static str },
    #[error("arborescence impossible : {0}")]
    Tree(String),
    #[error(transparent)]
    Archive(#[from] vault_core::Error),
    #[error("E/S sur {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Pilote de montage natif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Fuse,
    WinFsp,
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Backend::Fuse => "FUSE",
            Backend::WinFsp => "WinFsp",
        })
    }
}
