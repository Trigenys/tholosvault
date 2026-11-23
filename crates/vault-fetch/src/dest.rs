//! Fichier de destination : créé à la taille finale (épars quand le système
//! le permet), écrit par positions depuis plusieurs tâches.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Dest {
    file: Arc<File>,
}

impl Dest {
    pub fn create(path: &Path, len: u64) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.set_len(len)?;
        Ok(Self {
            file: Arc::new(file),
        })
    }

    pub fn open_existing(path: &Path, len: u64) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let actual = file.metadata()?.len();
        if actual != len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("destination de {actual} octets, {len} attendus"),
            ));
        }
        Ok(Self {
            file: Arc::new(file),
        })
    }

    pub fn write_at(&self, offset: u64, buf: &[u8]) -> io::Result<()> {
        write_all_at(&self.file, offset, buf)
    }

    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        read_exact_at(&self.file, offset, buf)
    }

    pub fn sync(&self) -> io::Result<()> {
        self.file.sync_data()
    }
}

#[cfg(unix)]
fn write_all_at(f: &File, offset: u64, buf: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    f.write_all_at(buf, offset)
}

#[cfg(unix)]
fn read_exact_at(f: &File, offset: u64, buf: &mut [u8]) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    f.read_exact_at(buf, offset)
}

#[cfg(windows)]
fn write_all_at(f: &File, mut offset: u64, mut buf: &[u8]) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        let n = f.seek_write(buf, offset)?;
        if n == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        buf = &buf[n..];
        offset += n as u64;
    }
    Ok(())
}

#[cfg(windows)]
fn read_exact_at(f: &File, mut offset: u64, mut buf: &mut [u8]) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        let n = f.seek_read(buf, offset)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        buf = &mut buf[n..];
        offset += n as u64;
    }
    Ok(())
}
