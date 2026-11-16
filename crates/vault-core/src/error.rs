use std::path::PathBuf;

/// Erreurs du cœur. Chaque variante est actionnable : elle nomme ce qui a échoué
/// et, quand c'est pertinent, où.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("E/S sur {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("archive trop courte ({len} octets) pour contenir en-tête et trailer")]
    TooShort { len: u64 },

    #[error("en-tête invalide : {0}")]
    BadHeader(&'static str),

    #[error("version de format {found} non prise en charge (maximum {max_supported})")]
    UnsupportedVersion { found: u16, max_supported: u16 },

    #[error("aucun trailer valide trouvé (les deux copies sont corrompues)")]
    NoValidTrailer,

    #[error("section {section} hors des bornes de l'archive (offset {offset}, longueur {len}, taille {file_len})")]
    SectionOutOfBounds {
        section: &'static str,
        offset: u64,
        len: u64,
        file_len: u64,
    },

    #[error("section {section} mal formée : {detail}")]
    MalformedSection {
        section: &'static str,
        detail: String,
    },

    #[error("chemin absent de l'archive : {0}")]
    NotFound(String),

    #[error("morceau {index} corrompu : hachage attendu {expected}, obtenu {actual}")]
    ChunkMismatch {
        index: u32,
        expected: String,
        actual: String,
    },

    #[error("décompression du morceau {index} impossible : {source}")]
    Decompress {
        index: u32,
        #[source]
        source: std::io::Error,
    },

    #[error("compression impossible : {0}")]
    Compress(#[source] std::io::Error),

    #[error("chemin non représentable en UTF-8 : {0}")]
    NonUtf8Path(PathBuf),

    #[error("chemin trop long ({len} octets, maximum {max}) : {path}")]
    PathTooLong {
        path: String,
        len: usize,
        max: usize,
    },

    #[error("paramètres de découpage invalides : {0}")]
    BadChunkParams(&'static str),

    #[error("paramètres de parité invalides : {0}")]
    BadParityParams(&'static str),

    #[error("archive sans section de parité : corruption détectée mais non réparable")]
    NoParity,

    #[error("corruption non réparable : groupe {group} avec {erased} shards effacés pour {max} de parité")]
    Unrepairable { group: u64, erased: u64, max: u32 },

    #[error("calcul Reed-Solomon impossible : {0}")]
    ReedSolomon(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
