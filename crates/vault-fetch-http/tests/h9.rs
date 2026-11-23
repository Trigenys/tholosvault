//! Thèses H9a / H9e (ADR-0005) sur serveur local avec injection de pannes.

use std::fs;
use std::ops::Range;
use std::path::Path;

use tempfile::TempDir;
use vault_core::{pack_dir, Archive, ChunkParams, PackOptions, ParityOptions};
use vault_fetch::plan::TRAILER_TAIL_LEN;
use vault_fetch::{Error, Event, FetchOptions, Fetcher, Outcome, Summary, UnitKind};
use vault_fetch_http::test_server::{CorruptAt, Cut, TestServer};
use vault_fetch_http::{HttpTransportBuilder, RetryPolicy};

const SEGMENT: u64 = 256 * 1024;

/// Archive de test : ~1,5 Mio de contenu peu compressible, morceaux de 16 Kio.
fn fixture(dir: &Path, seed: u64) -> Vec<u8> {
    let src = dir.join("src");
    fs::create_dir_all(&src).unwrap();
    let mut x = seed.wrapping_mul(0x9e3779b97f4a7c15) | 1;
    for f in 0..3 {
        let mut data = Vec::with_capacity(512 * 1024);
        while data.len() < 512 * 1024 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            data.extend_from_slice(&x.to_le_bytes());
        }
        fs::write(src.join(format!("f{f}.bin")), data).unwrap();
    }
    let out = dir.join("fixture.vault");
    pack_dir(
        &src,
        &out,
        &PackOptions {
            level: 1,
            chunk: ChunkParams {
                min: 4096,
                avg: 16 * 1024,
                max: 64 * 1024,
            },
            parity: ParityOptions::default(),
        },
    )
    .unwrap();
    fs::read(&out).unwrap()
}

fn chunk_ranges(archive: &Path) -> Vec<Range<u64>> {
    let ar = Archive::open(archive).unwrap();
    ar.chunks()
        .iter()
        .map(|c| c.offset..c.offset + c.compressed_len as u64)
        .collect()
}

struct Harness {
    _dir: TempDir,
    tables_start: u64,
    server: TestServer,
    data: Vec<u8>,
    chunks: Vec<Range<u64>>,
    dest: std::path::PathBuf,
}

async fn harness(seed: u64) -> Harness {
    let dir = TempDir::new().unwrap();
    let data = fixture(dir.path(), seed);
    let fixture_path = dir.path().join("fixture.vault");
    let chunks = chunk_ranges(&fixture_path);
    let tables_start = Archive::open(&fixture_path)
        .unwrap()
        .trailer()
        .chunk_index_off;
    assert!(
        chunks.len() > 20,
        "fixture trop petite : {} morceaux",
        chunks.len()
    );
    let server = TestServer::start(data.clone()).await;
    Harness {
        dest: dir.path().join("out.vault"),
        _dir: dir,
        tables_start,
        server,
        data,
        chunks,
    }
}

impl Harness {
    fn opts(&self) -> FetchOptions {
        let mut o = FetchOptions::new(&self.dest, self.server.url());
        o.segment_bytes = SEGMENT;
        o
    }

    fn fetcher(&self, opts: FetchOptions) -> Fetcher<vault_fetch_http::HttpTransport> {
        let t = HttpTransportBuilder::new(self.server.url())
            .retry(RetryPolicy {
                initial: std::time::Duration::from_millis(10),
                ..RetryPolicy::default()
            })
            .build()
            .unwrap();
        Fetcher::new(t, opts)
    }

    async fn run(&self, opts: FetchOptions) -> vault_fetch::Result<Summary> {
        self.fetcher(opts).run().await
    }

    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn assert_dest_identical(&self) {
        assert_eq!(fs::read(&self.dest).unwrap(), self.data);
        assert!(
            !vault_fetch::part_path(&self.dest).exists(),
            ".part doit disparaître"
        );
    }

    /// Requêtes GET ayant demandé au moins un octet de contenu (hors tables et trailer).
    fn content_gets(&self) -> usize {
        let tables_start = self.tables_start;
        self.server
            .log()
            .iter()
            .filter(|r| r.method == "GET")
            .filter(|r| {
                r.range
                    .as_ref()
                    .map(|x| x.start < tables_start)
                    .unwrap_or(true)
            })
            .count()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn clean_fetch_is_identical() {
    let h = harness(1).await;
    let s = h.run(h.opts()).await.unwrap();
    assert_eq!(s.outcome, Outcome::Complete);
    assert_eq!(s.chunks_rejected, 0);
    h.assert_dest_identical();
}

/// H9a : une corruption injectée coûte exactement un morceau et une requête.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn corrupt_chunk_costs_one_chunk_one_request() {
    let h = harness(2).await;
    let clean = h.run(h.opts()).await.unwrap();
    h.server.clear_log();
    fs::remove_file(&h.dest).unwrap();

    let target = h.chunks[7].clone();
    h.server.faults().corrupt = Some(CorruptAt {
        offset: target.start + 3,
        remaining: 1,
    });
    let s = h.run(h.opts()).await.unwrap();
    assert_eq!(s.outcome, Outcome::Complete);
    assert_eq!(s.chunks_rejected, 1);
    assert_eq!(s.bytes_rejected, target.end - target.start);
    assert_eq!(
        s.requests,
        clean.requests + 1,
        "une seule requête supplémentaire"
    );
    let exact: Vec<_> = h
        .server
        .log()
        .into_iter()
        .filter(|r| r.method == "GET" && r.range.as_ref() == Some(&target))
        .collect();
    assert_eq!(
        exact.len(),
        1,
        "la relance ne demande que le morceau rejeté"
    );
    h.assert_dest_identical();
}

/// H9a : après une coupure, la reprise ne re-télécharge aucun octet déjà vérifié.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resume_after_cut_redownloads_zero_verified_bytes() {
    let h = harness(3).await;
    let mid = h.chunks[h.chunks.len() / 2].clone();
    h.server.faults().cut = Some(Cut {
        at: mid.start,
        after: 1000,
        remaining: 1,
    });
    // Première passe : aucune relance interne, la coupure interrompt le run
    // comme le ferait un arrêt du processus.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let mut o = h.opts();
    o.max_segment_attempts = 1;
    o.events = Some(tx);
    let err = h
        .run(o)
        .await
        .expect_err("la coupure doit interrompre la passe");
    assert!(matches!(err, Error::SegmentExhausted { .. }), "{err}");
    let mut verified: Vec<Range<u64>> = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        if let Event::UnitVerified { start, bytes, .. } = ev {
            verified.push(start..start + bytes);
        }
    }
    assert!(!verified.is_empty());
    assert!(vault_fetch::part_path(&h.dest).exists());

    h.server.clear_log();
    let s = h.run(h.opts()).await.unwrap();
    assert_eq!(s.outcome, Outcome::Complete);
    let rebytes: u64 = verified
        .iter()
        .map(|r| h.server.bytes_served_within(r.clone()))
        .sum();
    assert_eq!(rebytes, 0, "octets re-téléchargés déjà vérifiés");
    // Validateur fort inchangé : tables et trailer relus localement.
    let tail_gets = h
        .server
        .log()
        .iter()
        .filter(|r| {
            r.method == "GET"
                && r.range
                    .as_ref()
                    .map(|x| x.start >= h.len() - TRAILER_TAIL_LEN)
                    .unwrap_or(false)
        })
        .count();
    assert_eq!(tail_gets, 0);
    h.assert_dest_identical();
}

/// H9e : racine attendue différente → arrêt avant tout octet de contenu.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_mismatch_stops_before_any_content_byte() {
    let h = harness(4).await;
    let mut o = h.opts();
    o.expected_root = Some([0xabu8; 32]);
    let err = h.run(o).await.err().unwrap();
    assert!(matches!(err, Error::RootMismatch { .. }), "{err}");
    assert_eq!(h.content_gets(), 0);
    assert!(!h.dest.exists());
    assert!(!vault_fetch::part_path(&h.dest).exists());
}

/// Idempotence : relancer sur une archive complète n'émet aucune requête de contenu.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rerun_on_complete_archive_requests_no_content() {
    let h = harness(5).await;
    h.run(h.opts()).await.unwrap();
    h.server.clear_log();
    let s = h.run(h.opts()).await.unwrap();
    assert_eq!(s.outcome, Outcome::AlreadyComplete);
    assert_eq!(h.content_gets(), 0);
    assert_eq!(s.requests, 2, "sonde + trailer");
    h.assert_dest_identical();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn source_without_ranges_falls_back_to_single_stream() {
    let h = harness(6).await;
    {
        let mut f = h.server.faults();
        f.ignore_range = true;
        f.no_head = true;
    }
    let s = h.run(h.opts()).await.unwrap();
    assert_eq!(s.outcome, Outcome::Complete);
    assert_eq!(s.bytes_received, h.len(), "un seul flux complet compté");
    assert!(h
        .server
        .log()
        .iter()
        .all(|r| r.status == 200 || r.status == 405));
    h.assert_dest_identical();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_change_during_resume_is_refused() {
    let h = harness(7).await;
    let mid = h.chunks[h.chunks.len() / 2].clone();
    h.server.faults().cut = Some(Cut {
        at: mid.start,
        after: 10,
        remaining: 1,
    });
    let mut o = h.opts();
    o.max_segment_attempts = 1;
    assert!(h.run(o).await.is_err());
    // Nouvelle version de même longueur : validateur fort différent.
    let mut changed = h.data.clone();
    changed[mid.start as usize] ^= 0x01;
    h.server.replace(changed);
    let err = h.run(h.opts()).await.err().unwrap();
    assert!(matches!(err, Error::RemoteChanged { .. }), "{err}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_after_on_503_is_honoured() {
    let h = harness(8).await;
    {
        let mut f = h.server.faults();
        f.unavailable_next = 1;
        f.retry_after_secs = 0;
    }
    let s = h.run(h.opts()).await.unwrap();
    assert_eq!(s.outcome, Outcome::Complete);
    assert!(h.server.log().iter().any(|r| r.status == 503));
    h.assert_dest_identical();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skip_parity_then_complete() {
    let h = harness(9).await;
    let mut o = h.opts();
    o.skip_parity = true;
    let s = h.run(o).await.unwrap();
    assert_eq!(s.outcome, Outcome::CompleteWithoutParity);
    assert!(vault_fetch::part_path(&h.dest).exists());
    let ar = Archive::open(&h.dest).unwrap();
    assert!(ar.verify().is_ok(), "contenu intègre sans parité");
    h.server.clear_log();
    let s = h.run(h.opts()).await.unwrap();
    assert_eq!(s.outcome, Outcome::Complete);
    let parity_only = h
        .server
        .log()
        .iter()
        .filter(|r| r.method == "GET")
        .all(|r| {
            r.range
                .as_ref()
                .map(|x| x.start >= ar.trailer().parity_off())
                .unwrap_or(false)
        });
    assert!(parity_only, "seule la parité est demandée");
    h.assert_dest_identical();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_is_read_only() {
    let h = harness(10).await;
    let r = h.fetcher(h.opts()).plan().await.unwrap();
    assert_eq!(r.len, h.len());
    assert!(r.segments > 1);
    assert!(!h.dest.exists());
    assert!(!vault_fetch::part_path(&h.dest).exists());
    assert_eq!(h.content_gets(), 0);
    let _ = UnitKind::Header;
}
