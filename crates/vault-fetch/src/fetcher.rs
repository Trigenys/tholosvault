//! Orchestration : préparation (sonde, trailer, tables, plan, reprise),
//! file de segments partagée entre N tâches, vérification par unité,
//! persistance périodique de l'état.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures_util::StreamExt;
use tokio::sync::Notify;
use vault_core::format::Trailer;
use vault_core::merkle::{to_hex, Hash};
use vault_core::{
    parse_archive_tables, read_trailer_tail, verify_chunk_frame, Archive, ArchiveTables,
};

use crate::dest::Dest;
use crate::error::{Error, Result};
use crate::limiter::RateLimiter;
use crate::plan::{tables_range, trailer_range, Plan, Segment, UnitKind, TRAILER_TAIL_LEN};
use crate::state::{part_path, source_id, Bitmap, PartState};
use crate::transport::{RangeResponse, RemoteInfo, RemoteVersion, Transport};
use crate::{Event, FetchOptions, Outcome, PlanReport, Summary};

const STATE_FLUSH_INTERVAL: Duration = Duration::from_secs(1);

pub struct Fetcher<T: Transport + 'static> {
    transport: Arc<T>,
    opts: FetchOptions,
}

/// Compteurs partagés par toutes les étapes.
#[derive(Default)]
struct Counters {
    requests: AtomicU64,
    bytes_received: AtomicU64,
    chunks_rejected: AtomicU64,
    bytes_rejected: AtomicU64,
    cuts: AtomicU64,
    verified_bytes: AtomicU64,
}

enum Prepared {
    /// Destination complète et identique : aucun contenu à demander.
    AlreadyComplete {
        root: Hash,
        len: u64,
    },
    /// Source sans plages : un seul flux puis vérification complète.
    Stream {
        len: u64,
    },
    Ranged(Box<Ranged>),
}

struct Ranged {
    len: u64,
    version: Option<RemoteVersion>,
    trailer: Trailer,
    tables: ArchiveTables,
    plan: Plan,
    bitmap: Bitmap,
    resumed: bool,
    /// Octets reçus à écrire dans une destination neuve (tables, trailer).
    pending_writes: Vec<(u64, Vec<u8>)>,
}

impl<T: Transport + 'static> Fetcher<T> {
    pub fn new(transport: T, opts: FetchOptions) -> Self {
        Self {
            transport: Arc::new(transport),
            opts,
        }
    }

    fn emit(&self, ev: Event) {
        if let Some(tx) = &self.opts.events {
            let _ = tx.send(ev);
        }
    }

    /// Plan en lecture seule : aucune écriture sur disque.
    pub async fn plan(&self) -> Result<PlanReport> {
        let counters = Counters::default();
        match self.prepare(&counters, true).await? {
            Prepared::AlreadyComplete { root, len } => Ok(PlanReport {
                len,
                archive_root: root,
                accepts_ranges: true,
                units: 0,
                verified_units: 0,
                missing_bytes: 0,
                content_bytes: 0,
                segments: 0,
                resumed: false,
                skip_parity: self.opts.skip_parity,
                already_complete: true,
            }),
            Prepared::Stream { len } => Ok(PlanReport {
                len,
                archive_root: self.opts.expected_root.unwrap_or_default(),
                accepts_ranges: false,
                units: 1,
                verified_units: 0,
                missing_bytes: len,
                content_bytes: len,
                segments: 1,
                resumed: false,
                skip_parity: false,
                already_complete: false,
            }),
            Prepared::Ranged(r) => {
                let segs = self.segments(&r);
                let missing: u64 = segs.iter().map(Segment::len).sum();
                Ok(PlanReport {
                    len: r.len,
                    archive_root: r.trailer.archive_root,
                    accepts_ranges: true,
                    units: r.plan.units.len(),
                    verified_units: r.bitmap.count(),
                    missing_bytes: missing,
                    content_bytes: r.plan.content_bytes(),
                    segments: segs.len(),
                    resumed: r.resumed,
                    skip_parity: self.opts.skip_parity,
                    already_complete: false,
                })
            }
        }
    }

    pub async fn run(self) -> Result<Summary> {
        let t0 = Instant::now();
        let counters = Arc::new(Counters::default());
        let prepared = self.prepare(&counters, false).await?;
        let (outcome, root, len) = match prepared {
            Prepared::AlreadyComplete { root, len } => (Outcome::AlreadyComplete, root, len),
            Prepared::Stream { len } => {
                let root = self.run_stream(&counters, len).await?;
                (Outcome::Complete, root, len)
            }
            Prepared::Ranged(r) => {
                let root = r.trailer.archive_root;
                let len = r.len;
                let outcome = self.run_ranged(counters.clone(), *r).await?;
                (outcome, root, len)
            }
        };
        Ok(Summary {
            outcome,
            archive_root: root,
            len,
            requests: counters.requests.load(Ordering::Relaxed),
            bytes_received: counters.bytes_received.load(Ordering::Relaxed),
            chunks_rejected: counters.chunks_rejected.load(Ordering::Relaxed),
            bytes_rejected: counters.bytes_rejected.load(Ordering::Relaxed),
            cuts: counters.cuts.load(Ordering::Relaxed),
            elapsed: t0.elapsed(),
        })
    }

    fn segments(&self, r: &Ranged) -> Vec<Segment> {
        let skip_parity = self.opts.skip_parity;
        r.plan.segments(self.opts.segment_bytes, |i| {
            !r.bitmap.get(i) && !(skip_parity && r.plan.units[i].kind == UnitKind::Parity)
        })
    }

    // ----- préparation ------------------------------------------------------

    async fn fetch_exact(&self, counters: &Counters, range: Range<u64>) -> Result<Vec<u8>> {
        counters.requests.fetch_add(1, Ordering::Relaxed);
        let resp = self.transport.get_range(range.clone()).await?;
        if !resp.partial || resp.start != range.start {
            return Err(Error::RangeIgnored);
        }
        let want = (range.end - range.start) as usize;
        let mut buf = Vec::with_capacity(want);
        let mut body = resp.body;
        while buf.len() < want {
            match body.next().await {
                Some(Ok(b)) => {
                    counters
                        .bytes_received
                        .fetch_add(b.len() as u64, Ordering::Relaxed);
                    buf.extend_from_slice(&b);
                }
                Some(Err(e)) => return Err(e.into()),
                None => {
                    return Err(crate::TransportError::Protocol(format!(
                        "flux interrompu après {} octets sur {want}",
                        buf.len()
                    ))
                    .into())
                }
            }
        }
        if buf.len() != want {
            return Err(
                crate::TransportError::Protocol("octets au-delà de la plage".into()).into(),
            );
        }
        Ok(buf)
    }

    fn check_expected_root(&self, trailer: &Trailer) -> Result<()> {
        if let Some(exp) = self.opts.expected_root {
            if exp != trailer.archive_root {
                return Err(Error::RootMismatch {
                    expected: to_hex(&exp),
                    actual: to_hex(&trailer.archive_root),
                });
            }
        }
        Ok(())
    }

    async fn prepare(&self, counters: &Counters, dry: bool) -> Result<Prepared> {
        counters.requests.fetch_add(1, Ordering::Relaxed);
        let info: RemoteInfo = self.transport.probe().await?;
        let len = info.len.ok_or(Error::UnknownLength)?;
        self.emit(Event::Probed {
            len,
            accepts_ranges: info.accepts_ranges,
        });
        let dest = self.opts.dest.clone();
        let part = part_path(&dest);

        if self.opts.restart && !dry {
            remove_if_exists(&dest)?;
            remove_if_exists(&part)?;
        }
        let dest_exists = dest.exists() && !(self.opts.restart && dry);

        if !info.accepts_ranges {
            if dest_exists && !part.exists() {
                if let (Some(exp), Ok(ar)) = (self.opts.expected_root, Archive::open(&dest)) {
                    if *ar.archive_root() == exp && ar.trailer().data_len > 0 {
                        return Ok(Prepared::AlreadyComplete { root: exp, len });
                    }
                }
            }
            return Ok(Prepared::Stream { len });
        }

        let part_state = if dest_exists {
            PartState::load(&part)?
        } else {
            if !dry {
                remove_if_exists(&part)?;
            }
            None
        };

        // Destination complète (pas d'état) : sonde + trailer, zéro contenu.
        if dest_exists && part_state.is_none() {
            let tail = self.fetch_exact(counters, trailer_range(len)).await?;
            let (trailer, _) = read_trailer_tail(&tail)?;
            self.check_expected_root(&trailer)?;
            let local = Archive::open(&dest).map_err(|e| Error::DestinationDiffers {
                detail: format!("archive locale illisible : {e}"),
            })?;
            let local_len = fs::metadata(&dest)?.len();
            if *local.archive_root() != trailer.archive_root || local_len != len {
                return Err(Error::DestinationDiffers {
                    detail: format!(
                        "racine locale {}, racine distante {}",
                        to_hex(local.archive_root()),
                        to_hex(&trailer.archive_root)
                    ),
                });
            }
            return Ok(Prepared::AlreadyComplete {
                root: trailer.archive_root,
                len,
            });
        }

        let sid = source_id(&self.opts.source_label);
        if let Some(st) = part_state {
            match self.resume(counters, &dest, len, &info, &sid, st).await? {
                Some(r) => return Ok(Prepared::Ranged(Box::new(r))),
                None => {
                    // État inutilisable : repartir de zéro.
                    if !dry {
                        remove_if_exists(&part)?;
                    }
                }
            }
        }

        // Départ à neuf : trailer → racine attendue → tables → plan.
        let tail = self.fetch_exact(counters, trailer_range(len)).await?;
        let (trailer, _) = read_trailer_tail(&tail)?;
        self.check_expected_root(&trailer)?;
        let trange = tables_range(&trailer);
        if trange.end + trailer.parity_len + TRAILER_TAIL_LEN != len || trange.start > trange.end {
            return Err(Error::Layout(format!(
                "trailer : tables {}..{} + parité {} octets ≠ {len}",
                trange.start, trange.end, trailer.parity_len
            )));
        }
        let tbytes = self.fetch_exact(counters, trange.clone()).await?;
        let tables = parse_tables_checked(&trailer, &tbytes)?;
        let plan = Plan::build(&trailer, &tables, len, self.opts.segment_bytes)?;
        let mut bitmap = Bitmap::new(plan.units.len());
        for (i, u) in plan.units.iter().enumerate() {
            if matches!(u.kind, UnitKind::Tables | UnitKind::Trailer) {
                bitmap.set(i);
            }
        }
        counters.verified_bytes.store(
            (trange.end - trange.start) + TRAILER_TAIL_LEN,
            Ordering::Relaxed,
        );
        Ok(Prepared::Ranged(Box::new(Ranged {
            len,
            version: info.version,
            trailer,
            tables,
            plan,
            bitmap,
            resumed: false,
            pending_writes: vec![(trange.start, tbytes), (len - TRAILER_TAIL_LEN, tail)],
        })))
    }

    /// Reprise sur `.part` : `None` si l'état ne décrit pas cette source.
    async fn resume(
        &self,
        counters: &Counters,
        dest: &Path,
        len: u64,
        info: &RemoteInfo,
        sid: &str,
        st: PartState,
    ) -> Result<Option<Ranged>> {
        if st.source_id != sid || st.len != len {
            return Ok(None);
        }
        let remote_strong = info.version.as_ref().and_then(RemoteVersion::strong);
        let same_version = match (&st.etag, remote_strong) {
            (Some(a), Some(b)) => {
                if a != b {
                    return Err(Error::RemoteChanged {
                        detail: "validateur fort différent".into(),
                    });
                }
                true
            }
            _ => false,
        };
        // Tables et trailer sont toujours écrits avant le premier octet de
        // contenu : on les relit localement plutôt que de les re-télécharger.
        let local = match Dest::open_existing(dest, len) {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };
        let mut tail = vec![0u8; TRAILER_TAIL_LEN as usize];
        if local.read_at(len - TRAILER_TAIL_LEN, &mut tail).is_err() {
            return Ok(None);
        }
        let Ok((trailer, _)) = read_trailer_tail(&tail) else {
            return Ok(None);
        };
        if to_hex(&trailer.archive_root) != st.root {
            return Ok(None);
        }
        if !same_version {
            // Validateur faible ou absent : on redemande le trailer distant
            // (256 octets, hors contenu) et on compare les racines.
            let remote_tail = self.fetch_exact(counters, trailer_range(len)).await?;
            let (remote, _) = read_trailer_tail(&remote_tail)?;
            if remote.archive_root != trailer.archive_root {
                return Err(Error::RemoteChanged {
                    detail: format!(
                        "racine {} devenue {}",
                        to_hex(&trailer.archive_root),
                        to_hex(&remote.archive_root)
                    ),
                });
            }
        }
        self.check_expected_root(&trailer)?;
        let trange = tables_range(&trailer);
        let mut tbytes = vec![0u8; (trange.end - trange.start) as usize];
        if local.read_at(trange.start, &mut tbytes).is_err() {
            return Ok(None);
        }
        let Ok(tables) = parse_tables_checked(&trailer, &tbytes) else {
            return Ok(None);
        };
        let plan = Plan::build(&trailer, &tables, len, self.opts.segment_bytes)?;
        let Ok(bitmap) = st.bitmap() else {
            return Ok(None);
        };
        if bitmap.len() != plan.units.len() {
            return Ok(None);
        }
        let verified: u64 = plan
            .units
            .iter()
            .enumerate()
            .filter(|(i, _)| bitmap.get(*i))
            .map(|(_, u)| u.len())
            .sum();
        counters.verified_bytes.store(verified, Ordering::Relaxed);
        Ok(Some(Ranged {
            len,
            version: info.version.clone(),
            trailer,
            tables,
            plan,
            bitmap,
            resumed: true,
            pending_writes: Vec::new(),
        }))
    }

    // ----- flux unique (source sans plages) --------------------------------

    async fn run_stream(&self, counters: &Counters, len: u64) -> Result<Hash> {
        let dest_path = self.opts.dest.clone();
        remove_if_exists(&part_path(&dest_path))?;
        let dest = Dest::create(&dest_path, len)?;
        let limiter = self.opts.limit_bytes_per_sec.map(RateLimiter::new);
        counters.requests.fetch_add(1, Ordering::Relaxed);
        let resp = self.transport.get_range(0..len).await?;
        if resp.start != 0 {
            return Err(crate::TransportError::Protocol("flux ne débutant pas à 0".into()).into());
        }
        let mut body = resp.body;
        let mut pos = 0u64;
        while let Some(item) = body.next().await {
            let b = item?;
            if let Some(l) = &limiter {
                l.acquire(b.len()).await;
            }
            counters
                .bytes_received
                .fetch_add(b.len() as u64, Ordering::Relaxed);
            if pos + b.len() as u64 > len {
                return Err(crate::TransportError::Protocol(
                    "octets au-delà de la longueur".into(),
                )
                .into());
            }
            let d = dest.clone();
            let off = pos;
            let n = b.len() as u64;
            tokio::task::spawn_blocking(move || d.write_at(off, &b))
                .await
                .expect("tâche d'écriture")?;
            pos += n;
            counters.verified_bytes.store(pos, Ordering::Relaxed);
        }
        if pos != len {
            return Err(crate::TransportError::Protocol(format!(
                "flux interrompu après {pos} octets sur {len}"
            ))
            .into());
        }
        dest.sync()?;
        let expected = self.opts.expected_root;
        let path = dest_path.clone();
        tokio::task::spawn_blocking(move || -> Result<Hash> {
            let ar = Archive::open(&path)?;
            let root = *ar.archive_root();
            if let Some(exp) = expected {
                if exp != root {
                    return Err(Error::RootMismatch {
                        expected: to_hex(&exp),
                        actual: to_hex(&root),
                    });
                }
            }
            let rep = ar.verify();
            if !rep.is_ok() {
                return Err(Error::FinalCheck(format!(
                    "{} morceau(x) et {} fichier(s) corrompus",
                    rep.bad_chunks.len(),
                    rep.bad_files.len()
                )));
            }
            Ok(root)
        })
        .await
        .expect("tâche de vérification")
    }

    // ----- plages : N connexions ------------------------------------------

    async fn run_ranged(&self, counters: Arc<Counters>, r: Ranged) -> Result<Outcome> {
        let dest_path = self.opts.dest.clone();
        let part = part_path(&dest_path);
        let dest = if r.resumed {
            Dest::open_existing(&dest_path, r.len)?
        } else {
            Dest::create(&dest_path, r.len)?
        };
        for (off, bytes) in &r.pending_writes {
            dest.write_at(*off, bytes)?;
        }
        let segments = self.segments(&r);
        let missing: u64 = segments.iter().map(Segment::len).sum();
        self.emit(Event::Planned {
            units: r.plan.units.len(),
            verified_units: r.bitmap.count(),
            missing_bytes: missing,
            total_bytes: r.len,
        });
        let etag = r
            .version
            .as_ref()
            .and_then(RemoteVersion::strong)
            .map(str::to_string);
        let root_hex = to_hex(&r.trailer.archive_root);
        let sid = source_id(&self.opts.source_label);

        let shared = Arc::new(Shared {
            transport: self.transport.clone(),
            dest,
            plan: r.plan,
            chunks: r.tables.chunks,
            version: r.version,
            bitmap: Mutex::new(r.bitmap),
            dirty: AtomicBool::new(true),
            queue: Mutex::new(segments.into_iter().collect()),
            in_flight: AtomicUsize::new(0),
            notify: Notify::new(),
            limiter: self.opts.limit_bytes_per_sec.map(RateLimiter::new),
            counters,
            rejects: Mutex::new(HashMap::new()),
            attempts: Mutex::new(HashMap::new()),
            failure: Mutex::new(None),
            cancelled: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            events: self.opts.events.clone(),
            max_chunk_rejects: self.opts.max_chunk_rejects,
            max_segment_attempts: self.opts.max_segment_attempts,
        });
        let state_of = {
            let shared = shared.clone();
            move || {
                let bm = shared.bitmap.lock().unwrap();
                PartState::new(
                    sid.clone(),
                    shared.plan.len,
                    root_hex.clone(),
                    etag.clone(),
                    &bm,
                )
            }
        };
        state_of().save(&part)?;

        let flusher = {
            let shared = shared.clone();
            let part = part.clone();
            let state_of = state_of.clone();
            tokio::spawn(async move {
                while !shared.finished.load(Ordering::Acquire) {
                    tokio::time::sleep(STATE_FLUSH_INTERVAL).await;
                    if shared.dirty.swap(false, Ordering::AcqRel) {
                        let _ = state_of().save(&part);
                    }
                }
            })
        };

        let n = self.opts.connections.max(1);
        let mut workers = Vec::with_capacity(n);
        for _ in 0..n {
            let s = shared.clone();
            workers.push(tokio::spawn(async move { worker(s).await }));
        }
        for w in workers {
            w.await.expect("tâche de téléchargement");
        }
        shared.finished.store(true, Ordering::Release);
        let _ = flusher.await;
        shared.dest.sync()?;
        state_of().save(&part)?;

        if let Some(e) = shared.failure.lock().unwrap().take() {
            return Err(e);
        }

        let (all, content_done) = {
            let bm = shared.bitmap.lock().unwrap();
            let all = (0..bm.len()).all(|i| bm.get(i));
            let content_done = shared
                .plan
                .units
                .iter()
                .enumerate()
                .all(|(i, u)| bm.get(i) || u.kind == UnitKind::Parity);
            (all, content_done)
        };
        if !content_done {
            return Err(Error::FinalCheck(
                "unités manquantes en fin de passe".into(),
            ));
        }
        // Dernier contrôle : l'archive s'ouvre et annonce la racine attendue.
        let path = dest_path.clone();
        let expected = shared.plan_root();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let ar = Archive::open(&path)?;
            if *ar.archive_root() != expected {
                return Err(Error::FinalCheck(
                    "racine finale différente du trailer reçu".into(),
                ));
            }
            Ok(())
        })
        .await
        .expect("tâche de contrôle")?;
        if all {
            remove_if_exists(&part)?;
            Ok(Outcome::Complete)
        } else {
            Ok(Outcome::CompleteWithoutParity)
        }
    }
}

struct Shared<T: Transport + 'static> {
    transport: Arc<T>,
    dest: Dest,
    plan: Plan,
    chunks: Vec<vault_core::ChunkEntry>,
    version: Option<RemoteVersion>,
    bitmap: Mutex<Bitmap>,
    dirty: AtomicBool,
    queue: Mutex<VecDeque<Segment>>,
    in_flight: AtomicUsize,
    notify: Notify,
    limiter: Option<RateLimiter>,
    counters: Arc<Counters>,
    rejects: Mutex<HashMap<usize, u32>>,
    attempts: Mutex<HashMap<u64, u32>>,
    failure: Mutex<Option<Error>>,
    cancelled: AtomicBool,
    finished: AtomicBool,
    events: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
    max_chunk_rejects: u32,
    max_segment_attempts: u32,
}

impl<T: Transport + 'static> Shared<T> {
    fn emit(&self, ev: Event) {
        if let Some(tx) = &self.events {
            let _ = tx.send(ev);
        }
    }

    fn plan_root(&self) -> Hash {
        // La racine est celle du trailer déjà écrit ; on la relit via les
        // tables (identique par construction).
        let mut tail = vec![0u8; TRAILER_TAIL_LEN as usize];
        let _ = self
            .dest
            .read_at(self.plan.len - TRAILER_TAIL_LEN, &mut tail);
        read_trailer_tail(&tail)
            .map(|(t, _)| t.archive_root)
            .unwrap_or_default()
    }

    fn fail(&self, e: Error) {
        let mut f = self.failure.lock().unwrap();
        if f.is_none() {
            *f = Some(e);
        }
        self.cancelled.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    fn push_front(&self, seg: Segment) {
        self.queue.lock().unwrap().push_front(seg);
        self.notify.notify_waiters();
    }

    fn mark_verified(&self, idx: usize) {
        let u = &self.plan.units[idx];
        {
            let mut bm = self.bitmap.lock().unwrap();
            bm.set(idx);
        }
        self.dirty.store(true, Ordering::Release);
        let v = self
            .counters
            .verified_bytes
            .fetch_add(u.len(), Ordering::Relaxed)
            + u.len();
        self.emit(Event::UnitVerified {
            kind: u.kind,
            start: u.range.start,
            bytes: u.len(),
            verified_bytes: v,
        });
    }

    /// Relance un segment (ou abandonne après `max_segment_attempts`).
    fn retry_or_fail(&self, seg: Segment, attempt: u32, reason: String) -> Result<()> {
        if attempt >= self.max_segment_attempts {
            return Err(Error::SegmentExhausted {
                start: seg.range.start,
                end: seg.range.end,
                attempts: attempt,
                last: reason,
            });
        }
        self.emit(Event::SegmentRetry {
            start: seg.range.start,
            end: seg.range.end,
            attempt,
            reason,
        });
        self.push_front(seg);
        Ok(())
    }

    async fn finish_unit(&self, idx: usize, data: Vec<u8>) -> Result<()> {
        let u = self.plan.units[idx].clone();
        let dest = self.dest.clone();
        let off = u.range.start;
        let outcome = match u.kind {
            UnitKind::Chunk(i) => {
                let entry = self.chunks[i as usize];
                tokio::task::spawn_blocking(move || -> std::result::Result<(), Error> {
                    verify_chunk_frame(&entry, i, &data).map_err(Error::Core)?;
                    dest.write_at(off, &data)?;
                    Ok(())
                })
                .await
                .expect("tâche de vérification")
            }
            _ => tokio::task::spawn_blocking(move || dest.write_at(off, &data).map_err(Error::Io))
                .await
                .expect("tâche d'écriture"),
        };
        match outcome {
            Ok(()) => {
                self.mark_verified(idx);
                Ok(())
            }
            Err(Error::Core(_)) => {
                let UnitKind::Chunk(index) = u.kind else {
                    unreachable!("seuls les morceaux sont vérifiés");
                };
                self.counters
                    .chunks_rejected
                    .fetch_add(1, Ordering::Relaxed);
                self.counters
                    .bytes_rejected
                    .fetch_add(u.len(), Ordering::Relaxed);
                let attempt = {
                    let mut r = self.rejects.lock().unwrap();
                    let n = r.entry(idx).or_insert(0);
                    *n += 1;
                    *n
                };
                self.emit(Event::ChunkRejected { index, attempt });
                if attempt >= self.max_chunk_rejects {
                    return Err(Error::ChunkCorruptRepeatedly {
                        index,
                        attempts: attempt,
                    });
                }
                // Une seule requête de la taille du morceau.
                self.push_front(Segment {
                    range: u.range.clone(),
                    units: vec![idx],
                });
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn process(&self, seg: Segment) -> Result<()> {
        let attempt = {
            let mut a = self.attempts.lock().unwrap();
            let n = a.entry(seg.range.start).or_insert(0);
            *n += 1;
            *n
        };
        self.counters.requests.fetch_add(1, Ordering::Relaxed);
        let resp: RangeResponse = match self.transport.get_range(seg.range.clone()).await {
            Ok(r) => r,
            Err(e) => return self.retry_or_fail(seg, attempt, e.to_string()),
        };
        if !resp.partial || resp.start != seg.range.start {
            return Err(Error::RangeIgnored);
        }
        if let (Some(before), Some(now)) = (&self.version, &resp.version) {
            if before != now {
                return Err(Error::RemoteChanged {
                    detail: "validateur différent de la sonde".into(),
                });
            }
        }
        let mut body = resp.body;
        let mut ui = 0usize;
        let mut pos = seg.range.start;
        let mut buf: Vec<u8> = Vec::with_capacity(self.plan.units[seg.units[0]].len() as usize);
        while ui < seg.units.len() {
            if self.cancelled.load(Ordering::Acquire) {
                return Ok(());
            }
            let item: Option<std::result::Result<Bytes, crate::TransportError>> = body.next().await;
            let bytes = match item {
                Some(Ok(b)) => b,
                other => {
                    let reason = match other {
                        Some(Err(e)) => e.to_string(),
                        _ => "flux terminé avant la fin de la plage".into(),
                    };
                    self.counters.cuts.fetch_add(1, Ordering::Relaxed);
                    let resume_at = self.plan.units[seg.units[ui]].range.start;
                    self.emit(Event::SegmentCut {
                        start: seg.range.start,
                        end: seg.range.end,
                        resume_at,
                    });
                    let rest = Segment {
                        range: resume_at..seg.range.end,
                        units: seg.units[ui..].to_vec(),
                    };
                    return self.retry_or_fail(rest, attempt, reason);
                }
            };
            if let Some(l) = &self.limiter {
                l.acquire(bytes.len()).await;
            }
            self.counters
                .bytes_received
                .fetch_add(bytes.len() as u64, Ordering::Relaxed);
            let mut data = &bytes[..];
            while !data.is_empty() && ui < seg.units.len() {
                let unit = &self.plan.units[seg.units[ui]];
                let need = (unit.range.end - pos) as usize;
                let take = need.min(data.len());
                buf.extend_from_slice(&data[..take]);
                data = &data[take..];
                pos += take as u64;
                if pos == unit.range.end {
                    let full = std::mem::take(&mut buf);
                    self.finish_unit(seg.units[ui], full).await?;
                    ui += 1;
                    if ui < seg.units.len() {
                        buf.reserve(self.plan.units[seg.units[ui]].len() as usize);
                    }
                }
            }
            if !data.is_empty() {
                return Err(
                    crate::TransportError::Protocol("octets au-delà de la plage".into()).into(),
                );
            }
        }
        Ok(())
    }
}

async fn worker<T: Transport + 'static>(shared: Arc<Shared<T>>) {
    loop {
        if shared.cancelled.load(Ordering::Acquire) {
            return;
        }
        // Le garde du verrou est relâché avant toute attente.
        let (seg, idle) = {
            let mut q = shared.queue.lock().unwrap();
            match q.pop_front() {
                Some(s) => {
                    shared.in_flight.fetch_add(1, Ordering::AcqRel);
                    (Some(s), false)
                }
                None => (None, shared.in_flight.load(Ordering::Acquire) == 0),
            }
        };
        let Some(seg) = seg else {
            if idle {
                return;
            }
            // Délai borné : évite un réveil perdu entre le test et l'attente.
            let _ = tokio::time::timeout(Duration::from_millis(50), shared.notify.notified()).await;
            continue;
        };
        if let Err(e) = shared.process(seg).await {
            shared.fail(e);
        }
        shared.in_flight.fetch_sub(1, Ordering::AcqRel);
        shared.notify.notify_waiters();
    }
}

fn parse_tables_checked(trailer: &Trailer, tbytes: &[u8]) -> Result<ArchiveTables> {
    let base = trailer.chunk_index_off;
    let slice = |off: u64, len: u64| -> Result<&[u8]> {
        let s = (off - base) as usize;
        tbytes
            .get(s..s + len as usize)
            .ok_or_else(|| Error::Layout("tables hors de la plage reçue".into()))
    };
    let tables = parse_archive_tables(
        trailer,
        slice(trailer.chunk_index_off, trailer.chunk_index_len)?,
        slice(trailer.file_table_off, trailer.file_table_len)?,
        slice(trailer.hash_table_off, trailer.hash_table_len)?,
    )?;
    let actual = tables.archive_root();
    if actual != trailer.archive_root {
        return Err(Error::TablesCorrupt {
            expected: to_hex(&trailer.archive_root),
            actual: to_hex(&actual),
        });
    }
    Ok(tables)
}

fn remove_if_exists(p: &Path) -> std::io::Result<()> {
    match fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
