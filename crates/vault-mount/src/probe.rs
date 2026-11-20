//! Détection du pilote de montage disponible, sans rien monter.

#[cfg(all(target_os = "linux", feature = "fuse"))]
use crate::error::Backend;
use crate::error::MountError;

/// Vérifie qu'un adaptateur est compilé et que son pilote est utilisable.
/// N'a aucun effet de bord. L'erreur renvoyée porte un conseil d'installation.
pub fn probe() -> Result<crate::Backend, MountError> {
    #[cfg(all(target_os = "linux", feature = "fuse"))]
    {
        linux::probe()
    }
    #[cfg(not(all(target_os = "linux", feature = "fuse")))]
    {
        Err(not_compiled())
    }
}

pub(crate) fn not_compiled() -> MountError {
    MountError::NotCompiled {
        hint: if cfg!(windows) {
            "adaptateur WinFsp à venir ; utiliser Linux avec la feature `fuse`"
        } else if cfg!(target_os = "linux") {
            "recompiler vault-mount avec la feature `fuse`"
        } else {
            "aucun adaptateur pour cet OS ; cibles prévues : Linux (FUSE), Windows (WinFsp)"
        },
    }
}

#[cfg(all(target_os = "linux", feature = "fuse"))]
mod linux {
    use std::fs::OpenOptions;
    use std::path::PathBuf;

    use super::*;

    const HINT: &str = "installer fuse3 (ex. `apt install fuse3`) et vérifier l'accès à /dev/fuse";

    pub fn probe() -> Result<Backend, MountError> {
        if let Err(e) = OpenOptions::new().read(true).write(true).open("/dev/fuse") {
            return Err(MountError::DriverUnavailable {
                backend: Backend::Fuse,
                detail: format!("/dev/fuse inaccessible : {e}"),
                hint: HINT,
            });
        }
        if fusermount().is_none() {
            return Err(MountError::DriverUnavailable {
                backend: Backend::Fuse,
                detail: "fusermount3 introuvable (FUSERMOUNT_PATH, PATH, /sbin, /usr/bin)".into(),
                hint: HINT,
            });
        }
        Ok(Backend::Fuse)
    }

    /// Même règle de recherche que fuser (mode pur Rust).
    fn fusermount() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("FUSERMOUNT_PATH") {
            let p = PathBuf::from(p);
            return p.is_file().then_some(p);
        }
        let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        dirs.extend(["/sbin", "/usr/bin", "/usr/sbin"].map(PathBuf::from));
        dirs.into_iter()
            .flat_map(|d| [d.join("fusermount3"), d.join("fusermount")])
            .find(|p| p.is_file())
    }
}
