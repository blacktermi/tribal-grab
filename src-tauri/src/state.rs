use crate::prefs::AppPrefs;
use grab_core::Engine;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

pub struct AppState {
    pub engine: Engine,
    pub data_dir: PathBuf,
    pub prefs: Mutex<AppPrefs>,
    /// Processus `qlmanage -p` en cours (un seul aperçu à la fois).
    pub quicklook: Mutex<Option<std::process::Child>>,
    pub popover_hidden_at: Mutex<Option<Instant>>,
}
