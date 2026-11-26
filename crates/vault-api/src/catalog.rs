//! Catalogue : les archives `.vault` d'un répertoire racine, ouvertes à
//! chaque requête (en-tête, trailer et tables, sans décompression : L-002) et
//! refermées avec la réponse. Pas de cache d'archives ouvertes : une clé
//! taille + date de modification manquerait un remplacement de même taille
//! dans la granularité de l'horloge du système de fichiers, et servirait
//! l'ancienne version. `tholos pack` remplace par renommage (I-007) : la
//! nouvelle version est servie dès la requête suivante.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use vault_core::Archive;

use crate::error::ApiError;

/// Extension des archives servies.
pub const EXTENSION: &str = ".vault";

pub struct Catalog {
    root: PathBuf,
}

impl Catalog {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
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

    /// Archive `name`, ouverte pour la durée d'une requête. Un nom hors du répertoire racine
    /// (séparateur, `..`, fichier caché, autre extension) est refusé.
    pub fn get(&self, name: &str) -> Result<Arc<Archive>, ApiError> {
        if !valid_name(name) {
            return Err(ApiError::BadRequest(format!(
                "nom d'archive invalide : {name:?} (attendu : un nom de fichier *{EXTENSION} \
                 du répertoire servi, sans séparateur)"
            )));
        }
        let path = self.root.join(name);
        if !path.is_file() {
            return Err(ApiError::NotFound(format!("archive absente : {name}")));
        }
        let archive = Archive::open(&path).map_err(|e| {
            ApiError::Unprocessable(format!(
                "ouverture de {name} impossible : {e} (diagnostic : `tholos verify {name}`)"
            ))
        })?;
        Ok(Arc::new(archive))
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
