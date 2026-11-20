//! Cache de morceaux décompressés et vérifiés, partagé entre lectures.
//!
//! Une lecture partielle (FUSE, WinFsp, `tholos cat`) de 128 KiB chevauche en
//! général deux morceaux de 64 à 256 KiB : sans cache, chaque morceau serait
//! décompressé deux fois lors d'un parcours séquentiel. Le cache est un petit
//! LRU indexé par numéro de morceau ; seuls des morceaux dont le hachage a été
//! vérifié y entrent, un morceau corrompu n'y est jamais stocké.

use std::sync::{Arc, Mutex};

/// Capacité par défaut : 8 morceaux, soit 2 MiB au plus.
pub const DEFAULT_CAPACITY: usize = 8;

#[derive(Debug)]
pub struct ChunkCache {
    capacity: usize,
    /// Du moins récent au plus récent.
    entries: Mutex<Vec<(u32, Arc<[u8]>)>>,
}

impl Default for ChunkCache {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl ChunkCache {
    /// `capacity == 0` désactive le cache (chaque lecture décompresse).
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Mutex::new(Vec::with_capacity(capacity)),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Morceau en cache, promu en tête s'il est présent.
    pub fn get(&self, index: u32) -> Option<Arc<[u8]>> {
        let mut e = self.lock();
        let pos = e.iter().position(|(i, _)| *i == index)?;
        let item = e.remove(pos);
        let data = item.1.clone();
        e.push(item);
        Some(data)
    }

    /// Insère un morceau déjà vérifié ; évince le moins récent si nécessaire.
    pub fn insert(&self, index: u32, data: Arc<[u8]>) {
        if self.capacity == 0 {
            return;
        }
        let mut e = self.lock();
        if let Some(pos) = e.iter().position(|(i, _)| *i == index) {
            e.remove(pos);
        } else if e.len() >= self.capacity {
            e.remove(0);
        }
        e.push((index, data));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<(u32, Arc<[u8]>)>> {
        // Un panic pendant une insertion n'a pas pu laisser le vecteur dans un
        // état incohérent : on récupère le verrou empoisonné.
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arc(b: &[u8]) -> Arc<[u8]> {
        Arc::from(b)
    }

    #[test]
    fn lru_evicts_least_recent() {
        let c = ChunkCache::new(2);
        c.insert(1, arc(b"a"));
        c.insert(2, arc(b"b"));
        assert_eq!(&*c.get(1).unwrap(), b"a");
        c.insert(3, arc(b"c"));
        assert!(c.get(2).is_none(), "2 était le moins récent");
        assert!(c.get(1).is_some() && c.get(3).is_some());
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn zero_capacity_stores_nothing() {
        let c = ChunkCache::new(0);
        c.insert(1, arc(b"a"));
        assert!(c.get(1).is_none() && c.is_empty());
    }
}
