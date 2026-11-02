//! Préférences propres à l'application (apparence, sons, Dock, barre de menus).
//! Les réglages du moteur restent dans `grab_core::Settings`.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppPrefs {
    /// "dark", "light" ou "system".
    pub theme: String,
    /// "comfortable" ou "compact".
    pub density: String,
    pub show_thumbnails: bool,
    pub hide_dock: bool,
    /// Lance l'application à l'ouverture de session, sans fenêtre.
    pub launch_at_login: bool,
    pub menubar_icon: bool,
    pub menubar_speed: bool,
    pub sounds: bool,
    pub sound_added: String,
    pub sound_finished: String,
    pub sound_failed: String,
    pub sound_queue_finished: String,
    pub focus_on_question: bool,
    pub search_in_main: bool,
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            density: "comfortable".into(),
            show_thumbnails: true,
            hide_dock: false,
            launch_at_login: false,
            menubar_icon: true,
            menubar_speed: true,
            sounds: true,
            sound_added: String::new(),
            sound_finished: "Glass".into(),
            sound_failed: "Basso".into(),
            sound_queue_finished: "Hero".into(),
            focus_on_question: true,
            search_in_main: false,
        }
    }
}

impl AppPrefs {
    fn file(dir: &Path) -> PathBuf {
        dir.join("app-prefs.json")
    }
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(Self::file(dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::write(Self::file(dir), serde_json::to_string_pretty(self).unwrap_or_default())
    }
    /// Sans icône dans le Dock ni dans la barre de menus, l'application serait inaccessible.
    pub fn sanitize(&mut self) {
        if !matches!(self.theme.as_str(), "dark" | "light" | "system") {
            self.theme = "dark".into();
        }
        if !matches!(self.density.as_str(), "comfortable" | "compact") {
            self.density = "comfortable".into();
        }
        if self.hide_dock {
            self.menubar_icon = true;
        }
    }
}

/// Liste des sons système disponibles (noms sans extension).
pub fn system_sounds() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir("/System/Library/Sounds")
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".aiff")).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Joue un son système avec `afplay`. Le nom doit figurer dans la liste (pas de chemin libre).
pub fn play_sound(name: &str) {
    if name.is_empty() || !system_sounds().iter().any(|s| s == name) {
        return;
    }
    let _ = std::process::Command::new("/usr/bin/afplay")
        .arg(format!("/System/Library/Sounds/{name}.aiff"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
