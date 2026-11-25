//! Téléchargement d'archives `.vault` conscient du format (ADR-0005).
//!
//! Le cœur ne parle pas HTTP : il consomme un [`Transport`] (sonde + lectures
//! par plage) et toute la logique de format vient de `vault-core`
//! (`read_trailer_tail`, `parse_archive_tables`, `verify_chunk_frame`).
//!
//! Déroulé : sonde → trailer (256 derniers octets) → contrôle de la racine
//! attendue → tables → plan d'unités alignées sur les morceaux → segments de
//! taille cible → N connexions → vérification BLAKE3 de chaque morceau à
//! l'arrivée → bitmap persistée dans `<dest>.part` → reprise sans
//! re-télécharger un octet déjà vérifié.

pub mod dest;
pub mod error;
mod fetcher;
pub mod file;
pub mod limiter;
pub mod plan;
pub mod state;
pub mod transport;

use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use vault_core::merkle::Hash;

pub use error::{Error, Result};
pub use fetcher::Fetcher;
pub use file::{local_source, FileTransport};
pub use limiter::parse_rate;
pub use plan::{Plan, Segment, Unit, UnitKind, DEFAULT_SEGMENT_BYTES};
pub use state::{part_path, PartState};
pub use transport::{
    ByteStream, RangeResponse, RemoteInfo, RemoteVersion, Transport, TransportError,
};

pub const DEFAULT_CONNECTIONS: usize = 4;
pub const DEFAULT_MAX_CHUNK_REJECTS: u32 = 3;
pub const DEFAULT_MAX_SEGMENT_ATTEMPTS: u32 = 5;

#[derive(Debug, Clone)]
pub struct FetchOptions {
    pub dest: PathBuf,
    /// Identifiant de la source (l'URL, par exemple) ; seul son hachage est
    /// persisté dans l'état de reprise.
    pub source_label: String,
    /// Racine d'archive attendue : comparée au trailer avant tout octet de contenu.
    pub expected_root: Option<Hash>,
    pub connections: usize,
    pub limit_bytes_per_sec: Option<u64>,
    /// Ne pas télécharger la section de parité ; l'état `.part` est conservé
    /// pour qu'une relance sans cette option complète l'archive.
    pub skip_parity: bool,
    /// Ignorer destination et état existants.
    pub restart: bool,
    pub segment_bytes: u64,
    pub max_chunk_rejects: u32,
    pub max_segment_attempts: u32,
    pub events: Option<UnboundedSender<Event>>,
}

impl FetchOptions {
    pub fn new(dest: impl Into<PathBuf>, source_label: impl Into<String>) -> Self {
        Self {
            dest: dest.into(),
            source_label: source_label.into(),
            expected_root: None,
            connections: DEFAULT_CONNECTIONS,
            limit_bytes_per_sec: None,
            skip_parity: false,
            restart: false,
            segment_bytes: DEFAULT_SEGMENT_BYTES,
            max_chunk_rejects: DEFAULT_MAX_CHUNK_REJECTS,
            max_segment_attempts: DEFAULT_MAX_SEGMENT_ATTEMPTS,
            events: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Probed {
        len: u64,
        accepts_ranges: bool,
    },
    Planned {
        units: usize,
        verified_units: usize,
        missing_bytes: u64,
        total_bytes: u64,
    },
    UnitVerified {
        kind: UnitKind,
        start: u64,
        bytes: u64,
        verified_bytes: u64,
    },
    ChunkRejected {
        index: u32,
        attempt: u32,
    },
    SegmentCut {
        start: u64,
        end: u64,
        resume_at: u64,
    },
    SegmentRetry {
        start: u64,
        end: u64,
        attempt: u32,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Destination déjà complète et identique à la source : rien téléchargé.
    AlreadyComplete,
    Complete,
    /// Contenu complet, parité non téléchargée (`skip_parity`) ; `.part` conservé.
    CompleteWithoutParity,
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub outcome: Outcome,
    pub archive_root: Hash,
    pub len: u64,
    /// Requêtes émises (sonde, trailer, tables, segments, relances).
    pub requests: u64,
    pub bytes_received: u64,
    pub chunks_rejected: u64,
    pub bytes_rejected: u64,
    pub cuts: u64,
    pub elapsed: Duration,
}

/// Plan en lecture seule (`--plan`).
#[derive(Debug, Clone)]
pub struct PlanReport {
    pub len: u64,
    pub archive_root: Hash,
    pub accepts_ranges: bool,
    pub units: usize,
    pub verified_units: usize,
    pub missing_bytes: u64,
    pub content_bytes: u64,
    pub segments: usize,
    pub resumed: bool,
    pub skip_parity: bool,
    pub already_complete: bool,
}
