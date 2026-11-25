//! Adaptateur FUSE (Linux) : traduit les requêtes du noyau vers [`VaultFs`]
//! et les [`FsError`] vers des errno. Aucune logique de format ici.

use std::cell::RefCell;
use std::ffi::OsStr;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use fuser::{
    BackgroundSession, Config, Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags,
    Generation, INodeNo, InitFlags, KernelConfig, LockOwner, MountOption, OpenAccMode, OpenFlags,
    ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, ReplyOpen, ReplyStatfs, ReplyXattr, Request,
    SessionACL,
};

use crate::{Attr, FsError, MountError, MountOptions, NodeKind, VaultFs};

const BLOCK: u32 = 4096;
const READAHEAD: u32 = 1 << 20;

thread_local! {
    /// Tampon de réponse réutilisé par fil de service : une requête de lecture
    /// n'alloue ni ne remet à zéro `size` octets à chaque appel.
    static READ_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

struct FuseFs {
    fs: Arc<VaultFs>,
    ttl: Duration,
    uid: u32,
    gid: u32,
    /// Le noyau accepte `ENOSYS` à `open` et n'envoie alors plus ni `open` ni
    /// `release` : deux allers-retours de moins par fichier. Sans effet sur
    /// la sûreté : le montage est `ro` (le VFS rejette toute ouverture en
    /// écriture) et un fichier n'a pas de descripteur côté démon.
    no_open: AtomicBool,
}

fn errno(e: FsError) -> Errno {
    match e {
        FsError::NotFound => Errno::ENOENT,
        FsError::NotDir => Errno::ENOTDIR,
        FsError::IsDir => Errno::EISDIR,
        FsError::Corrupt(_) => Errno::EIO,
    }
}

/// Protocole xattr : `size == 0` demande la taille ; sinon la valeur doit
/// tenir dans `size` octets, faute de quoi `ERANGE`.
fn reply_xattr(reply: ReplyXattr, size: u32, value: &[u8]) {
    let Ok(len) = u32::try_from(value.len()) else {
        return reply.error(Errno::ERANGE);
    };
    if size == 0 {
        reply.size(len);
    } else if len <= size {
        reply.data(value);
    } else {
        reply.error(Errno::ERANGE);
    }
}

fn kind(k: NodeKind) -> FileType {
    match k {
        NodeKind::Dir => FileType::Directory,
        NodeKind::File => FileType::RegularFile,
    }
}

impl FuseFs {
    fn attr(&self, a: Attr) -> FileAttr {
        FileAttr {
            ino: INodeNo(a.ino),
            size: a.size,
            blocks: a.size.div_ceil(512),
            atime: a.mtime,
            mtime: a.mtime,
            ctime: a.mtime,
            crtime: a.mtime,
            kind: kind(a.kind),
            perm: match a.kind {
                NodeKind::Dir => 0o555,
                NodeKind::File => 0o444,
            },
            nlink: a.nlink,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: BLOCK,
            flags: 0,
        }
    }
}

impl Filesystem for FuseFs {
    fn init(&mut self, _req: &Request, config: &mut KernelConfig) -> std::io::Result<()> {
        if let Err(max) = config.set_max_readahead(READAHEAD) {
            let _ = config.set_max_readahead(max);
        }
        let caps = config.capabilities();
        self.no_open.store(
            caps.contains(InitFlags::FUSE_NO_OPEN_SUPPORT),
            Ordering::Relaxed,
        );
        if caps.contains(InitFlags::FUSE_PARALLEL_DIROPS) {
            let _ = config.add_capabilities(InitFlags::FUSE_PARALLEL_DIROPS);
        }
        Ok(())
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let Some(name) = name.to_str() else {
            return reply.error(Errno::ENOENT);
        };
        match self.fs.lookup(parent.0, name) {
            Ok(a) => reply.entry(&self.ttl, &self.attr(a), Generation(0)),
            Err(e) => reply.error(errno(e)),
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        match self.fs.getattr(ino.0) {
            Ok(a) => reply.attr(&self.ttl, &self.attr(a)),
            Err(e) => reply.error(errno(e)),
        }
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        if self.no_open.load(Ordering::Relaxed) {
            return reply.error(Errno::ENOSYS);
        }
        if !matches!(flags.acc_mode(), OpenAccMode::O_RDONLY) {
            return reply.error(Errno::EROFS);
        }
        match self.fs.getattr(ino.0) {
            Ok(a) if a.kind == NodeKind::Dir => reply.error(Errno::EISDIR),
            Ok(_) => reply.opened(FileHandle(0), FopenFlags::FOPEN_KEEP_CACHE),
            Err(e) => reply.error(errno(e)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        READ_BUF.with_borrow_mut(|buf| {
            buf.resize(size as usize, 0);
            match self.fs.read(ino.0, offset, buf) {
                Ok(n) => reply.data(&buf[..n]),
                Err(e) => reply.error(errno(e)),
            }
        });
    }

    fn opendir(&self, _req: &Request, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
        match self.fs.getattr(ino.0) {
            Ok(a) if a.kind == NodeKind::File => reply.error(Errno::ENOTDIR),
            Ok(_) => reply.opened(FileHandle(0), FopenFlags::FOPEN_CACHE_DIR),
            Err(e) => reply.error(errno(e)),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        match self.fs.readdir(ino.0, offset) {
            Ok(entries) => {
                for e in entries {
                    if reply.add(INodeNo(e.ino), e.next_offset, kind(e.kind), e.name) {
                        break;
                    }
                }
                reply.ok();
            }
            Err(e) => reply.error(errno(e)),
        }
    }

    fn getxattr(&self, _req: &Request, ino: INodeNo, name: &OsStr, size: u32, reply: ReplyXattr) {
        let Some(name) = name.to_str() else {
            return reply.error(Errno::NO_XATTR);
        };
        match self.fs.xattr(ino.0, name) {
            Ok(Some(v)) => reply_xattr(reply, size, v.as_bytes()),
            Ok(None) => reply.error(Errno::NO_XATTR),
            Err(e) => reply.error(errno(e)),
        }
    }

    fn listxattr(&self, _req: &Request, ino: INodeNo, size: u32, reply: ReplyXattr) {
        match self.fs.xattr_names(ino.0) {
            Ok(names) => {
                let mut list = Vec::new();
                for n in names {
                    list.extend_from_slice(n.as_bytes());
                    list.push(0);
                }
                reply_xattr(reply, size, &list);
            }
            Err(e) => reply.error(errno(e)),
        }
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        let st = self.fs.statfs();
        let blocks = st.bytes.div_ceil(u64::from(BLOCK));
        reply.statfs(blocks, 0, 0, st.files, 0, BLOCK, 255, BLOCK);
    }
}

pub(crate) fn spawn(
    fs: Arc<VaultFs>,
    mountpoint: &Path,
    opts: &MountOptions,
) -> Result<BackgroundSession, MountError> {
    let io = |source| MountError::Io {
        path: mountpoint.to_path_buf(),
        source,
    };
    let meta = std::fs::metadata(mountpoint).map_err(io)?;
    let threads = opts.threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });
    let mut config = Config::default();
    config.mount_options = vec![
        MountOption::RO,
        MountOption::FSName("tholosvault".into()),
        MountOption::Subtype("tholos".into()),
        MountOption::DefaultPermissions,
    ];
    config.n_threads = Some(threads.max(1));
    if opts.allow_other {
        config.acl = SessionACL::All;
        config.mount_options.push(MountOption::AutoUnmount);
    }
    let fuse = FuseFs {
        fs,
        ttl: opts.ttl,
        uid: meta.uid(),
        gid: meta.gid(),
        no_open: AtomicBool::new(false),
    };
    fuser::spawn_mount(fuse, mountpoint, &config).map_err(io)
}
