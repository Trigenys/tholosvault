//! Compression par morceau. Adopté : crate `zstd`. Le déterminisme de l'encodage
//! est un contrat du format (L-001) : la version de zstd et le niveau sont
//! enregistrés dans l'en-tête et vérifiés par test de non-régression.

use crate::error::{Error, Result};

/// Identifiant de codec inscrit dans l'en-tête.
pub const CODEC_ZSTD: u8 = 1;

/// Niveau zstd par défaut (plage recommandée par le rapport : 15 à 19).
pub const DEFAULT_LEVEL: i32 = 15;

/// Version numérique de la bibliothèque zstd liée (ex. 10507 pour 1.5.7).
pub fn zstd_version() -> u32 {
    zstd::zstd_safe::version_number()
}

/// Compresse un morceau en une frame zstd autonome (taille d'origine incluse).
pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>> {
    zstd::bulk::compress(data, level).map_err(Error::Compress)
}

/// Décompresse une frame vers exactement `expected_len` octets.
pub fn decompress(frame: &[u8], expected_len: usize, index: u32) -> Result<Vec<u8>> {
    let out = zstd::bulk::decompress(frame, expected_len)
        .map_err(|source| Error::Decompress { index, source })?;
    if out.len() != expected_len {
        return Err(Error::Decompress {
            index,
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "longueur décompressée {} différente de l'attendu {}",
                    out.len(),
                    expected_len
                ),
            ),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_deterministic() {
        let data: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        let a = compress(&data, DEFAULT_LEVEL).unwrap();
        let b = compress(&data, DEFAULT_LEVEL).unwrap();
        assert_eq!(a, b, "même entrée, mêmes octets (L-001)");
        assert!(a.len() < data.len());
        assert_eq!(decompress(&a, data.len(), 0).unwrap(), data);
    }

    #[test]
    fn wrong_length_is_an_error() {
        let frame = compress(b"bonjour", 3).unwrap();
        assert!(decompress(&frame, 3, 0).is_err());
    }
}
