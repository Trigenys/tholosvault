//! Cache de morceaux décompressés et vérifiés, partagé entre lectures.
//!
//! Une lecture partielle (FUSE, WinFsp, `tholos cat`) de 128 KiB chevauche en
//! général deux morceaux de 64 à 256 KiB : sans cache, chaque morceau serait
//! décompressé deux fois lors d'un parcours séquentiel. Le cache est un petit
//! LRU indexé par numéro de morceau ; seuls des morceaux dont le hachage a été
//! vérifié y entrent, un morceau corrompu n'y est jamais stocké.
//!
//! **Vol unique** : quand plusieurs lecteurs manquent le même morceau en même
//! temps (le noyau découpe une lecture anticipée en requêtes servies par
//! plusieurs fils), un seul le décode ; les autres attendent ce décodage au
//! lieu de le refaire chacun (ADR-0004). Si le décodeur échoue ou panique,
//! les lecteurs en attente reprennent la main et décodent eux-mêmes : une
//! erreur n'est jamais mise en cache, et personne ne reste bloqué.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// Capacité par défaut : 8 morceaux, soit 2 MiB au plus.
pub const DEFAULT_CAPACITY: usize = 8;

/// Compteurs du cache, cumulés depuis sa création (diagnostic, bancs).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Morceaux servis depuis le cache.
    pub hits: u64,
    /// Morceaux décodés par l'appelant (absent du cache et aucun décodage en cours).
    pub decodes: u64,
    /// Morceaux obtenus en attendant un décodage en cours chez un autre lecteur.
    pub joined: u64,
}

/// Décodage en cours d'un morceau, partagé entre le décodeur et ceux qui attendent.
#[derive(Debug, Default)]
struct Inflight {
    /// `None` : en cours ; `Some(Some(d))` : décodé ; `Some(None)` : échoué ou abandonné.
    done: Mutex<Option<Option<Arc<[u8]>>>>,
    cv: Condvar,
}

impl Inflight {
    fn finish(&self, data: Option<Arc<[u8]>>) {
        *self.done.lock().unwrap_or_else(|p| p.into_inner()) = Some(data);
        self.cv.notify_all();
    }

    fn wait(&self) -> Option<Arc<[u8]>> {
        let mut d = self.done.lock().unwrap_or_else(|p| p.into_inner());
        while d.is_none() {
            d = self.cv.wait(d).unwrap_or_else(|p| p.into_inner());
        }
        d.as_ref().and_then(Clone::clone)
    }
}

#[derive(Debug, Default)]
struct State {
    /// Du moins récent au plus récent.
    entries: Vec<(u32, Arc<[u8]>)>,
    inflight: HashMap<u32, Arc<Inflight>>,
}

#[derive(Debug)]
pub struct ChunkCache {
    capacity: usize,
    state: Mutex<State>,
    hits: AtomicU64,
    decodes: AtomicU64,
    joined: AtomicU64,
}

impl Default for ChunkCache {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl ChunkCache {
    /// `capacity == 0` désactive le cache (chaque lecture décompresse, sans vol unique).
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            state: Mutex::new(State {
                entries: Vec::with_capacity(capacity),
                inflight: HashMap::new(),
            }),
            hits: AtomicU64::new(0),
            decodes: AtomicU64::new(0),
            joined: AtomicU64::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.lock().entries.clear();
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            decodes: self.decodes.load(Ordering::Relaxed),
            joined: self.joined.load(Ordering::Relaxed),
        }
    }

    /// Morceau en cache, promu en tête s'il est présent. Ne compte pas dans
    /// [`CacheStats`] : les compteurs suivent [`ChunkCache::get_or_decode`].
    pub fn get(&self, index: u32) -> Option<Arc<[u8]>> {
        Self::get_locked(&mut self.lock(), index)
    }

    /// Insère un morceau déjà vérifié ; évince le moins récent si nécessaire.
    pub fn insert(&self, index: u32, data: Arc<[u8]>) {
        if self.capacity == 0 {
            return;
        }
        Self::insert_locked(&mut self.lock(), self.capacity, index, data);
    }

    /// Renvoie le morceau `index`, depuis le cache ou via `decode`, avec vol
    /// unique : si un autre lecteur le décode déjà, attend son résultat. Une
    /// erreur de `decode` est renvoyée à l'appelant et n'entre pas dans le
    /// cache ; les lecteurs en attente décodent alors eux-mêmes.
    pub fn get_or_decode<E>(
        &self,
        index: u32,
        decode: impl FnOnce() -> Result<Arc<[u8]>, E>,
    ) -> Result<Arc<[u8]>, E> {
        if self.capacity == 0 {
            self.decodes.fetch_add(1, Ordering::Relaxed);
            return decode();
        }
        loop {
            let mut st = self.lock();
            if let Some(d) = Self::get_locked(&mut st, index) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Ok(d);
            }
            if let Some(fl) = st.inflight.get(&index).cloned() {
                drop(st);
                if let Some(d) = fl.wait() {
                    self.joined.fetch_add(1, Ordering::Relaxed);
                    return Ok(d);
                }
                // Le décodeur a échoué ou abandonné : on reprend la main.
                continue;
            }
            let fl = Arc::new(Inflight::default());
            st.inflight.insert(index, fl.clone());
            drop(st);
            self.decodes.fetch_add(1, Ordering::Relaxed);

            // Même en cas de panic dans `decode`, le garde retire le vol et
            // réveille les lecteurs en attente.
            let mut guard = InflightGuard {
                cache: self,
                index,
                fl: &fl,
                data: None,
            };
            let r = decode();
            if let Ok(d) = &r {
                guard.data = Some(d.clone());
            }
            drop(guard);
            return r;
        }
    }

    fn get_locked(st: &mut State, index: u32) -> Option<Arc<[u8]>> {
        let pos = st.entries.iter().position(|(i, _)| *i == index)?;
        let item = st.entries.remove(pos);
        let data = item.1.clone();
        st.entries.push(item);
        Some(data)
    }

    fn insert_locked(st: &mut State, capacity: usize, index: u32, data: Arc<[u8]>) {
        if let Some(pos) = st.entries.iter().position(|(i, _)| *i == index) {
            st.entries.remove(pos);
        } else if st.entries.len() >= capacity {
            st.entries.remove(0);
        }
        st.entries.push((index, data));
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Un panic pendant une insertion n'a pas pu laisser le vecteur dans un
        // état incohérent : on récupère le verrou empoisonné.
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Clôture d'un décodage en vol : publie le résultat (ou l'abandon) et retire
/// l'entrée en cours, même si le décodeur a paniqué.
struct InflightGuard<'a> {
    cache: &'a ChunkCache,
    index: u32,
    fl: &'a Arc<Inflight>,
    data: Option<Arc<[u8]>>,
}

impl Drop for InflightGuard<'_> {
    fn drop(&mut self) {
        let mut st = self.cache.lock();
        st.inflight.remove(&self.index);
        if let Some(d) = &self.data {
            ChunkCache::insert_locked(&mut st, self.cache.capacity, self.index, d.clone());
        }
        drop(st);
        self.fl.finish(self.data.take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Barrier;
    use std::time::Duration;

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
        let d: Result<_, ()> = c.get_or_decode(1, || Ok(arc(b"x")));
        assert_eq!(&*d.unwrap(), b"x");
        assert!(c.is_empty());
    }

    #[test]
    fn get_or_decode_caches_and_counts() {
        let c = ChunkCache::new(4);
        let d: Result<_, ()> = c.get_or_decode(7, || Ok(arc(b"sept")));
        assert_eq!(&*d.unwrap(), b"sept");
        let d: Result<_, ()> = c.get_or_decode(7, || panic!("déjà en cache"));
        assert_eq!(&*d.unwrap(), b"sept");
        assert_eq!(
            c.stats(),
            CacheStats {
                hits: 1,
                decodes: 1,
                joined: 0
            }
        );
    }

    #[test]
    fn error_is_not_cached() {
        let c = ChunkCache::new(4);
        let r: Result<Arc<[u8]>, &str> = c.get_or_decode(3, || Err("corrompu"));
        assert_eq!(r.unwrap_err(), "corrompu");
        assert!(c.is_empty());
        assert!(c.lock().inflight.is_empty(), "aucun vol résiduel");
    }

    /// N lecteurs concurrents d'un même morceau manquant → un seul décodage.
    #[test]
    fn concurrent_miss_decodes_once() {
        const N: usize = 8;
        let c = Arc::new(ChunkCache::new(4));
        let decodes = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(N));
        let handles: Vec<_> = (0..N)
            .map(|_| {
                let (c, decodes, barrier) = (c.clone(), decodes.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    let d: Result<_, ()> = c.get_or_decode(42, || {
                        decodes.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(50));
                        Ok(arc(b"quarante-deux"))
                    });
                    d.unwrap()
                })
            })
            .collect();
        for h in handles {
            assert_eq!(&*h.join().unwrap(), b"quarante-deux");
        }
        assert_eq!(
            decodes.load(Ordering::SeqCst),
            1,
            "un seul décodage pour {N} lecteurs"
        );
        let s = c.stats();
        assert_eq!(s.decodes, 1);
        assert_eq!(s.joined + s.hits, (N - 1) as u64);
        assert!(
            s.joined >= 1,
            "au moins un lecteur a attendu le vol en cours"
        );
        assert_eq!(c.len(), 1);
    }

    /// Un décodeur qui échoue libère les lecteurs en attente, qui décodent eux-mêmes.
    #[test]
    fn failed_decode_releases_waiters() {
        let c = Arc::new(ChunkCache::new(4));
        let barrier = Arc::new(Barrier::new(2));
        let first = {
            let (c, barrier) = (c.clone(), barrier.clone());
            std::thread::spawn(move || {
                let r: Result<Arc<[u8]>, &str> = c.get_or_decode(1, || {
                    barrier.wait();
                    std::thread::sleep(Duration::from_millis(50));
                    Err("corrompu")
                });
                r.is_err()
            })
        };
        barrier.wait();
        let d: Result<_, &str> = c.get_or_decode(1, || Ok(arc(b"ok")));
        assert_eq!(&*d.unwrap(), b"ok");
        assert!(first.join().unwrap());
        assert_eq!(c.stats().decodes, 2);
    }

    /// Un décodeur qui panique ne laisse ni vol résiduel ni lecteur bloqué.
    #[test]
    fn panicking_decode_releases_waiters() {
        let c = Arc::new(ChunkCache::new(4));
        let barrier = Arc::new(Barrier::new(2));
        let first = {
            let (c, barrier) = (c.clone(), barrier.clone());
            std::thread::spawn(move || {
                let _: Result<Arc<[u8]>, ()> = c.get_or_decode(1, || {
                    barrier.wait();
                    std::thread::sleep(Duration::from_millis(50));
                    panic!("décodeur en panique")
                });
            })
        };
        barrier.wait();
        let d: Result<_, ()> = c.get_or_decode(1, || Ok(arc(b"ok")));
        assert_eq!(&*d.unwrap(), b"ok");
        assert!(first.join().is_err());
        assert!(c.lock().inflight.is_empty());
    }
}
