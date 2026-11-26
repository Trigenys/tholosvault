//! Catalogue : les archives `.vault` d'un répertoire racine, ouvertes à la
//! demande et gardées ouvertes tant que le fichier ne change pas (taille et
//! date de modification). Un remplacement atomique (`tholos pack` écrit un
//! temporaire puis renomme, I-007) est donc vu à la requête suivante.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use vault_core::Archive;

use crate::error::ApiError;

/// Extension des archives servies.
pub const EXTENSION: &str = ".vault";

struct Cached {
    len: u64,
    modified: Option<SystemTime>,
    archive: Arc<Archive>,
}

pub struct Catalog {
    root: PathBuf,
    open: Mutex<HashMap<String, Cached>>,
}

impl Catalog {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            open: Mutex::new(HashMap::new()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Noms des archives du répertoire racine (non récursif), triés.
    pub fn names(&self) -> Result<Vec<String>, ApiError> {
        let rd = fs::read_dir(&self.root).map_err(|e| {
            ApiError::Internal(format!(
                "lecture du répertoire {} impossible : {e}",
                self.root.display()
            ))
        })?;
        let mut names: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| valid_name(n))
            .collect();
        names.sort();
        Ok(names)
    }

    /// Archive `name`, ouverte au besoin. Un nom hors du répertoire racine
    /// (séparateur, `..`, fichier caché, autre extension) est refusé.
    pub fn get(&self, name: &str) -> Result<Arc<Archive>, ApiError> {
        if !valid_name(name) {
            return Err(ApiError::BadRequest(format!(
                "nom d'archive invalide : {name:?} (attendu : un nom de fichier *{EXTENSION} \
                 du répertoire servi, sans séparateur)"
            )));
        }
        let path = self.root.join(name);
        let meta = match fs::metadata(&path) {
            Ok(m) if m.is_file() => m,
            _ => return Err(ApiError::NotFound(format!("archive absente : {name}"))),
        };
        let (len, modified) = (meta.len(), meta.modified().ok());
        let mut open = self.open.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = open.get(name) {
            if c.len == len && c.modified == modified {
                return Ok(c.archive.clone());
            }
        }
        let archive = Arc::new(Archive::open(&path).map_err(|e| {
            ApiError::Unprocessable(format!(
                "ouverture de {name} impossible : {e} (diagnostic : `tholos verify {name}`)"
            ))
        })?);
        open.insert(
            name.to_string(),
            Cached {
                len,
                modified,
                archive: archive.clone(),
            },
        );
        Ok(archive)
    }
}

/// Nom de fichier simple se terminant par `.vault`, sans séparateur ni point initial.
pub fn valid_name(name: &str) -> bool {
    name.len() > EXTENSION.len()
        && name.ends_with(EXTENSION)
        && !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
}

#[cfg(test)]
mod tests {
    use super::valid_name;

    #[test]
    fn names_stay_inside_the_root() {
        assert!(valid_name("jeux.vault"));
        assert!(valid_name("v1.2.vault"));
        assert!(!valid_name(".vault"));
        assert!(!valid_name(".cache.vault"));
        assert!(!valid_name("../x.vault"));
        assert!(!valid_name("a/b.vault"));
        assert!(!valid_name("a\\b.vault"));
        assert!(!valid_name("a.tar"));
        assert!(!valid_name("a.vault.tholos-part"));
    }
}
