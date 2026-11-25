//! Transport « fichier » : récupère une archive depuis un chemin local ou
//! monté (disque externe, partage SMB/NFS, clé USB) avec la même mécanique
//! que HTTP — plages alignées sur les morceaux, vérification BLAKE3 à
//! l'arrivée, reprise par `.part` — au lieu d'une copie brute qui ne
//! vérifierait rien et repartirait de zéro après une coupure.
//!
//! Le transport ne connaît pas le format : il sert des octets par plage.
//! Il refuse une destination qui désigne le fichier source lui-même
//! (`--restart` supprimerait la source).

use std::fs::File;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;

use crate::transport::{RangeResponse, RemoteInfo, RemoteVersion, Transport, TransportError};

/// Taille des blocs lus par appel bloquant.
const BLOCK: u64 = 1 << 20;

/// Source locale d'une archive.
#[derive(Debug, Clone)]
pub struct FileTransport {
    path: PathBuf,
}

impl FileTransport {
    /// Prépare la lecture de `source` vers `dest`. Refuse une source absente
    /// ou qui n'est pas un fichier, et une destination qui est la source.
    pub fn new(source: &Path, dest: &Path) -> Result<Self, TransportError> {
        let meta = std::fs::metadata(source).map_err(|e| io_error(source, &e))?;
        if !meta.is_file() {
            return Err(TransportError::Protocol(format!(
                "{} n'est pas un fichier",
                source.display()
            )));
        }
        if same_file(source, dest) {
            return Err(TransportError::Protocol(format!(
                "la destination {} est le fichier source : choisir une autre destination (--out)",
                dest.display()
            )));
        }
        Ok(Self {
            path: source.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Reconnaît une source locale : URL `file://…` (décodée) ou chemin sans
/// schéma `http(s)://`. `None` pour une URL HTTP.
pub fn local_source(arg: &str) -> Option<PathBuf> {
    let lower = arg.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return None;
    }
    let Some(rest) = arg
        .strip_prefix("file://")
        .or_else(|| arg.strip_prefix("FILE://"))
    else {
        return Some(PathBuf::from(arg));
    };
    // file:///chemin (hôte vide) ou file://localhost/chemin.
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let decoded = percent_decode(rest);
    // file:///C:/dossier → C:/dossier sous Windows.
    let b = decoded.as_bytes();
    let path = if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        decoded[1..].to_string()
    } else {
        decoded
    };
    Some(PathBuf::from(path))
}

/// Décodage `%XX` ; une séquence invalide est conservée telle quelle.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn same_file(a: &Path, b: &Path) -> bool {
    let (Ok(ma), Ok(mb)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
        // Destination absente : ce ne peut pas être la source.
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if ma.dev() == mb.dev() && ma.ino() == mb.ino() {
            return true;
        }
    }
    let _ = (&ma, &mb);
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

fn io_error(path: &Path, e: &io::Error) -> TransportError {
    match e.kind() {
        io::ErrorKind::NotFound => TransportError::Protocol(format!(
            "source introuvable : {} (vérifier le chemin ou le montage du partage)",
            path.display()
        )),
        io::ErrorKind::PermissionDenied => TransportError::Protocol(format!(
            "lecture refusée sur {} (droits du fichier ou du partage)",
            path.display()
        )),
        _ => TransportError::Network(format!("lecture de {} : {e}", path.display())),
    }
}

/// Version d'une source locale : taille et date de modification à la
/// nanoseconde. Une réécriture du fichier invalide l'état de reprise ; les
/// morceaux restent de toute façon vérifiés un à un par BLAKE3.
fn version_of(meta: &std::fs::Metadata) -> Option<RemoteVersion> {
    let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(RemoteVersion::Strong(format!(
        "file-{}-{}",
        meta.len(),
        mtime.as_nanos()
    )))
}

fn read_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_at(buf, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        file.seek_read(buf, offset)
    }
}

#[async_trait]
impl Transport for FileTransport {
    async fn probe(&self) -> Result<RemoteInfo, TransportError> {
        let path = self.path.clone();
        let meta = tokio::task::spawn_blocking(move || std::fs::metadata(&path))
            .await
            .map_err(|e| TransportError::Network(e.to_string()))?
            .map_err(|e| io_error(&self.path, &e))?;
        Ok(RemoteInfo {
            len: Some(meta.len()),
            accepts_ranges: true,
            version: version_of(&meta),
        })
    }

    async fn get_range(&self, range: Range<u64>) -> Result<RangeResponse, TransportError> {
        let path = self.path.clone();
        let (file, meta) = tokio::task::spawn_blocking(move || {
            let f = File::open(&path)?;
            let m = f.metadata()?;
            Ok::<_, io::Error>((f, m))
        })
        .await
        .map_err(|e| TransportError::Network(e.to_string()))?
        .map_err(|e| io_error(&self.path, &e))?;
        let file = Arc::new(file);
        let label = self.path.display().to_string();
        let body = stream::unfold(range.start, move |pos| {
            let file = Arc::clone(&file);
            let label = label.clone();
            let end = range.end;
            async move {
                if pos >= end {
                    return None;
                }
                let want = (end - pos).min(BLOCK) as usize;
                let res = tokio::task::spawn_blocking(move || {
                    let mut buf = vec![0u8; want];
                    let n = read_at(&file, &mut buf, pos)?;
                    buf.truncate(n);
                    Ok::<_, io::Error>(buf)
                })
                .await;
                match res {
                    // Fin de fichier avant la fin de la plage : le flux s'arrête,
                    // le téléchargeur constate la coupure et redemande.
                    Ok(Ok(buf)) if buf.is_empty() => None,
                    Ok(Ok(buf)) => {
                        let next = pos + buf.len() as u64;
                        Some((Ok(Bytes::from(buf)), next))
                    }
                    Ok(Err(e)) => Some((
                        Err(TransportError::Network(format!("lecture de {label} : {e}"))),
                        end,
                    )),
                    Err(e) => Some((Err(TransportError::Network(e.to_string())), end)),
                }
            }
        });
        Ok(RangeResponse {
            version: version_of(&meta),
            partial: true,
            start: range.start,
            body: Box::pin(body),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_local_sources() {
        assert_eq!(local_source("https://x/a.vault"), None);
        assert_eq!(local_source("HTTP://x/a.vault"), None);
        assert_eq!(
            local_source("/mnt/nas/a.vault"),
            Some(PathBuf::from("/mnt/nas/a.vault"))
        );
        assert_eq!(
            local_source("file:///mnt/nas/mon%20archive.vault"),
            Some(PathBuf::from("/mnt/nas/mon archive.vault"))
        );
        assert_eq!(
            local_source("file://localhost/srv/a.vault"),
            Some(PathBuf::from("/srv/a.vault"))
        );
        assert_eq!(
            local_source("file:///C:/Archives/a.vault"),
            Some(PathBuf::from("C:/Archives/a.vault"))
        );
    }

    #[test]
    fn percent_decoding_keeps_invalid_sequences() {
        assert_eq!(percent_decode("a%2Fb"), "a/b");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("%C3%A9t%C3%A9"), "été");
    }
}
