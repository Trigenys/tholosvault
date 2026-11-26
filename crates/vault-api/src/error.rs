//! Erreurs de l'API : statut HTTP et corps JSON `{"error": "..."}` en français,
//! chaque message disant quoi faire.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    /// Plage hors du fichier : 416 avec `Content-Range: bytes */<taille>`.
    RangeNotSatisfiable {
        size: u64,
    },
    /// Archive illisible ou contenu corrompu.
    Unprocessable(String),
    /// Trop de vérifications simultanées.
    Busy(String),
    Internal(String),
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::RangeNotSatisfiable { .. } => StatusCode::RANGE_NOT_SATISFIABLE,
            Self::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Busy(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::BadRequest(m)
            | Self::NotFound(m)
            | Self::Unprocessable(m)
            | Self::Busy(m)
            | Self::Internal(m) => m.clone(),
            Self::RangeNotSatisfiable { size } => {
                format!("plage hors du fichier ({size} octets)")
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut r = (self.status(), Json(json!({ "error": self.message() }))).into_response();
        match self {
            Self::RangeNotSatisfiable { size } => {
                if let Ok(v) = HeaderValue::from_str(&format!("bytes */{size}")) {
                    r.headers_mut().insert(header::CONTENT_RANGE, v);
                }
            }
            Self::Busy(_) => {
                r.headers_mut()
                    .insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
            }
            _ => {}
        }
        r
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::Internal(format!("tâche interrompue : {e}"))
    }
}
