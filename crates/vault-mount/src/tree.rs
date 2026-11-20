//! Arborescence en mémoire construite depuis l'index de l'archive.
//!
//! Numéros d'inœud stables : `ino = index + 1`, racine `1`. Les répertoires
//! implicites (jamais stockés dans l'archive) sont créés à la volée. Les
//! enfants d'un répertoire sont triés par octets du nom : la recherche est une
//! dichotomie, le parcours est ordonné et déterministe.

use std::collections::BTreeMap;

use vault_core::Archive;

use crate::error::MountError;
use crate::{Ino, NodeKind, ROOT_INO};

pub(crate) struct Node {
    pub parent: Ino,
    pub kind: Kind,
}

pub(crate) enum Kind {
    Dir { children: Vec<(Box<str>, Ino)> },
    File { file: usize },
}

impl Node {
    pub fn node_kind(&self) -> NodeKind {
        match self.kind {
            Kind::Dir { .. } => NodeKind::Dir,
            Kind::File { .. } => NodeKind::File,
        }
    }
}

pub(crate) struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    pub fn build(archive: &Archive) -> Result<Self, MountError> {
        let mut nodes: Vec<Node> = vec![Node {
            parent: ROOT_INO,
            kind: Kind::Dir {
                children: Vec::new(),
            },
        }];
        // Index temporaire : enfants par répertoire, ordonnés par octets du nom.
        let mut maps: Vec<BTreeMap<Box<str>, Ino>> = vec![BTreeMap::new()];

        for (file_index, entry) in archive.files().iter().enumerate() {
            let parts = vault_core::path::components(&entry.path)
                .map_err(|e| MountError::Tree(e.to_string()))?;
            let (leaf, dirs) = parts.split_last().expect("components non vide");
            let mut dir = ROOT_INO;
            for part in dirs {
                let slot = (dir - 1) as usize;
                dir = match maps[slot].get(*part) {
                    Some(&ino) => match nodes[(ino - 1) as usize].kind {
                        Kind::Dir { .. } => ino,
                        Kind::File { .. } => {
                            return Err(MountError::Tree(format!(
                                "{} : un fichier porte déjà le nom d'un répertoire intermédiaire",
                                entry.path
                            )))
                        }
                    },
                    None => {
                        let ino = nodes.len() as Ino + 1;
                        nodes.push(Node {
                            parent: dir,
                            kind: Kind::Dir {
                                children: Vec::new(),
                            },
                        });
                        maps.push(BTreeMap::new());
                        maps[slot].insert((*part).into(), ino);
                        ino
                    }
                };
            }
            let slot = (dir - 1) as usize;
            if maps[slot].contains_key(*leaf) {
                return Err(MountError::Tree(format!(
                    "{} : nom déjà pris dans ce répertoire",
                    entry.path
                )));
            }
            let ino = nodes.len() as Ino + 1;
            nodes.push(Node {
                parent: dir,
                kind: Kind::File { file: file_index },
            });
            maps.push(BTreeMap::new());
            maps[slot].insert((*leaf).into(), ino);
        }

        for (node, map) in nodes.iter_mut().zip(maps) {
            if let Kind::Dir { children } = &mut node.kind {
                *children = map.into_iter().collect();
            }
        }
        Ok(Tree { nodes })
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn node(&self, ino: Ino) -> Option<&Node> {
        usize::try_from(ino)
            .ok()
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| self.nodes.get(i))
    }

    pub fn children(&self, ino: Ino) -> Option<&[(Box<str>, Ino)]> {
        match &self.node(ino)?.kind {
            Kind::Dir { children } => Some(children),
            Kind::File { .. } => None,
        }
    }

    pub fn lookup(&self, parent: Ino, name: &str) -> Option<Ino> {
        let children = self.children(parent)?;
        children
            .binary_search_by(|(n, _)| n.as_bytes().cmp(name.as_bytes()))
            .ok()
            .map(|i| children[i].1)
    }

    /// Nombre de sous-répertoires directs (pour `nlink`).
    pub fn subdirs(&self, ino: Ino) -> u32 {
        self.children(ino)
            .map(|c| {
                c.iter()
                    .filter(|(_, i)| matches!(self.node(*i), Some(n) if matches!(n.kind, Kind::Dir { .. })))
                    .count() as u32
            })
            .unwrap_or(0)
    }
}
