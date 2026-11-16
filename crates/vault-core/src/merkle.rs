//! Identifiants de contenu et arbres de Merkle BLAKE3.
//!
//! Séparation de domaine par premier octet : 0x00 feuille vide, 0x01 nœud interne,
//! 0x02 feuille de fichier (dans la racine d'archive). Un nœud impair est promu
//! tel quel au niveau supérieur. Une feuille seule est sa propre racine.

/// Hachage BLAKE3 de 32 octets.
pub type Hash = [u8; 32];

const DOMAIN_EMPTY: u8 = 0x00;
const DOMAIN_NODE: u8 = 0x01;
const DOMAIN_FILE: u8 = 0x02;

/// Identifiant d'un morceau : BLAKE3 de son contenu clair.
pub fn chunk_id(data: &[u8]) -> Hash {
    *blake3::hash(data).as_bytes()
}

/// Racine de Merkle d'une liste ordonnée de feuilles.
pub fn merkle_root(leaves: &[Hash]) -> Hash {
    if leaves.is_empty() {
        return *blake3::hash(&[DOMAIN_EMPTY]).as_bytes();
    }
    let mut level: Vec<Hash> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            if pair.len() == 2 {
                let mut h = blake3::Hasher::new();
                h.update(&[DOMAIN_NODE]);
                h.update(&pair[0]);
                h.update(&pair[1]);
                next.push(*h.finalize().as_bytes());
            } else {
                next.push(pair[0]);
            }
        }
        level = next;
    }
    level[0]
}

/// Feuille d'un fichier dans l'arbre d'archive : lie chemin, taille et racine.
pub fn file_leaf(path: &str, size: u64, file_root: &Hash) -> Hash {
    let mut h = blake3::Hasher::new();
    h.update(&[DOMAIN_FILE]);
    h.update(&(path.len() as u16).to_le_bytes());
    h.update(path.as_bytes());
    h.update(&size.to_le_bytes());
    h.update(file_root);
    *h.finalize().as_bytes()
}

pub fn to_hex(h: &Hash) -> String {
    let mut s = String::with_capacity(64);
    for b in h {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_leaf_is_root() {
        let leaf = chunk_id(b"a");
        assert_eq!(merkle_root(&[leaf]), leaf);
    }

    #[test]
    fn empty_root_is_stable_and_distinct() {
        let e = merkle_root(&[]);
        assert_eq!(e, merkle_root(&[]));
        assert_ne!(e, chunk_id(b""));
    }

    #[test]
    fn order_matters() {
        let a = chunk_id(b"a");
        let b = chunk_id(b"b");
        assert_ne!(merkle_root(&[a, b]), merkle_root(&[b, a]));
    }

    #[test]
    fn odd_promotion() {
        let l: Vec<Hash> = [b"a", b"b", b"c"].iter().map(|s| chunk_id(*s)).collect();
        let ab = merkle_root(&l[..2]);
        let mut h = blake3::Hasher::new();
        h.update(&[DOMAIN_NODE]);
        h.update(&ab);
        h.update(&l[2]);
        assert_eq!(merkle_root(&l), *h.finalize().as_bytes());
    }
}
