//! Montage lecture seule d'une archive tholosvault.
//!
//! Le cœur ([`VaultFs`]) est un système de fichiers logique, indépendant de
//! l'OS : inœuds, attributs, parcours de répertoire, lecture par plage. Il se
//! teste sans pilote. Les adaptateurs natifs (FUSE derrière la feature `fuse`,
//! WinFsp derrière la feature `winfsp`) ne font que traduire les appels du
//! noyau vers ce cœur et les erreurs vers les codes natifs.
//!
//! Toute la logique de format reste dans `vault-core` : ce crate ne lit pas un
//! octet du format lui-même.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use vault_core::{Archive, CacheStats, ChunkCache};

mod error;
#[cfg(all(target_os = "linux", feature = "fuse"))]
mod fuse;
mod probe;
mod tree;
#[cfg(all(windows, feature = "winfsp"))]
mod winfsp;

pub use error::{Backend, FsError, MountError};
pub use probe::probe;

use tree::{Kind, Tree};

/// Numéro d'inœud. La racine vaut [`ROOT_INO`] ; les autres sont stables pour
/// la durée de vie du [`VaultFs`] (index de l'arbre + 1).
pub type Ino = u64;
pub const ROOT_INO: Ino = 1;

/// Attribut étendu : racine Merkle d'un fichier (hex).
pub const XATTR_ROOT: &str = "user.tholos.root";
/// Attribut étendu : nombre de morceaux d'un fichier.
pub const XATTR_CHUNKS: &str = "user.tholos.chunks";
/// Attribut étendu : racine de l'archive montée (hex), sur tout nœud.
pub const XATTR_ARCHIVE_ROOT: &str = "user.tholos.archive_root";

/// Morceaux décompressés gardés en mémoire (LRU) pour servir les lectures
/// séquentielles découpées par le noyau sans redécompresser. Dimensionné pour
/// une fenêtre de lecture anticipée de 1 MiB (jusqu'à 16 morceaux de 64 KiB)
/// servie par plusieurs fils sur plusieurs fichiers à la fois : 64 morceaux,
/// soit 16 MiB au plus avec des morceaux de 256 KiB (ADR-0004).
pub const CACHE_CHUNKS: usize = 64;

/// Compteurs des lectures servies par [`VaultFs::read`] (diagnostic, bancs).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReadStats {
    /// Appels à [`VaultFs::read`].
    pub calls: u64,
    /// Octets rendus par ces appels.
    pub bytes: u64,
    /// Compteurs du cache de morceaux.
    pub cache: CacheStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Dir,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attr {
    pub ino: Ino,
    pub kind: NodeKind,
    pub size: u64,
    pub nlink: u32,
    pub mtime: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirEntry<'a> {
    pub ino: Ino,
    pub kind: NodeKind,
    pub name: &'a str,
    /// Décalage à passer à [`VaultFs::readdir`] pour reprendre après cette entrée.
    pub next_offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatFs {
    /// Nombre de nœuds (fichiers et répertoires, racine comprise).
    pub files: u64,
    /// Somme des tailles logiques des fichiers.
    pub bytes: u64,
}

/// Système de fichiers logique, lecture seule, bâti sur une archive ouverte.
pub struct VaultFs {
    archive: Archive,
    tree: Tree,
    cache: ChunkCache,
    mtime: SystemTime,
    bytes: u64,
    read_calls: AtomicU64,
    read_bytes: AtomicU64,
}

impl VaultFs {
    /// Ouvre l'archive et construit l'arbre. La date de modification exposée
    /// pour tous les nœuds est celle du fichier d'archive.
    pub fn open(path: &Path) -> Result<Self, MountError> {
        let mtime = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let archive = Archive::open(path)?;
        Self::from_archive(archive, mtime)
    }

    pub fn from_archive(archive: Archive, mtime: SystemTime) -> Result<Self, MountError> {
        let tree = Tree::build(&archive)?;
        let bytes = archive.files().iter().map(|f| f.size).sum();
        Ok(VaultFs {
            archive,
            tree,
            cache: ChunkCache::new(CACHE_CHUNKS),
            mtime,
            bytes,
            read_calls: AtomicU64::new(0),
            read_bytes: AtomicU64::new(0),
        })
    }

    /// Remplace le cache de morceaux par un cache vide de `chunks` entrées
    /// (0 : aucune mise en cache). Les compteurs du cache repartent de zéro.
    pub fn set_cache_chunks(&mut self, chunks: usize) {
        self.cache = ChunkCache::new(chunks);
    }

    pub fn cache_chunks(&self) -> usize {
        self.cache.capacity()
    }

    /// Compteurs cumulés depuis l'ouverture (ou le dernier
    /// [`VaultFs::set_cache_chunks`] pour la partie cache).
    pub fn read_stats(&self) -> ReadStats {
        ReadStats {
            calls: self.read_calls.load(Ordering::Relaxed),
            bytes: self.read_bytes.load(Ordering::Relaxed),
            cache: self.cache.stats(),
        }
    }

    pub fn archive(&self) -> &Archive {
        &self.archive
    }

    pub fn getattr(&self, ino: Ino) -> Result<Attr, FsError> {
        let node = self.tree.node(ino).ok_or(FsError::NotFound)?;
        Ok(match node.kind {
            Kind::Dir { .. } => Attr {
                ino,
                kind: NodeKind::Dir,
                size: 0,
                nlink: 2 + self.tree.subdirs(ino),
                mtime: self.mtime,
            },
            Kind::File { file } => Attr {
                ino,
                kind: NodeKind::File,
                size: self.archive.files()[file].size,
                nlink: 1,
                mtime: self.mtime,
            },
        })
    }

    /// Résout `name` dans le répertoire `parent`.
    pub fn lookup(&self, parent: Ino, name: &str) -> Result<Attr, FsError> {
        let node = self.tree.node(parent).ok_or(FsError::NotFound)?;
        if node.node_kind() != NodeKind::Dir {
            return Err(FsError::NotDir);
        }
        let ino = self.tree.lookup(parent, name).ok_or(FsError::NotFound)?;
        self.getattr(ino)
    }

    /// Résout un chemin relatif à la racine (`a/b/c`). Chemin vide = racine.
    pub fn resolve(&self, path: &str) -> Result<Attr, FsError> {
        let mut ino = ROOT_INO;
        for part in path.split('/').filter(|p| !p.is_empty()) {
            ino = self.lookup(ino, part)?.ino;
        }
        self.getattr(ino)
    }

    /// Entrées du répertoire `ino` à partir de `offset` : `.` (0), `..` (1),
    /// puis les enfants triés par octets du nom.
    pub fn readdir(
        &self,
        ino: Ino,
        offset: u64,
    ) -> Result<impl Iterator<Item = DirEntry<'_>> + '_, FsError> {
        let node = self.tree.node(ino).ok_or(FsError::NotFound)?;
        let children = self.tree.children(ino).ok_or(FsError::NotDir)?;
        let fixed = [(".", ino), ("..", node.parent)];
        let iter = fixed
            .into_iter()
            .map(|(n, i)| (n, i, NodeKind::Dir))
            .chain(children.iter().map(move |(name, child)| {
                let kind = self
                    .tree
                    .node(*child)
                    .map(|n| n.node_kind())
                    .unwrap_or(NodeKind::File);
                (name.as_ref(), *child, kind)
            }))
            .enumerate()
            .skip(usize::try_from(offset).unwrap_or(usize::MAX))
            .map(|(i, (name, ino, kind))| DirEntry {
                ino,
                kind,
                name,
                next_offset: i as u64 + 1,
            });
        Ok(iter)
    }

    /// Lit `buf.len()` octets au plus depuis `offset`. Renvoie 0 au-delà de la fin.
    /// Un morceau corrompu renvoie `FsError::Corrupt` et n'est jamais mis en cache.
    pub fn read(&self, ino: Ino, offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
        let node = self.tree.node(ino).ok_or(FsError::NotFound)?;
        match node.kind {
            Kind::Dir { .. } => Err(FsError::IsDir),
            Kind::File { file } => {
                let n = self
                    .archive
                    .read_range_into_cached(file, offset, buf, &self.cache)?;
                self.read_calls.fetch_add(1, Ordering::Relaxed);
                self.read_bytes.fetch_add(n as u64, Ordering::Relaxed);
                Ok(n)
            }
        }
    }

    /// Noms des attributs étendus exposés par le nœud `ino`, dans un ordre
    /// stable : voir [`VaultFs::xattr`].
    pub fn xattr_names(&self, ino: Ino) -> Result<&'static [&'static str], FsError> {
        let node = self.tree.node(ino).ok_or(FsError::NotFound)?;
        Ok(match node.kind {
            Kind::File { .. } => &[XATTR_ROOT, XATTR_CHUNKS, XATTR_ARCHIVE_ROOT],
            Kind::Dir { .. } => &[XATTR_ARCHIVE_ROOT],
        })
    }

    /// Valeur d'un attribut étendu, lue dans l'index sans décompression :
    /// - `user.tholos.root` (fichier) : racine Merkle BLAKE3 du fichier, en hex ;
    ///   deux fichiers de même racine ont le même contenu, d'une archive à l'autre ;
    /// - `user.tholos.chunks` (fichier) : nombre de morceaux référencés ;
    /// - `user.tholos.archive_root` (tout nœud) : racine de l'archive montée.
    ///
    /// Les valeurs sont celles déclarées par l'index ; `tholos verify` les
    /// contrôle contre le contenu. `Ok(None)` : attribut absent de ce nœud.
    pub fn xattr(&self, ino: Ino, name: &str) -> Result<Option<String>, FsError> {
        let node = self.tree.node(ino).ok_or(FsError::NotFound)?;
        let hex = vault_core::merkle::to_hex;
        Ok(match (name, &node.kind) {
            (XATTR_ARCHIVE_ROOT, _) => Some(hex(self.archive.archive_root())),
            (XATTR_ROOT, Kind::File { file }) => Some(hex(&self.archive.files()[*file].root)),
            (XATTR_CHUNKS, Kind::File { file }) => {
                Some(self.archive.files()[*file].chunks.len().to_string())
            }
            _ => None,
        })
    }

    pub fn statfs(&self) -> StatFs {
        StatFs {
            files: self.tree.len() as u64,
            bytes: self.bytes,
        }
    }
}

/// Options de montage communes aux adaptateurs.
#[derive(Debug, Clone)]
pub struct MountOptions {
    /// Threads servant les requêtes du noyau (`None` : parallélisme disponible).
    pub threads: Option<usize>,
    /// Morceaux gardés en cache (`None` : [`CACHE_CHUNKS`] ; `Some(0)` : aucun cache).
    pub cache_chunks: Option<usize>,
    /// Autoriser les autres utilisateurs (FUSE `allow_other`).
    pub allow_other: bool,
    /// Durée de validité des attributs et entrées côté noyau. L'archive est
    /// immuable : une heure par défaut.
    pub ttl: Duration,
}

impl Default for MountOptions {
    fn default() -> Self {
        MountOptions {
            threads: None,
            cache_chunks: None,
            allow_other: false,
            ttl: Duration::from_secs(3600),
        }
    }
}

/// Montage actif. Le laisser tomber démonte au mieux ; [`MountHandle::unmount`]
/// démonte explicitement et remonte l'erreur éventuelle.
pub struct MountHandle {
    inner: handle::Inner,
    fs: Arc<VaultFs>,
}

mod handle {
    pub(super) enum Inner {
        #[cfg(all(target_os = "linux", feature = "fuse"))]
        Fuse(Option<fuser::BackgroundSession>),
        #[cfg(all(windows, feature = "winfsp"))]
        WinFsp(Option<crate::winfsp::Session>),
        #[allow(dead_code)]
        Inert,
    }
}

impl MountHandle {
    /// Le système de fichiers servi : ses compteurs ([`VaultFs::read_stats`])
    /// reflètent les requêtes du noyau.
    pub fn fs(&self) -> &VaultFs {
        &self.fs
    }

    /// Vrai tant que la session sert des requêtes.
    pub fn is_alive(&self) -> bool {
        match &self.inner {
            #[cfg(all(target_os = "linux", feature = "fuse"))]
            handle::Inner::Fuse(s) => s.as_ref().is_some_and(|s| !s.guard.is_finished()),
            #[cfg(all(windows, feature = "winfsp"))]
            handle::Inner::WinFsp(s) => s.as_ref().is_some_and(|s| s.is_alive()),
            handle::Inner::Inert => false,
        }
    }

    pub fn unmount(mut self) -> Result<(), MountError> {
        match &mut self.inner {
            #[cfg(all(target_os = "linux", feature = "fuse"))]
            handle::Inner::Fuse(s) => match s.take() {
                Some(s) => s.umount_and_join().map_err(|e| MountError::Io {
                    path: "<démontage>".into(),
                    source: e,
                }),
                None => Ok(()),
            },
            #[cfg(all(windows, feature = "winfsp"))]
            handle::Inner::WinFsp(s) => {
                if let Some(s) = s.take() {
                    s.unmount();
                }
                Ok(())
            }
            handle::Inner::Inert => Ok(()),
        }
    }
}

/// Monte `fs` sur `mountpoint` avec l'adaptateur natif compilé pour la cible.
pub fn mount(
    fs: VaultFs,
    mountpoint: &Path,
    opts: &MountOptions,
) -> Result<MountHandle, MountError> {
    let mut fs = fs;
    if let Some(n) = opts.cache_chunks {
        fs.set_cache_chunks(n);
    }
    let fs = Arc::new(fs);
    #[cfg(all(target_os = "linux", feature = "fuse"))]
    {
        let session = fuse::spawn(fs.clone(), mountpoint, opts)?;
        Ok(MountHandle {
            inner: handle::Inner::Fuse(Some(session)),
            fs,
        })
    }
    #[cfg(all(windows, feature = "winfsp"))]
    {
        let session = winfsp::spawn(fs.clone(), mountpoint, opts)?;
        Ok(MountHandle {
            inner: handle::Inner::WinFsp(Some(session)),
            fs,
        })
    }
    #[cfg(not(any(
        all(target_os = "linux", feature = "fuse"),
        all(windows, feature = "winfsp")
    )))]
    {
        let _ = (fs, mountpoint, opts);
        Err(probe::not_compiled())
    }
}
