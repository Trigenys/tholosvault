//! Serveur HTTP local de test (axum) : sert un document en mémoire avec
//! injection de pannes et journal des requêtes. Réservé aux tests et au banc.

use std::net::SocketAddr;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use bytes::Bytes;
use futures_util::stream::{self, StreamExt as _};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedRequest {
    pub method: String,
    /// Plage demandée (`Range`), si présente.
    pub range: Option<Range<u64>>,
    pub status: u16,
    /// Octets effectivement émis (avant une coupure éventuelle).
    pub bytes_served: u64,
}

/// Pannes injectées ; chacune se consomme ou se maintient selon son champ.
#[derive(Debug, Default)]
pub struct Faults {
    /// Ignorer `Range` : toujours 200 avec le document entier.
    pub ignore_range: bool,
    /// Ne pas annoncer `Accept-Ranges` ni honorer HEAD (405).
    pub no_head: bool,
    /// Corrompre l'octet à cette position lors de la N-ième requête qui le
    /// couvre (compteur décrémenté à chaque service de cette position).
    pub corrupt: Option<CorruptAt>,
    /// Couper le flux après avoir émis `after` octets des requêtes couvrant
    /// l'octet `at` ; consommé `times` fois.
    pub cut: Option<Cut>,
    /// Répondre 503 + Retry-After à ce nombre de prochaines requêtes GET.
    pub unavailable_next: u32,
    pub retry_after_secs: u64,
    /// Validateur faible au lieu d'un fort.
    pub weak_etag: bool,
    /// Ne pas envoyer d'ETag.
    pub no_etag: bool,
    /// Délai artificiel par réponse (simulation de latence).
    pub latency: Duration,
}

#[derive(Debug, Clone)]
pub struct CorruptAt {
    pub offset: u64,
    /// Nombre de services restants qui seront corrompus.
    pub remaining: u32,
}

#[derive(Debug, Clone)]
pub struct Cut {
    pub at: u64,
    pub after: u64,
    pub remaining: u32,
}

struct Inner {
    data: Mutex<Bytes>,
    etag: Mutex<String>,
    faults: Mutex<Faults>,
    log: Mutex<Vec<LoggedRequest>>,
    content_requests: AtomicU64,
}

#[derive(Clone)]
pub struct TestServer {
    inner: Arc<Inner>,
    addr: SocketAddr,
    handle: Arc<JoinHandle<()>>,
}

impl TestServer {
    pub async fn start(data: Vec<u8>) -> TestServer {
        let etag = format!("\"{}\"", fingerprint(&data));
        let inner = Arc::new(Inner {
            data: Mutex::new(Bytes::from(data)),
            etag: Mutex::new(etag),
            faults: Mutex::new(Faults::default()),
            log: Mutex::new(Vec::new()),
            content_requests: AtomicU64::new(0),
        });
        let app = Router::new()
            .route("/archive.vault", get(serve).head(serve))
            .with_state(inner.clone());
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("bind");
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        TestServer {
            inner,
            addr,
            handle: Arc::new(handle),
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}/archive.vault", self.addr)
    }

    pub fn faults(&self) -> std::sync::MutexGuard<'_, Faults> {
        self.inner.faults.lock().unwrap()
    }

    pub fn log(&self) -> Vec<LoggedRequest> {
        self.inner.log.lock().unwrap().clone()
    }

    pub fn clear_log(&self) {
        self.inner.log.lock().unwrap().clear();
    }

    pub fn len(&self) -> u64 {
        self.inner.data.lock().unwrap().len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn etag(&self) -> String {
        self.inner.etag.lock().unwrap().clone()
    }

    /// Remplace le document (et son ETag) : simule une nouvelle version.
    pub fn replace(&self, data: Vec<u8>) {
        *self.inner.etag.lock().unwrap() = format!("\"{}\"", fingerprint(&data));
        *self.inner.data.lock().unwrap() = Bytes::from(data);
    }

    /// Octets servis par les requêtes GET dont la plage recoupe `range`.
    pub fn bytes_served_within(&self, range: Range<u64>) -> u64 {
        self.log()
            .iter()
            .filter(|r| r.method == "GET")
            .map(|r| {
                let served = r.range.clone().unwrap_or(0..r.bytes_served);
                let served = served.start..served.start + r.bytes_served;
                let s = served.start.max(range.start);
                let e = served.end.min(range.end);
                e.saturating_sub(s)
            })
            .sum()
    }

    pub fn abort(&self) {
        self.handle.abort();
    }
}

fn fingerprint(data: &[u8]) -> String {
    // Dépendance volontairement évitée : empreinte simple suffisante en test.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}-{}", data.len())
}

fn parse_range(h: &HeaderMap, len: u64) -> Option<Range<u64>> {
    let v = h.get(header::RANGE)?.to_str().ok()?;
    let spec = v.strip_prefix("bytes=")?;
    let (a, b) = spec.split_once('-')?;
    let start: u64 = a.trim().parse().ok()?;
    let end: u64 = if b.trim().is_empty() {
        len
    } else {
        b.trim().parse::<u64>().ok()? + 1
    };
    Some(start..end.min(len))
}

async fn serve(State(inner): State<Arc<Inner>>, method: Method, headers: HeaderMap) -> Response {
    let data = inner.data.lock().unwrap().clone();
    let len = data.len() as u64;
    let etag = inner.etag.lock().unwrap().clone();
    let requested = parse_range(&headers, len);
    let (no_head, ignore_range, weak, no_etag, latency, unavailable, cut, corrupt) = {
        let mut f = inner.faults.lock().unwrap();
        let unavailable = if method == Method::GET && f.unavailable_next > 0 {
            f.unavailable_next -= 1;
            Some(f.retry_after_secs)
        } else {
            None
        };
        let covers = |o: u64| {
            requested
                .as_ref()
                .map(|r| r.contains(&o))
                .unwrap_or(o < len)
        };
        let cut = match &mut f.cut {
            Some(c) if method == Method::GET && c.remaining > 0 && covers(c.at) => {
                c.remaining -= 1;
                Some(c.after)
            }
            _ => None,
        };
        let corrupt = match &mut f.corrupt {
            Some(c) if method == Method::GET && c.remaining > 0 && covers(c.offset) => {
                c.remaining -= 1;
                Some(c.offset)
            }
            _ => None,
        };
        (
            f.no_head,
            f.ignore_range,
            f.weak_etag,
            f.no_etag,
            f.latency,
            unavailable,
            cut,
            corrupt,
        )
    };
    if !latency.is_zero() {
        tokio::time::sleep(latency).await;
    }
    let log = |status: u16, served: u64| {
        inner.log.lock().unwrap().push(LoggedRequest {
            method: method.to_string(),
            range: requested.clone(),
            status,
            bytes_served: served,
        });
    };
    let mut resp_headers = HeaderMap::new();
    if !no_etag {
        let v = if weak {
            format!("W/{etag}")
        } else {
            etag.clone()
        };
        resp_headers.insert(header::ETAG, v.parse().unwrap());
    }
    if !no_head && !ignore_range {
        resp_headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    }

    if method == Method::HEAD {
        if no_head {
            log(405, 0);
            return StatusCode::METHOD_NOT_ALLOWED.into_response();
        }
        resp_headers.insert(header::CONTENT_LENGTH, len.to_string().parse().unwrap());
        log(200, 0);
        return (StatusCode::OK, resp_headers).into_response();
    }
    if let Some(secs) = unavailable {
        resp_headers.insert(header::RETRY_AFTER, secs.to_string().parse().unwrap());
        log(503, 0);
        return (StatusCode::SERVICE_UNAVAILABLE, resp_headers).into_response();
    }

    let (status, range) = match (&requested, ignore_range) {
        (Some(r), false) => {
            if r.start >= len || r.is_empty() {
                resp_headers.insert(
                    header::CONTENT_RANGE,
                    format!("bytes */{len}").parse().unwrap(),
                );
                log(416, 0);
                return (StatusCode::RANGE_NOT_SATISFIABLE, resp_headers).into_response();
            }
            resp_headers.insert(
                header::CONTENT_RANGE,
                format!("bytes {}-{}/{len}", r.start, r.end - 1)
                    .parse()
                    .unwrap(),
            );
            (StatusCode::PARTIAL_CONTENT, r.clone())
        }
        _ => (StatusCode::OK, 0..len),
    };
    inner.content_requests.fetch_add(1, Ordering::Relaxed);
    let mut body = data
        .slice(range.start as usize..range.end as usize)
        .to_vec();
    if let Some(off) = corrupt {
        body[(off - range.start) as usize] ^= 0xff;
    }
    let total = body.len() as u64;
    let served = match cut {
        Some(after) => after.min(total),
        None => total,
    };
    resp_headers.insert(header::CONTENT_LENGTH, total.to_string().parse().unwrap());
    log(status.as_u16(), served);
    let first = Bytes::from(body).slice(0..served as usize);
    if cut.is_some() {
        // Les octets servis sont d'abord écrits sur la connexion (la pause
        // laisse hyper vider son tampon), puis la connexion est réinitialisée :
        // le client voit un corps tronqué, pas une requête jamais répondue.
        let s = stream::iter([Ok::<Bytes, std::io::Error>(first)]).chain(stream::once(async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "coupure simulée",
            ))
        }));
        return (status, resp_headers, Body::from_stream(s)).into_response();
    }
    // Flux en morceaux pour exercer le découpage côté client.
    let chunks: Vec<Result<Bytes, std::io::Error>> = first
        .chunks(64 * 1024)
        .map(|c| Ok(Bytes::copy_from_slice(c)))
        .collect();
    (
        status,
        resp_headers,
        Body::from_stream(stream::iter(chunks)),
    )
        .into_response()
}
