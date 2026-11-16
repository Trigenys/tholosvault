//! Agencement binaire du format v0. Tous les entiers sont little-endian.
//! Les sections de métadonnées sont en clair (L-002) et lisibles par mmap.
//!
//! ```text
//! [en-tête 64 o][frames zstd…][index morceaux][table fichiers][table hachage][trailer 128 o][trailer 128 o]
//! ```

use crate::error::{Error, Result};
use crate::merkle::Hash;

pub const MAGIC: [u8; 8] = *b"THOLOSVT";
pub const TRAILER_MAGIC: [u8; 8] = *b"THOLOSTR";
pub const FORMAT_VERSION: u16 = 0;
pub const HEADER_LEN: usize = 64;
pub const TRAILER_LEN: usize = 128;
pub const TRAILER_COPIES: usize = 2;
pub const CHUNK_ENTRY_LEN: usize = 48;
pub const HASH_ENTRY_LEN: usize = 40;
pub const MAX_PATH_LEN: usize = u16::MAX as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub format_version: u16,
    pub flags: u16,
    pub codec_id: u8,
    pub codec_level: i8,
    pub codec_version: u32,
    pub chunk_min: u32,
    pub chunk_avg: u32,
    pub chunk_max: u32,
    /// Identifiant du dictionnaire (zéros = aucun). Réservé pour une version future.
    pub dict_id: Hash,
}

impl Header {
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        b[0..8].copy_from_slice(&MAGIC);
        b[8..10].copy_from_slice(&self.format_version.to_le_bytes());
        b[10..12].copy_from_slice(&self.flags.to_le_bytes());
        b[12] = self.codec_id;
        b[13] = self.codec_level as u8;
        b[16..20].copy_from_slice(&self.codec_version.to_le_bytes());
        b[20..24].copy_from_slice(&self.chunk_min.to_le_bytes());
        b[24..28].copy_from_slice(&self.chunk_avg.to_le_bytes());
        b[28..32].copy_from_slice(&self.chunk_max.to_le_bytes());
        b[32..64].copy_from_slice(&self.dict_id);
        b
    }

    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() < HEADER_LEN {
            return Err(Error::BadHeader("trop court"));
        }
        if b[0..8] != MAGIC {
            return Err(Error::BadHeader("nombre magique absent"));
        }
        let format_version = u16_at(b, 8);
        if format_version != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion {
                found: format_version,
                expected: FORMAT_VERSION,
            });
        }
        let mut dict_id = [0u8; 32];
        dict_id.copy_from_slice(&b[32..64]);
        Ok(Self {
            format_version,
            flags: u16_at(b, 10),
            codec_id: b[12],
            codec_level: b[13] as i8,
            codec_version: u32_at(b, 16),
            chunk_min: u32_at(b, 20),
            chunk_avg: u32_at(b, 24),
            chunk_max: u32_at(b, 28),
            dict_id,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trailer {
    pub chunk_index_off: u64,
    pub chunk_index_len: u64,
    pub file_table_off: u64,
    pub file_table_len: u64,
    pub hash_table_off: u64,
    pub hash_table_len: u64,
    pub chunk_count: u64,
    pub file_count: u64,
    pub archive_root: Hash,
    pub data_len: u64,
}

impl Trailer {
    pub fn encode(&self) -> [u8; TRAILER_LEN] {
        let mut b = [0u8; TRAILER_LEN];
        b[0..8].copy_from_slice(&TRAILER_MAGIC);
        b[8..16].copy_from_slice(&self.chunk_index_off.to_le_bytes());
        b[16..24].copy_from_slice(&self.chunk_index_len.to_le_bytes());
        b[24..32].copy_from_slice(&self.file_table_off.to_le_bytes());
        b[32..40].copy_from_slice(&self.file_table_len.to_le_bytes());
        b[40..48].copy_from_slice(&self.hash_table_off.to_le_bytes());
        b[48..56].copy_from_slice(&self.hash_table_len.to_le_bytes());
        b[56..64].copy_from_slice(&self.chunk_count.to_le_bytes());
        b[64..72].copy_from_slice(&self.file_count.to_le_bytes());
        b[72..104].copy_from_slice(&self.archive_root);
        b[104..112].copy_from_slice(&self.data_len.to_le_bytes());
        let crc = crc32c::crc32c(&b[..TRAILER_LEN - 4]);
        b[TRAILER_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
        b
    }

    /// `None` si la copie est corrompue (magique ou CRC32C invalide).
    pub fn decode(b: &[u8]) -> Option<Self> {
        if b.len() != TRAILER_LEN || b[0..8] != TRAILER_MAGIC {
            return None;
        }
        let crc = u32_at(b, TRAILER_LEN - 4);
        if crc32c::crc32c(&b[..TRAILER_LEN - 4]) != crc {
            return None;
        }
        let mut archive_root = [0u8; 32];
        archive_root.copy_from_slice(&b[72..104]);
        Some(Self {
            chunk_index_off: u64_at(b, 8),
            chunk_index_len: u64_at(b, 16),
            file_table_off: u64_at(b, 24),
            file_table_len: u64_at(b, 32),
            hash_table_off: u64_at(b, 40),
            hash_table_len: u64_at(b, 48),
            chunk_count: u64_at(b, 56),
            file_count: u64_at(b, 64),
            archive_root,
            data_len: u64_at(b, 104),
        })
    }
}

pub(crate) fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

pub(crate) fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}

pub(crate) fn u64_at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = Header {
            format_version: FORMAT_VERSION,
            flags: 0,
            codec_id: 1,
            codec_level: -3,
            codec_version: 10507,
            chunk_min: 1,
            chunk_avg: 2,
            chunk_max: 3,
            dict_id: [7u8; 32],
        };
        assert_eq!(Header::decode(&h.encode()).unwrap(), h);
    }

    #[test]
    fn trailer_detects_corruption() {
        let t = Trailer {
            chunk_index_off: 1,
            chunk_index_len: 2,
            file_table_off: 3,
            file_table_len: 4,
            hash_table_off: 5,
            hash_table_len: 6,
            chunk_count: 7,
            file_count: 8,
            archive_root: [9u8; 32],
            data_len: 10,
        };
        let mut b = t.encode();
        assert_eq!(Trailer::decode(&b), Some(t));
        b[20] ^= 0xff;
        assert_eq!(Trailer::decode(&b), None);
    }

    #[test]
    fn wrong_version_is_rejected() {
        let mut b = Header {
            format_version: FORMAT_VERSION + 1,
            flags: 0,
            codec_id: 1,
            codec_level: 1,
            codec_version: 0,
            chunk_min: 0,
            chunk_avg: 0,
            chunk_max: 0,
            dict_id: [0; 32],
        }
        .encode();
        assert!(matches!(
            Header::decode(&b),
            Err(Error::UnsupportedVersion { .. })
        ));
        b[0] = b'X';
        assert!(matches!(Header::decode(&b), Err(Error::BadHeader(_))));
    }
}
