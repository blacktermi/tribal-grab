use serde::{Serialize, Serializer};

/// Erreurs du moteur. Les messages s'adressent à l'utilisateur, en français.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Adresse invalide : {0}")]
    InvalidUrl(String),
    #[error("Erreur réseau : {0}")]
    Network(String),
    #[error("Erreur de fichier : {0}")]
    Io(String),
    #[error("Erreur de base de données : {0}")]
    Db(String),
    #[error("Outil introuvable : {0}. Installez-le ou indiquez son chemin dans les réglages.")]
    ToolMissing(String),
    #[error("Extraction impossible : {0}")]
    Extract(String),
    #[error("Ce contenu est protégé par un DRM (Widevine, FairPlay ou PlayReady). Tribal Grab ne le télécharge pas.")]
    Drm,
    #[error("Lien non pris en charge : {0}")]
    Unsupported(String),
    #[error("Espace disque insuffisant : {0}")]
    DiskFull(String),
    #[error("Tâche introuvable : {0}")]
    NoTask(i64),
    #[error("Opération annulée")]
    Cancelled,
    #[error("Téléchargement mis en pause")]
    Paused,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        if e.raw_os_error() == Some(28) {
            return Error::DiskFull("le disque est plein".into());
        }
        Error::Io(e.to_string())
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e.to_string())
    }
}
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        // Le message de reqwest peut contenir l'URL complète : on la retire.
        Error::Network(crate::util::redact(&e.without_url().to_string()))
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Other(format!("JSON invalide : {e}"))
    }
}
impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Self {
        Error::InvalidUrl(e.to_string())
    }
}
