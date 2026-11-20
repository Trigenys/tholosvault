//! Chemins d'archive : règle unique de validation des composants, partagée
//! par l'extraction (`tholos extract`) et le montage (`vault-mount`).
//!
//! Un chemin d'archive est relatif, séparé par `/`, sans composant vide, `.`
//! ni `..`, et sans `\`, `:` ou octet nul (portabilité Windows, pas de sortie
//! du répertoire cible).

use crate::error::{Error, Result};

/// Découpe `rel` en composants validés ; refuse tout chemin qui sortirait d'un
/// répertoire cible ou ne serait pas portable.
pub fn components(rel: &str) -> Result<Vec<&str>> {
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.iter().any(|p| !is_valid_component(p)) {
        return Err(Error::InvalidPath(rel.to_string()));
    }
    Ok(parts)
}

pub fn is_valid_component(part: &str) -> bool {
    !part.is_empty() && part != "." && part != ".." && !part.contains(['\\', ':', '\0'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_relative_paths() {
        assert_eq!(components("a").unwrap(), ["a"]);
        assert_eq!(components("a/b.txt").unwrap(), ["a", "b.txt"]);
        assert_eq!(components("é/ü").unwrap(), ["é", "ü"]);
    }

    #[test]
    fn rejects_escapes_and_unportable_names() {
        for bad in [
            "", "/a", "a/", "a//b", ".", "..", "a/../b", "a\\b", "c:x", "a\0",
        ] {
            assert!(
                matches!(components(bad), Err(Error::InvalidPath(_))),
                "{bad:?}"
            );
        }
    }
}
