//! Adaptateur WinFsp (Windows) : traduit les requêtes du pilote vers
//! [`VaultFs`] et les [`FsError`] vers des NTSTATUS. Aucune logique de format ici.
//!
//! Le volume est déclaré en lecture seule et insensible à la casse (comportement
//! attendu par l'Explorateur et les outils Windows) ; la casse d'origine est
//! préservée. Le cœur restant sensible à la casse, la correspondance
//! insensible se fait ici, composant par composant, sans toucher au cœur.

use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    LocalFree, HLOCAL, STATUS_END_OF_FILE, STATUS_FILE_IS_A_DIRECTORY, STATUS_IO_DEVICE_ERROR,
    STATUS_MEDIA_WRITE_PROTECTED, STATUS_NOT_A_DIRECTORY, STATUS_OBJECT_NAME_NOT_FOUND,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{GetSecurityDescriptorLength, PSECURITY_DESCRIPTOR};
use windows::Win32::Storage::FileSystem::{
    DELETE, FILE_APPEND_DATA, FILE_ATTRIBUTE_ARCHIVE, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_READONLY, FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA, FILE_WRITE_EA, WRITE_DAC,
    WRITE_OWNER,
};
use winfsp::filesystem::{
    DirInfo, DirMarker, FileInfo, FileSecurity, FileSystemContext, OpenFileInfo, VolumeInfo,
    WideNameInfo,
};
use winfsp::host::{FileSystemHost, VolumeParams};
use winfsp::{FspError, U16CStr};

type WinError = windows::core::Error;

use crate::{Attr, FsError, Ino, MountError, MountOptions, NodeKind, VaultFs, ROOT_INO};

const SECTOR: u16 = 4096;
const VOLUME_LABEL: &str = "tholosvault";
/// Propriétaire et groupe : Administrateurs ; tout le monde : lecture et exécution.
const SDDL: &str = "O:BAG:BAD:P(A;;FRFX;;;WD)";
/// Écart entre l'époque Windows (1601) et l'époque Unix, en unités de 100 ns.
const EPOCH_DIFF_100NS: u64 = 116_444_736_000_000_000;
const WRITE_BITS: u32 = FILE_WRITE_DATA.0
    | FILE_APPEND_DATA.0
    | FILE_WRITE_ATTRIBUTES.0
    | FILE_WRITE_EA.0
    | DELETE.0
    | WRITE_DAC.0
    | WRITE_OWNER.0;

fn ntstatus(e: FsError) -> FspError {
    match e {
        FsError::NotFound => STATUS_OBJECT_NAME_NOT_FOUND.into(),
        FsError::NotDir => STATUS_NOT_A_DIRECTORY.into(),
        FsError::IsDir => STATUS_FILE_IS_A_DIRECTORY.into(),
        FsError::Corrupt(_) => STATUS_IO_DEVICE_ERROR.into(),
    }
}

fn attributes(kind: NodeKind) -> u32 {
    match kind {
        NodeKind::Dir => (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_READONLY).0,
        NodeKind::File => (FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_ARCHIVE).0,
    }
}

fn filetime(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| (d.as_nanos() / 100) as u64 + EPOCH_DIFF_100NS)
        .unwrap_or(EPOCH_DIFF_100NS)
}

struct Resolved {
    attr: Attr,
    /// Chemin tel qu'il existe dans l'archive (casse d'origine), séparé par `\`.
    normalized: Vec<u16>,
}

struct WinFspFs {
    fs: Arc<VaultFs>,
    security: Vec<u8>,
    mtime: u64,
}

impl WinFspFs {
    fn file_info(&self, a: Attr) -> FileInfo {
        FileInfo {
            file_attributes: attributes(a.kind),
            reparse_tag: 0,
            allocation_size: a.size.div_ceil(u64::from(SECTOR)) * u64::from(SECTOR),
            file_size: a.size,
            creation_time: self.mtime,
            last_access_time: self.mtime,
            last_write_time: self.mtime,
            change_time: self.mtime,
            index_number: a.ino,
            hard_links: a.nlink,
            ea_size: 0,
        }
    }

    /// Résout un composant : correspondance exacte d'abord, puis insensible à la casse.
    fn child(&self, parent: Ino, name: &str) -> Result<(Attr, String), FsError> {
        if let Ok(a) = self.fs.lookup(parent, name) {
            return Ok((a, name.to_owned()));
        }
        let wanted = name.to_lowercase();
        let found = self
            .fs
            .readdir(parent, 2)?
            .find(|e| e.name.to_lowercase() == wanted)
            .map(|e| (e.ino, e.name.to_owned()));
        let (ino, actual) = found.ok_or(FsError::NotFound)?;
        Ok((self.fs.getattr(ino)?, actual))
    }

    fn resolve(&self, file_name: &U16CStr) -> Result<Resolved, FspError> {
        let path = file_name.to_string_lossy();
        let mut ino = ROOT_INO;
        let mut normalized: Vec<u16> = Vec::new();
        for part in path.split(['\\', '/']).filter(|p| !p.is_empty()) {
            let (a, actual) = self.child(ino, part).map_err(ntstatus)?;
            normalized.push(u16::from(b'\\'));
            normalized.extend(actual.encode_utf16());
            ino = a.ino;
        }
        if normalized.is_empty() {
            normalized.push(u16::from(b'\\'));
        }
        let attr = self.fs.getattr(ino).map_err(ntstatus)?;
        Ok(Resolved { attr, normalized })
    }

    /// Recopie le descripteur si le tampon est assez grand ; renvoie toujours sa taille.
    fn copy_security(&self, out: Option<&mut [c_void]>) -> u64 {
        let len = self.security.len();
        if let Some(buf) = out {
            if buf.len() >= len {
                // SAFETY : `buf` fait au moins `len` octets ; recopie d'octets opaques.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        self.security.as_ptr(),
                        buf.as_mut_ptr().cast::<u8>(),
                        len,
                    );
                }
            }
        }
        len as u64
    }
}

impl FileSystemContext for WinFspFs {
    type FileContext = Attr;

    fn get_security_by_name(
        &self,
        file_name: &U16CStr,
        security_descriptor: Option<&mut [c_void]>,
        _reparse_point_resolver: impl FnOnce(&U16CStr) -> Option<FileSecurity>,
    ) -> winfsp::Result<FileSecurity> {
        let r = self.resolve(file_name)?;
        Ok(FileSecurity {
            reparse: false,
            sz_security_descriptor: self.copy_security(security_descriptor),
            attributes: attributes(r.attr.kind),
        })
    }

    fn open(
        &self,
        file_name: &U16CStr,
        _create_options: u32,
        granted_access: u32,
        file_info: &mut OpenFileInfo,
    ) -> winfsp::Result<Self::FileContext> {
        if granted_access & WRITE_BITS != 0 {
            return Err(STATUS_MEDIA_WRITE_PROTECTED.into());
        }
        let r = self.resolve(file_name)?;
        *file_info.as_mut() = self.file_info(r.attr);
        file_info.set_normalized_name(&r.normalized, None);
        Ok(r.attr)
    }

    fn close(&self, _context: Self::FileContext) {}

    #[allow(clippy::too_many_arguments)]
    fn create(
        &self,
        _file_name: &U16CStr,
        _create_options: u32,
        _granted_access: u32,
        _file_attributes: u32,
        _security_descriptor: Option<&[c_void]>,
        _allocation_size: u64,
        _extra_buffer: Option<&[u8]>,
        _extra_buffer_is_reparse_point: bool,
        _file_info: &mut OpenFileInfo,
    ) -> winfsp::Result<Self::FileContext> {
        Err(STATUS_MEDIA_WRITE_PROTECTED.into())
    }

    fn write(
        &self,
        _context: &Self::FileContext,
        _buffer: &[u8],
        _offset: u64,
        _write_to_eof: bool,
        _constrained_io: bool,
        _file_info: &mut FileInfo,
    ) -> winfsp::Result<u32> {
        Err(STATUS_MEDIA_WRITE_PROTECTED.into())
    }

    fn get_file_info(
        &self,
        context: &Self::FileContext,
        file_info: &mut FileInfo,
    ) -> winfsp::Result<()> {
        let a = self.fs.getattr(context.ino).map_err(ntstatus)?;
        *file_info = self.file_info(a);
        Ok(())
    }

    fn get_security(
        &self,
        _context: &Self::FileContext,
        security_descriptor: Option<&mut [c_void]>,
    ) -> winfsp::Result<u64> {
        Ok(self.copy_security(security_descriptor))
    }

    fn read(
        &self,
        context: &Self::FileContext,
        buffer: &mut [u8],
        offset: u64,
    ) -> winfsp::Result<u32> {
        if context.kind == NodeKind::Dir {
            return Err(STATUS_FILE_IS_A_DIRECTORY.into());
        }
        if offset >= context.size {
            return Err(STATUS_END_OF_FILE.into());
        }
        let n = self
            .fs
            .read(context.ino, offset, buffer)
            .map_err(ntstatus)?;
        if n == 0 {
            return Err(STATUS_END_OF_FILE.into());
        }
        Ok(n as u32)
    }

    fn read_directory(
        &self,
        context: &Self::FileContext,
        _pattern: Option<&U16CStr>,
        marker: DirMarker,
        buffer: &mut [u8],
    ) -> winfsp::Result<u32> {
        if context.kind != NodeKind::Dir {
            return Err(STATUS_NOT_A_DIRECTORY.into());
        }
        // La racine n'expose ni `.` ni `..` (convention WinFsp) ; les autres
        // répertoires les listent en tête, comme le cœur les produit.
        let start = if context.ino == ROOT_INO { 2 } else { 0 };
        let entries = self.fs.readdir(context.ino, start).map_err(ntstatus)?;
        let marker: Option<String> = marker.inner_as_cstr().map(|m| m.to_string_lossy());
        let mut skipping = marker.is_some();
        let mut cursor = 0u32;
        let mut info: DirInfo<255> = DirInfo::new();
        for e in entries {
            if skipping {
                if marker.as_deref() == Some(e.name) {
                    skipping = false;
                }
                continue;
            }
            let Ok(attr) = self.fs.getattr(e.ino) else {
                continue;
            };
            info.reset();
            *info.file_info_mut() = self.file_info(attr);
            info.set_name(OsStr::new(e.name))?;
            if !info.append_to_buffer(buffer, &mut cursor) {
                return Ok(cursor);
            }
        }
        DirInfo::<255>::finalize_buffer(buffer, &mut cursor);
        Ok(cursor)
    }

    fn get_volume_info(&self, out: &mut VolumeInfo) -> winfsp::Result<()> {
        let st = self.fs.statfs();
        out.total_size = st.bytes.div_ceil(u64::from(SECTOR)) * u64::from(SECTOR);
        out.free_size = 0;
        out.set_volume_label(VOLUME_LABEL);
        Ok(())
    }
}

/// Descripteur de sécurité autorelatif construit depuis [`SDDL`].
fn security_descriptor() -> Result<Vec<u8>, std::io::Error> {
    let wide: Vec<u16> = OsStr::new(SDDL).encode_wide().chain([0]).collect();
    let mut desc = PSECURITY_DESCRIPTOR::default();
    let mut size = 0u32;
    // SAFETY : chaîne terminée par 0 ; la fonction alloue `desc` via LocalAlloc.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(wide.as_ptr()),
            SDDL_REVISION_1,
            &mut desc,
            Some(&mut size),
        )
        .map_err(|e| std::io::Error::other(e.message()))?;
    }
    if desc.0.is_null() {
        return Err(std::io::Error::other("descripteur de sécurité nul"));
    }
    // SAFETY : `desc` est un descripteur autorelatif valide de `len` octets,
    // libéré une seule fois après copie.
    let bytes = unsafe {
        let len = GetSecurityDescriptorLength(desc) as usize;
        let b = std::slice::from_raw_parts(desc.0.cast::<u8>(), len).to_vec();
        let _ = LocalFree(Some(HLOCAL(desc.0)));
        b
    };
    Ok(bytes)
}

/// Montage actif côté WinFsp. Le laisser tomber démonte et arrête le répartiteur.
pub(crate) struct Session {
    host: FileSystemHost<WinFspFs>,
    alive: Arc<AtomicBool>,
}

impl Session {
    pub(crate) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub(crate) fn unmount(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        if self.alive.swap(false, Ordering::AcqRel) {
            self.host.unmount();
            self.host.stop();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Lettre de lecteur (`X:`) ou dossier vide ou inexistant. WinFsp attend `X:`
/// sans barre finale ; il crée le dossier lui-même et le supprime au démontage.
fn mount_target(mountpoint: &Path) -> Result<String, MountError> {
    let io = |path: &Path, source: std::io::Error| MountError::Io {
        path: path.to_path_buf(),
        source,
    };
    let other = |msg: &str| io(mountpoint, std::io::Error::other(msg.to_owned()));
    let s = mountpoint
        .to_str()
        .ok_or_else(|| other("chemin non UTF-8"))?;
    let drive = s.trim_end_matches(['\\', '/']);
    if drive.len() == 2 && drive.ends_with(':') && drive.as_bytes()[0].is_ascii_alphabetic() {
        return Ok(drive.to_ascii_uppercase());
    }
    let abs = std::path::absolute(mountpoint).map_err(|e| io(mountpoint, e))?;
    if abs.exists() {
        if !abs.is_dir() {
            return Err(other("le point de montage n'est pas un dossier"));
        }
        let mut entries = std::fs::read_dir(&abs).map_err(|e| io(&abs, e))?;
        if entries.next().is_some() {
            return Err(other("le dossier de montage doit être vide"));
        }
        // WinFsp crée lui-même le dossier (point de reparse) et refuse un dossier
        // existant (STATUS_OBJECT_NAME_COLLISION) : on retire le dossier vide.
        std::fs::remove_dir(&abs).map_err(|e| io(&abs, e))?;
    }
    abs.to_str()
        .map(str::to_owned)
        .ok_or_else(|| other("chemin non UTF-8"))
}

pub(crate) fn spawn(
    fs: Arc<VaultFs>,
    mountpoint: &Path,
    opts: &MountOptions,
) -> Result<Session, MountError> {
    let fsp = |e: WinError| MountError::Io {
        path: mountpoint.to_path_buf(),
        source: std::io::Error::other(format!("WinFsp : {} ({:#x})", e.message(), e.code().0)),
    };
    let target = mount_target(mountpoint)?;
    let mtime = fs
        .getattr(ROOT_INO)
        .map(|a| filetime(a.mtime))
        .unwrap_or(EPOCH_DIFF_100NS);
    let security = security_descriptor().map_err(|source| MountError::Io {
        path: mountpoint.to_path_buf(),
        source,
    })?;
    let threads = opts.threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });
    let ttl = u32::try_from(opts.ttl.as_millis()).unwrap_or(u32::MAX);

    let mut params = VolumeParams::new();
    params
        .sector_size(SECTOR)
        .sectors_per_allocation_unit(1)
        .max_component_length(255)
        .volume_creation_time(mtime)
        .volume_serial_number(0x7468_6f6c)
        .file_info_timeout(ttl)
        .dir_info_timeout(ttl)
        .volume_info_timeout(ttl)
        .security_timeout(ttl)
        .case_sensitive_search(false)
        .case_preserved_names(true)
        .unicode_on_disk(true)
        .persistent_acls(true)
        .read_only_volume(true)
        .post_cleanup_when_modified_only(true)
        .flush_and_purge_on_cleanup(false)
        .filesystem_name(VOLUME_LABEL);

    let ctx = WinFspFs {
        fs,
        security,
        mtime,
    };
    let mut host: FileSystemHost<WinFspFs> = FileSystemHost::new(params, ctx).map_err(fsp)?;
    host.mount(target).map_err(fsp)?;
    host.start_with_threads(u32::try_from(threads.max(1)).unwrap_or(1))
        .map_err(fsp)?;
    Ok(Session {
        host,
        alive: Arc::new(AtomicBool::new(true)),
    })
}
