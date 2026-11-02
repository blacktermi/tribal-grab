//! Moteur de Tribal Grab : extraction, téléchargement, file d'attente, historique.
//! Sans dépendance à Tauri : l'application pilote tout par la façade `Engine`.
pub mod db;
pub mod download;
pub mod engine;
pub mod error;
pub mod extract;
pub mod history_io;
pub mod integrations;
pub mod model;
pub mod naming;
pub mod net;
pub mod post;
pub mod quality;
mod runner;
pub mod search;
pub mod secrets;
pub mod subs;
pub mod tools;
pub mod update;
pub mod util;
pub mod xml;

pub use engine::Engine;
pub use error::{Error, Result};
pub use model::*;
