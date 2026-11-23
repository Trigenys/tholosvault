//! État de reprise `.part` : bitmap des unités vérifiées, écrit de façon
//! atomique (fichier temporaire puis renommage). Ne contient ni URL ni
//! en-tête : la source n'est identifiée que par un hachage.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartState {
    pub version: u32,
    /// BLAKE3 (hex) de l'identifiant de source fourni par l'adaptateur.
    pub source_id: String,
    pub len: u64,
    /// Racine d'archive (hex) lue dans le trailer distant.
    pub root: String,
    /// Validateur fort de la source, s'il existe (jamais un validateur faible).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Nombre d'unités du plan ; la bitmap compte autant de bits.
    pub units: u64,
    /// Bitmap hex, bit `i` = unité `i` vérifiée (poids fort d'abord).
    pub verified: String,
    pub updated_unix: u64,
}

pub fn source_id(label: &str) -> String {
    blake3::hash(label.as_bytes()).to_hex().to_string()
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    bits: Vec<bool>,
}

impl Bitmap {
    pub fn new(n: usize) -> Self {
        Self {
            bits: vec![false; n],
        }
    }

    pub fn get(&self, i: usize) -> bool {
        self.bits[i]
    }

    pub fn set(&mut self, i: usize) {
        self.bits[i] = true;
    }

    pub fn len(&self) -> usize {
        self.bits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    pub fn count(&self) -> usize {
        self.bits.iter().filter(|b| **b).count()
    }

    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(self.bits.len().div_ceil(4));
        for nibble in self.bits.chunks(4) {
            let mut v = 0u8;
            for (k, b) in nibble.iter().enumerate() {
                if *b {
                    v |= 8 >> k;
                }
            }
            out.push(char::from_digit(v as u32, 16).unwrap());
        }
        out
    }

    pub fn from_hex(s: &str, n: usize) -> Option<Self> {
        if s.len() != n.div_ceil(4) {
            return None;
        }
        let mut bits = Vec::with_capacity(n);
        for c in s.chars() {
            let v = c.to_digit(16)? as u8;
            for k in 0..4 {
                if bits.len() < n {
                    bits.push(v & (8 >> k) != 0);
                }
            }
        }
        Some(Self { bits })
    }
}

impl PartState {
    pub fn new(
        source_id: String,
        len: u64,
        root: String,
        etag: Option<String>,
        bitmap: &Bitmap,
    ) -> Self {
        Self {
            version: STATE_VERSION,
            source_id,
            len,
            root,
            etag,
            units: bitmap.len() as u64,
            verified: bitmap.to_hex(),
            updated_unix: now(),
        }
    }

    pub fn bitmap(&self) -> Result<Bitmap> {
        Bitmap::from_hex(&self.verified, self.units as usize)
            .ok_or_else(|| Error::State("bitmap de longueur incohérente".into()))
    }

    pub fn load(path: &Path) -> Result<Option<Self>> {
        match fs::read(path) {
            Ok(bytes) => {
                let st: PartState = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::State(format!("{} : {e}", path.display())))?;
                if st.version != STATE_VERSION {
                    return Err(Error::State(format!(
                        "version {} non prise en charge",
                        st.version
                    )));
                }
                Ok(Some(st))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Écriture atomique : fichier voisin puis renommage par-dessus.
    pub fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("part.tmp");
        let json = serde_json::to_vec_pretty(self).map_err(|e| Error::State(e.to_string()))?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_hex_roundtrip() {
        for n in [0usize, 1, 3, 4, 5, 9, 17] {
            let mut b = Bitmap::new(n);
            for i in (0..n).step_by(3) {
                b.set(i);
            }
            let back = Bitmap::from_hex(&b.to_hex(), n).unwrap();
            assert_eq!(back, b, "n={n}");
        }
        assert!(Bitmap::from_hex("ff", 3).is_none());
    }

    #[test]
    fn state_roundtrip_without_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.vault.part");
        let mut bm = Bitmap::new(10);
        bm.set(2);
        let st = PartState::new(
            source_id("https://x/y?token=abc"),
            123,
            "00".into(),
            Some("\"e1\"".into()),
            &bm,
        );
        st.save(&p).unwrap();
        let raw = fs::read_to_string(&p).unwrap();
        assert!(!raw.contains("token"), "l'URL ne doit pas apparaître");
        let back = PartState::load(&p).unwrap().unwrap();
        assert_eq!(back, st);
        assert_eq!(back.bitmap().unwrap(), bm);
        assert_eq!(
            part_path(Path::new("x/a.vault")),
            PathBuf::from("x/a.vault.part")
        );
    }
}
