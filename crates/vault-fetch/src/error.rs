use crate::transport::TransportError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Transport(#[from] TransportError),

    #[error("entrée/sortie : {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Core(#[from] vault_core::Error),

    #[error("la source n'annonce pas la longueur du document")]
    UnknownLength,

    #[error("racine attendue {expected}, racine annoncée par l'archive {actual} : aucun octet de contenu demandé")]
    RootMismatch { expected: String, actual: String },

    #[error("la source a changé en cours de téléchargement ({detail}) ; relancer avec --restart")]
    RemoteChanged { detail: String },

    #[error(
        "tables distantes incohérentes : racine recalculée {actual} ≠ racine du trailer {expected}"
    )]
    TablesCorrupt { expected: String, actual: String },

    #[error("disposition d'archive incohérente : {0}")]
    Layout(String),

    #[error("la source annonce les plages mais a renvoyé le document entier")]
    RangeIgnored,

    #[error("morceau {index} rejeté {attempts} fois de suite : source corrompue")]
    ChunkCorruptRepeatedly { index: u32, attempts: u32 },

    #[error("segment {start}-{end} abandonné après {attempts} tentatives : {last}")]
    SegmentExhausted {
        start: u64,
        end: u64,
        attempts: u32,
        last: String,
    },

    #[error("état de reprise illisible : {0}")]
    State(String),

    #[error("destination existante sans état de reprise et différente de la source ({detail}) ; relancer avec --restart")]
    DestinationDiffers { detail: String },

    #[error("archive téléchargée invalide : {0}")]
    FinalCheck(String),
}

pub type Result<T> = std::result::Result<T, Error>;
