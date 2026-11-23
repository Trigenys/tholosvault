//! Adaptateur HTTP pour `vault-fetch` : sonde (`HEAD`, repli `GET Range: 0-0`),
//! lectures par plage, validateurs (`ETag`, `If-Range`), relances avec repli
//! exponentiel plafonné et respect de `Retry-After`.
//!
//! Il ne connaît rien du format : seul `vault-fetch` décide quoi demander.
//! Les en-têtes injectés (authentification, par exemple) ne quittent jamais
//! ce module et n'apparaissent dans aucun message d'erreur.

use std::ops::Range;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::TryStreamExt;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, ETAG,
    IF_RANGE, RANGE, RETRY_AFTER,
};
use reqwest::{Client, Method, Response, StatusCode};
use vault_fetch::{
    ByteStream, RangeResponse, RemoteInfo, RemoteVersion, Transport, TransportError,
};

#[cfg(feature = "test-server")]
pub mod test_server;

pub const DEFAULT_MAX_ATTEMPTS: u32 = 5;
pub const DEFAULT_BACKOFF_INITIAL: Duration = Duration::from_millis(500);
pub const DEFAULT_BACKOFF_CAP: Duration = Duration::from_secs(8);

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("URL invalide : {0}")]
    Url(String),
    #[error("en-tête invalide : {0}")]
    Header(String),
    #[error("client HTTP : {0}")]
    Client(String),
}

/// Politique de relance d'une requête (par tentative de transport).
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial: Duration,
    pub cap: Duration,
    /// Plafond appliqué à `Retry-After` pour ne pas rester bloqué.
    pub retry_after_cap: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            initial: DEFAULT_BACKOFF_INITIAL,
            cap: DEFAULT_BACKOFF_CAP,
            retry_after_cap: Duration::from_secs(60),
        }
    }
}

impl RetryPolicy {
    /// Aucune relance : utile en test pour observer chaque échec.
    pub fn none() -> Self {
        Self {
            max_attempts: 1,
            ..Self::default()
        }
    }

    fn backoff(&self, attempt: u32) -> Duration {
        let mult = 1u32
            .checked_shl(attempt.saturating_sub(1))
            .unwrap_or(u32::MAX);
        self.initial.saturating_mul(mult).min(self.cap)
    }
}

pub struct HttpTransportBuilder {
    url: String,
    headers: HeaderMap,
    retry: RetryPolicy,
    timeout: Duration,
    user_agent: String,
}

impl HttpTransportBuilder {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: HeaderMap::new(),
            retry: RetryPolicy::default(),
            timeout: Duration::from_secs(30),
            user_agent: format!("tholos-fetch/{}", env!("CARGO_PKG_VERSION")),
        }
    }

    /// En-tête envoyé à chaque requête (jamais journalisé ni persisté).
    pub fn header(mut self, name: &str, value: &str) -> Result<Self, BuildError> {
        let n =
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| BuildError::Header(name.into()))?;
        let mut v = HeaderValue::from_str(value).map_err(|_| BuildError::Header(name.into()))?;
        v.set_sensitive(true);
        self.headers.insert(n, v);
        Ok(self)
    }

    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// Délai sans octet reçu avant de considérer la connexion coupée.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    pub fn build(self) -> Result<HttpTransport, BuildError> {
        let url: reqwest::Url = self
            .url
            .parse()
            .map_err(|e| BuildError::Url(format!("{e}")))?;
        // Fournisseur cryptographique `ring` (pas de cmake/NASM) ; l'installation
        // échoue sans conséquence si un fournisseur est déjà en place.
        let _ = rustls::crypto::ring::default_provider().install_default();
        // Les relances sont la responsabilité de cet adaptateur (visibles,
        // comptées, bornées) : la couche de relance implicite de reqwest est
        // désactivée pour qu'aucune requête ne soit réémise à son insu.
        let client = Client::builder()
            .retry(reqwest::retry::never())
            .user_agent(self.user_agent)
            .default_headers(self.headers)
            .read_timeout(self.timeout)
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| BuildError::Client(e.to_string()))?;
        Ok(HttpTransport {
            client,
            url,
            retry: self.retry,
        })
    }
}

pub struct HttpTransport {
    client: Client,
    url: reqwest::Url,
    retry: RetryPolicy,
}

fn parse_etag(h: &HeaderMap) -> Option<RemoteVersion> {
    let v = h.get(ETAG)?.to_str().ok()?.trim();
    if v.is_empty() {
        return None;
    }
    if let Some(rest) = v.strip_prefix("W/") {
        Some(RemoteVersion::Weak(rest.to_string()))
    } else {
        Some(RemoteVersion::Strong(v.to_string()))
    }
}

fn header_u64(h: &HeaderMap, name: &HeaderName) -> Option<u64> {
    h.get(name)?.to_str().ok()?.trim().parse().ok()
}

/// `bytes a-b/total` → `(a, total)` ; `total` peut être `*`.
fn parse_content_range(h: &HeaderMap) -> Option<(u64, Option<u64>)> {
    let v = h.get(CONTENT_RANGE)?.to_str().ok()?.trim();
    let rest = v.strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (start, _) = range.split_once('-')?;
    let start = start.trim().parse().ok()?;
    let total = total.trim().parse().ok();
    Some((start, total))
}

fn retry_after(h: &HeaderMap) -> Option<Duration> {
    let v = h.get(RETRY_AFTER)?.to_str().ok()?.trim();
    v.parse::<u64>().ok().map(Duration::from_secs)
}

fn retryable(code: StatusCode) -> bool {
    matches!(code.as_u16(), 408 | 425 | 429 | 500 | 502 | 503 | 504)
}

impl HttpTransport {
    /// Envoie la requête ; relance sur erreur réseau ou statut transitoire.
    async fn send(
        &self,
        method: Method,
        range: Option<&Range<u64>>,
        if_range: Option<&str>,
    ) -> Result<Response, TransportError> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let mut req = self.client.request(method.clone(), self.url.clone());
            if let Some(r) = range {
                req = req.header(RANGE, format!("bytes={}-{}", r.start, r.end - 1));
            }
            if let Some(v) = if_range {
                req = req.header(IF_RANGE, v);
            }
            let outcome = match req.send().await {
                Ok(resp) => {
                    let code = resp.status();
                    if code.is_success() || code == StatusCode::RANGE_NOT_SATISFIABLE {
                        return Ok(resp);
                    }
                    let wait = retry_after(resp.headers());
                    if retryable(code) {
                        Err((
                            TransportError::Status {
                                code: code.as_u16(),
                            },
                            wait,
                        ))
                    } else {
                        return Err(TransportError::Status {
                            code: code.as_u16(),
                        });
                    }
                }
                Err(e) => Err((TransportError::Network(sanitize(&e)), None)),
            };
            let (err, wait) = match outcome {
                Ok(r) => return Ok(r),
                Err(x) => x,
            };
            if attempt >= self.retry.max_attempts {
                return Err(err);
            }
            let delay = wait
                .map(|w| w.min(self.retry.retry_after_cap))
                .unwrap_or_else(|| self.retry.backoff(attempt));
            tokio::time::sleep(delay).await;
        }
    }
}

/// Message d'erreur sans l'URL (qui peut porter un jeton en requête).
fn sanitize(e: &reqwest::Error) -> String {
    let mut s = e.to_string();
    if let Some(u) = e.url() {
        s = s.replace(u.as_str(), "<url>");
    }
    s
}

#[async_trait]
impl Transport for HttpTransport {
    async fn probe(&self) -> Result<RemoteInfo, TransportError> {
        // HEAD d'abord ; certains serveurs le refusent : repli sur une plage d'un octet.
        let head = self.send(Method::HEAD, None, None).await;
        if let Ok(resp) = &head {
            if resp.status() == StatusCode::OK {
                let h = resp.headers();
                let accepts = h
                    .get(ACCEPT_RANGES)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.eq_ignore_ascii_case("bytes"))
                    .unwrap_or(false);
                return Ok(RemoteInfo {
                    len: header_u64(h, &CONTENT_LENGTH),
                    accepts_ranges: accepts,
                    version: parse_etag(h),
                });
            }
        }
        let resp = self.send(Method::GET, Some(&(0..1)), None).await?;
        let h = resp.headers().clone();
        let version = parse_etag(&h);
        match resp.status() {
            StatusCode::PARTIAL_CONTENT => {
                let (_, total) = parse_content_range(&h).ok_or_else(|| {
                    TransportError::Protocol("Content-Range absent ou invalide".into())
                })?;
                Ok(RemoteInfo {
                    len: total,
                    accepts_ranges: true,
                    version,
                })
            }
            _ => Ok(RemoteInfo {
                len: header_u64(&h, &CONTENT_LENGTH),
                accepts_ranges: false,
                version,
            }),
        }
    }

    async fn get_range(&self, range: Range<u64>) -> Result<RangeResponse, TransportError> {
        let resp = self.send(Method::GET, Some(&range), None).await?;
        let code = resp.status();
        let h = resp.headers();
        let version = parse_etag(h);
        let (partial, start) = match code {
            StatusCode::PARTIAL_CONTENT => {
                let (start, _) = parse_content_range(h).ok_or_else(|| {
                    TransportError::Protocol("Content-Range absent ou invalide".into())
                })?;
                (true, start)
            }
            StatusCode::OK => (false, 0),
            StatusCode::RANGE_NOT_SATISFIABLE => {
                return Err(TransportError::Protocol(format!(
                    "plage {}..{} refusée (416)",
                    range.start, range.end
                )))
            }
            other => {
                return Err(TransportError::Status {
                    code: other.as_u16(),
                })
            }
        };
        let body: ByteStream = Box::pin(
            resp.bytes_stream()
                .map_err(|e| TransportError::Network(sanitize(&e))),
        );
        Ok(RangeResponse {
            version,
            partial,
            start,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_capped() {
        let p = RetryPolicy::default();
        assert_eq!(p.backoff(1), Duration::from_millis(500));
        assert_eq!(p.backoff(2), Duration::from_secs(1));
        assert_eq!(p.backoff(10), DEFAULT_BACKOFF_CAP);
    }

    #[test]
    fn parses_validators_and_ranges() {
        let mut h = HeaderMap::new();
        h.insert(ETAG, HeaderValue::from_static("W/\"abc\""));
        assert_eq!(parse_etag(&h), Some(RemoteVersion::Weak("\"abc\"".into())));
        h.insert(ETAG, HeaderValue::from_static("\"abc\""));
        assert_eq!(
            parse_etag(&h),
            Some(RemoteVersion::Strong("\"abc\"".into()))
        );
        h.insert(CONTENT_RANGE, HeaderValue::from_static("bytes 10-19/100"));
        assert_eq!(parse_content_range(&h), Some((10, Some(100))));
        h.insert(CONTENT_RANGE, HeaderValue::from_static("bytes 10-19/*"));
        assert_eq!(parse_content_range(&h), Some((10, None)));
    }

    #[test]
    fn sensitive_header_never_in_error() {
        let b = HttpTransportBuilder::new("http://127.0.0.1:1/x")
            .header("Authorization", "Bearer secret-token")
            .unwrap()
            .build()
            .unwrap();
        assert!(!format!("{:?}", b.client).contains("secret-token"));
    }
}
