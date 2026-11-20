//! Détection du pilote de montage disponible, sans rien monter.

#[cfg(any(
    all(target_os = "linux", feature = "fuse"),
    all(windows, feature = "winfsp")
))]
use crate::error::Backend;
use crate::error::MountError;

/// Vérifie qu'un adaptateur est compilé et que son pilote est utilisable.
/// N'a aucun effet de bord. L'erreur renvoyée porte un conseil d'installation.
pub fn probe() -> Result<crate::Backend, MountError> {
    #[cfg(all(target_os = "linux", feature = "fuse"))]
    {
        linux::probe()
    }
    #[cfg(all(windows, feature = "winfsp"))]
    {
        windows::probe()
    }
    #[cfg(not(any(
        all(target_os = "linux", feature = "fuse"),
        all(windows, feature = "winfsp")
    )))]
    {
        Err(not_compiled())
    }
}

#[cfg(not(any(
    all(target_os = "linux", feature = "fuse"),
    all(windows, feature = "winfsp")
)))]
pub(crate) fn not_compiled() -> MountError {
    MountError::NotCompiled {
        hint: if cfg!(windows) {
            "recompiler vault-mount avec la feature `winfsp`"
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

#[cfg(all(windows, feature = "winfsp"))]
mod windows {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;

    use ::windows::core::PCWSTR;
    use ::windows::Win32::System::LibraryLoader::LoadLibraryW;

    use super::*;

    pub(crate) const HINT: &str = "installer WinFsp depuis winfsp.dev";

    const DLL: &str = if cfg!(target_arch = "x86_64") {
        "winfsp-x64.dll"
    } else if cfg!(target_arch = "x86") {
        "winfsp-x86.dll"
    } else {
        "winfsp-a64.dll"
    };

    fn unavailable(detail: String) -> MountError {
        MountError::DriverUnavailable {
            backend: Backend::WinFsp,
            detail,
            hint: HINT,
        }
    }

    /// Répertoire d'installation annoncé par l'installeur WinFsp.
    fn install_dir() -> Option<PathBuf> {
        [r"SOFTWARE\WOW6432Node\WinFsp", r"SOFTWARE\WinFsp"]
            .into_iter()
            .find_map(|k| {
                windows_registry::LOCAL_MACHINE
                    .open(k)
                    .and_then(|k| k.get_string("InstallDir"))
                    .ok()
            })
            .map(PathBuf::from)
    }

    /// Vérifie l'installation de WinFsp et charge sa DLL (liaison différée) :
    /// sans pilote, le binaire a démarré et renvoie une erreur actionnable.
    pub fn probe() -> Result<Backend, MountError> {
        let dir = install_dir().ok_or_else(|| {
            unavailable(r"WinFsp absent du registre (HKLM\SOFTWARE\WinFsp)".into())
        })?;
        let dll = dir.join("bin").join(DLL);
        if !dll.is_file() {
            return Err(unavailable(format!("{} introuvable", dll.display())));
        }
        let wide: Vec<u16> = OsStr::new(&dll).encode_wide().chain([0]).collect();
        // SAFETY : chemin terminé par 0, valide pendant l'appel.
        unsafe { LoadLibraryW(PCWSTR(wide.as_ptr())) }.map_err(|e| {
            unavailable(format!("chargement de {} : {}", dll.display(), e.message()))
        })?;
        winfsp::winfsp_init().map_err(|e| unavailable(format!("initialisation WinFsp : {e}")))?;
        Ok(Backend::WinFsp)
    }
}
