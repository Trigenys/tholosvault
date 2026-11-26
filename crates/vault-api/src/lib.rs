//! API HTTP en lecture seule sur un répertoire d'archives `.vault`
//! (adaptateur mince au-dessus de `vault-core`, `docs/produit/stack.md` §5).
//!
//! Routes (`docs/api/lecture.md`) :
//!
//! | Méthode | Chemin | Réponse |
//! |---|---|---|
//! | GET | `/health` | `{"status":"ok"}` |
//! | GET | `/v1/archives` | catalogue : bilan de chaque archive |
//! | GET | `/v1/archives/{nom}` | bilan d'une archive (sans décompression) |
//! | GET | `/v1/archives/{nom}/files` | liste des fichiers (chemin, taille, racine) |
//! | GET, HEAD | `/v1/archives/{nom}/files/{*chemin}` | contenu, `Range` à une plage, morceaux vérifiés |
//! | GET | `/v1/archives/{nom}/verify?quick=true&sample=N` | rapport de vérification |
//!
//! Aucune route n'écrit : réparer reste `tholos repair`. Le contenu servi est
//! vérifié morceau par morceau (BLAKE3) ; un morceau corrompu donne 422 avant
//! le premier octet s'il est en tête de plage, sinon coupe la réponse.

pub mod catalog;
pub mod error;

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use bytes::Bytes;
use http_range_header::RangeUnsatisfiableError;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Semaphore};
use vault_core::{merkle, Archive};

pub use catalog::Catalog;
pub use error::ApiError;

/// Schéma des réponses JSON : clés ajoutées seulement, jamais renommées.
pub const SCHEMA: &str = "tholos-api/1";

/// Taille des blocs lus puis émis pour un contenu de fichier.
const PIECE: usize = 1 << 20;

/// Réglages du service.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// Répertoire des archives servies (non récursif).
    pub root: PathBuf,
    /// Vérifications simultanées (chacune occupe un cœur) ; au-delà : 503.
    pub verify_slots: usize,
}

impl ApiConfig {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            verify_slots: 2,
        }
    }
}

struct AppState {
    catalog: Catalog,
    verify_slots: Semaphore,
}

type Shared = Arc<AppState>;

/// Routeur Axum de l'API.
pub fn router(config: ApiConfig) -> Router {
    let state = Arc::new(AppState {
        catalog: Catalog::new(config.root),
        verify_slots: Semaphore::new(config.verify_slots.max(1)),
    });
    Router::new()
        .route("/health", get(|| async { Json(json!({ "status": "ok" })) }))
        .route("/v1/archives", get(list_archives))
        .route("/v1/archives/{name}", get(archive_summary))
        .route("/v1/archives/{name}/files", get(list_files))
        .route("/v1/archives/{name}/files/{*path}", get(file_content))
        .route("/v1/archives/{name}/verify", get(verify))
        .fallback(|| async {
            ApiError::NotFound("route inconnue (voir docs/api/lecture.md)".to_string())
        })
        .with_state(state)
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f).await?
}

async fn open(st: &Shared, name: String) -> Result<Arc<Archive>, ApiError> {
    let st = st.clone();
    blocking(move || st.catalog.get(&name)).await
}

/// Bilan d'une archive, sans décompression (`Archive::summary`).
pub fn summary_json(name: &str, ar: &Archive) -> Value {
    let s = ar.summary();
    json!({
        "name": name,
        "format_version": s.format_version,
        "files": s.files,
        "logical_bytes": s.logical_bytes,
        "unique_chunks": s.unique_chunks,
        "dedup_saved_bytes": s.dedup_saved_bytes(),
        "compressed_bytes": s.compressed_bytes,
        "parity_bytes": s.parity_bytes,
        "archive_bytes": s.archive_bytes,
        "ratio": s.ratio().map(|r| (r * 100.0).round() / 100.0),
        "archive_root": merkle::to_hex(&s.archive_root),
    })
}

async fn list_archives(State(st): State<Shared>) -> Result<Json<Value>, ApiError> {
    let st2 = st.clone();
    let entries = blocking(move || {
        let names = st2.catalog.names()?;
        Ok(names
            .into_iter()
            .map(|n| match st2.catalog.get(&n) {
                Ok(ar) => summary_json(&n, &ar),
                // Une archive illisible ne masque pas les autres.
                Err(e) => json!({ "name": n, "error": e.message() }),
            })
            .collect::<Vec<_>>())
    })
    .await?;
    Ok(Json(json!({ "schema": SCHEMA, "archives": entries })))
}

async fn archive_summary(
    State(st): State<Shared>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let ar = open(&st, name.clone()).await?;
    let mut v = summary_json(&name, &ar);
    v["schema"] = json!(SCHEMA);
    Ok(Json(v))
}

async fn list_files(
    State(st): State<Shared>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let ar = open(&st, name.clone()).await?;
    let files: Vec<Value> = ar
        .files()
        .iter()
        .map(|f| json!({ "path": f.path, "size": f.size, "root": merkle::to_hex(&f.root) }))
        .collect();
    Ok(Json(json!({
        "schema": SCHEMA,
        "name": name,
        "archive_root": merkle::to_hex(ar.archive_root()),
        "files": files,
    })))
}

/// Plage demandée, en octets `[start, end)`. `None` : fichier entier (pas de
/// `Range`, plage invalide ou plusieurs plages, que la RFC 9110 permet d'ignorer).
fn requested_range(headers: &HeaderMap, size: u64) -> Result<Option<(u64, u64)>, ApiError> {
    let Some(v) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) else {
        return Ok(None);
    };
    let unsatisfiable = |e: &RangeUnsatisfiableError| {
        matches!(e, RangeUnsatisfiableError::Unsatisfiable)
            .then_some(ApiError::RangeNotSatisfiable { size })
    };
    let parsed = match http_range_header::parse_range_header(v) {
        Ok(p) => p,
        Err(e) => return unsatisfiable(&e).map_or(Ok(None), Err),
    };
    match parsed.validate(size) {
        Ok(r) if r.len() == 1 => Ok(Some((*r[0].start(), *r[0].end() + 1))),
        Ok(_) => Ok(None),
        Err(e) => unsatisfiable(&e).map_or(Ok(None), Err),
    }
}

fn corrupt(path: &str, e: impl std::fmt::Display) -> ApiError {
    ApiError::Unprocessable(format!(
        "contenu de {path} illisible : {e} (vérifier avec `tholos verify`, réparer avec `tholos repair`)"
    ))
}

async fn file_content(
    State(st): State<Shared>,
    Path((name, path)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let ar = open(&st, name.clone()).await?;
    let idx = ar
        .find_index(&path)
        .ok_or_else(|| ApiError::NotFound(format!("fichier absent de {name} : {path}")))?;
    let (size, root) = (ar.files()[idx].size, ar.files()[idx].root);
    let range = requested_range(&headers, size)?;
    let (start, end) = range.unwrap_or((0, size));
    let len = end - start;

    let mut resp = Response::builder()
        .status(if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, len)
        // Racine de Merkle du fichier : validateur fort par construction.
        .header(header::ETAG, format!("\"{}\"", merkle::to_hex(&root)))
        .header("x-tholos-archive-root", merkle::to_hex(ar.archive_root()));
    if range.is_some() {
        resp = resp.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{}/{size}", end - 1),
        );
    }
    if method == Method::HEAD || len == 0 {
        return resp
            .body(Body::empty())
            .map_err(|e| ApiError::Internal(e.to_string()));
    }

    // Premier bloc lu avant l'en-tête : un morceau corrompu en tête donne un 422.
    let first_len = len.min(PIECE as u64) as usize;
    let first = {
        let (ar, path) = (ar.clone(), path.clone());
        blocking(move || {
            let mut buf = vec![0u8; first_len];
            read_exact_at(&ar, idx, start, &mut buf).map_err(|e| corrupt(&path, e))?;
            Ok(Bytes::from(buf))
        })
        .await?
    };

    let (tx, mut rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(4);
    tx.try_send(Ok(first)).expect("canal neuf");
    tokio::task::spawn_blocking(move || {
        let mut pos = start + first_len as u64;
        while pos < end {
            let n = (end - pos).min(PIECE as u64) as usize;
            let mut buf = vec![0u8; n];
            let item = read_exact_at(&ar, idx, pos, &mut buf)
                .map(|_| Bytes::from(buf))
                .map_err(|e| std::io::Error::other(corrupt(&path, e).message()));
            let failed = item.is_err();
            if tx.blocking_send(item).is_err() || failed {
                // Client parti, ou corruption : la réponse est coupée.
                return;
            }
            pos += n as u64;
        }
    });
    let stream = futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx));
    resp.body(Body::from_stream(stream))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

fn read_exact_at(
    ar: &Archive,
    file: usize,
    mut pos: u64,
    buf: &mut [u8],
) -> vault_core::Result<()> {
    let mut done = 0;
    while done < buf.len() {
        let n = ar.read_range_into(file, pos, &mut buf[done..])?;
        if n == 0 {
            return Err(vault_core::Error::MalformedSection {
                section: "table des fichiers",
                detail: format!("fichier plus court que sa taille déclarée (octet {pos})"),
            });
        }
        done += n;
        pos += n as u64;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct VerifyQuery {
    #[serde(default)]
    quick: bool,
    sample: Option<usize>,
}

async fn verify(
    State(st): State<Shared>,
    Path(name): Path<String>,
    Query(q): Query<VerifyQuery>,
) -> Result<Json<Value>, ApiError> {
    let _slot = st.verify_slots.try_acquire().map_err(|_| {
        ApiError::Busy("vérifications en cours au maximum ; réessayer plus tard".to_string())
    })?;
    let ar = open(&st, name.clone()).await?;
    let sample = q.sample.unwrap_or(64);
    let quick = q.quick;
    let v = blocking(move || {
        let r = if quick {
            ar.verify_quick(sample)
        } else {
            ar.verify()
        };
        let d = r.damage.as_ref();
        Ok(json!({
            "schema": SCHEMA,
            "name": name,
            "mode": if quick { "quick" } else { "full" },
            "ok": r.is_ok(),
            "needs_repair": d.is_some_and(|d| d.needs_repair()),
            "repairable": r.repairable(),
            "chunks_checked": r.chunks_checked,
            "chunks_total": ar.chunks().len(),
            "bad_chunks": r.bad_chunks,
            "bad_files": r.bad_files,
            "archive_root_ok": r.archive_root_ok,
            "bad_data_shards": d.map_or(0, |d| d.bad_data_shards.len()),
            "bad_parity_shards": d.map_or(0, |d| d.bad_parity_shards.len()),
            "bad_trailer_copies": d.map_or(0, |d| d.bad_trailer_copies),
            "archive_root": merkle::to_hex(ar.archive_root()),
        }))
    })
    .await?;
    Ok(Json(v))
}
