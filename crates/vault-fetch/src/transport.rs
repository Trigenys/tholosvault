//! Contrat de transport : le cœur ne connaît ni HTTP ni le format de l'URL.
//! Un adaptateur (`vault-fetch-http`) fournit une sonde et des lectures par
//! plage d'octets ; les en-têtes d'authentification restent dans l'adaptateur.

use std::ops::Range;
use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::Stream;

/// Identité de version annoncée par la source (ETag HTTP, par exemple).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteVersion {
    /// Validateur fort : deux valeurs égales garantissent des octets identiques.
    Strong(String),
    /// Validateur faible : ne suffit pas à réutiliser un état de reprise.
    Weak(String),
}

impl RemoteVersion {
    pub fn strong(&self) -> Option<&str> {
        match self {
            RemoteVersion::Strong(s) => Some(s),
            RemoteVersion::Weak(_) => None,
        }
    }
}

/// Résultat de la sonde initiale (HEAD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteInfo {
    /// Longueur totale si la source l'annonce.
    pub len: Option<u64>,
    /// La source accepte les requêtes par plage.
    pub accepts_ranges: bool,
    pub version: Option<RemoteVersion>,
}

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, TransportError>> + Send>>;

/// Réponse à une lecture par plage.
pub struct RangeResponse {
    pub version: Option<RemoteVersion>,
    /// `true` si la source a honoré la plage (206) ; `false` si elle renvoie
    /// le document entier depuis l'octet 0 (200).
    pub partial: bool,
    /// Premier octet servi (égal au début de la plage si `partial`).
    pub start: u64,
    pub body: ByteStream,
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("statut {code} renvoyé par la source")]
    Status { code: u16 },
    #[error("erreur réseau : {0}")]
    Network(String),
    #[error("réponse invalide : {0}")]
    Protocol(String),
}

#[async_trait]
pub trait Transport: Send + Sync {
    async fn probe(&self) -> Result<RemoteInfo, TransportError>;
    async fn get_range(&self, range: Range<u64>) -> Result<RangeResponse, TransportError>;
}
