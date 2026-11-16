//! Découpage content-defined. Adopté : `fastcdc` (variante 2020, normalisation).
//! Le découpage est déterministe pour des paramètres donnés ; les paramètres sont
//! enregistrés dans l'en-tête de l'archive (voir `format::Header`).

use std::ops::Range;

use crate::error::{Error, Result};

/// Paramètres FastCDC en octets. Par défaut 64 KiB / 128 KiB / 256 KiB,
/// plage retenue par le rapport de recherche pour le tier « chaud ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkParams {
    pub min: u32,
    pub avg: u32,
    pub max: u32,
}

impl Default for ChunkParams {
    fn default() -> Self {
        Self {
            min: 64 * 1024,
            avg: 128 * 1024,
            max: 256 * 1024,
        }
    }
}

impl ChunkParams {
    pub fn validate(&self) -> Result<()> {
        use fastcdc::v2020::{MAXIMUM_MAX, MINIMUM_MIN};
        if (self.min as usize) < MINIMUM_MIN {
            return Err(Error::BadChunkParams("min trop petit (< 64 octets)"));
        }
        if (self.max as usize) > MAXIMUM_MAX {
            return Err(Error::BadChunkParams("max trop grand (> 1 GiB)"));
        }
        if !(self.min <= self.avg && self.avg <= self.max) {
            return Err(Error::BadChunkParams("il faut min <= avg <= max"));
        }
        Ok(())
    }
}

/// Découpe `data` en plages contiguës couvrant tout le tampon, dans l'ordre.
/// Un tampon vide produit zéro plage.
pub fn split(data: &[u8], params: ChunkParams) -> Vec<Range<usize>> {
    if data.is_empty() {
        return Vec::new();
    }
    fastcdc::v2020::FastCDC::new(
        data,
        params.min as usize,
        params.avg as usize,
        params.max as usize,
    )
    .map(|c| c.offset..c.offset + c.length)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x & 0xff) as u8
            })
            .collect()
    }

    #[test]
    fn covers_whole_buffer_in_order() {
        let data = pseudo_random(3 * 1024 * 1024, 7);
        let ranges = split(&data, ChunkParams::default());
        assert!(ranges.len() > 4);
        let mut cursor = 0;
        for r in &ranges {
            assert_eq!(r.start, cursor);
            cursor = r.end;
        }
        assert_eq!(cursor, data.len());
    }

    #[test]
    fn respects_bounds_except_last() {
        let p = ChunkParams::default();
        let data = pseudo_random(4 * 1024 * 1024, 11);
        let ranges = split(&data, p);
        for r in &ranges[..ranges.len() - 1] {
            let len = (r.end - r.start) as u32;
            assert!(len >= p.min && len <= p.max, "longueur {len}");
        }
    }

    #[test]
    fn deterministic() {
        let data = pseudo_random(1024 * 1024, 3);
        assert_eq!(
            split(&data, ChunkParams::default()),
            split(&data, ChunkParams::default())
        );
    }

    #[test]
    fn empty_is_empty() {
        assert!(split(&[], ChunkParams::default()).is_empty());
    }
}
