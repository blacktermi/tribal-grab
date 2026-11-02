//! Déclare les commandes de l'application (chaque commande exige une permission explicite)
//! et génère la capacité des fenêtres locales à partir des fonctions `#[tauri::command]`.
//! Les pages distantes (extraction guidée, connexion) ne reçoivent que `guided_report`,
//! voir `capabilities/guided-remote.json`.
use std::fs;

const SOURCES: &[&str] = &["src/commands.rs", "src/guided.rs", "src/sites.rs"];
/// Seule commande accessible aux pages distantes.
const REMOTE_ONLY: &str = "guided_report";

fn commands() -> Vec<String> {
    let mut out = Vec::new();
    for f in SOURCES {
        println!("cargo:rerun-if-changed={f}");
        let Ok(src) = fs::read_to_string(f) else { continue };
        let mut lines = src.lines();
        while let Some(l) = lines.next() {
            if l.trim() != "#[tauri::command]" {
                continue;
            }
            for l2 in lines.by_ref() {
                let t = l2.trim();
                if let Some(rest) = t.strip_prefix("pub async fn ").or_else(|| t.strip_prefix("pub fn ")) {
                    out.push(rest.split(['(', '<']).next().unwrap().trim().to_string());
                    break;
                }
            }
        }
    }
    out.sort();
    out
}

fn main() {
    let cmds = commands();
    let local: Vec<String> = cmds
        .iter()
        .filter(|c| c.as_str() != REMOTE_ONLY)
        .map(|c| format!("\"allow-{}\"", c.replace('_', "-")))
        .collect();
    let cap = format!(
        r#"{{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "local-windows",
  "description": "Fenêtres locales de l'application (fichier généré par build.rs).",
  "local": true,
  "windows": ["main", "prefs", "library", "tray"],
  "webviews": ["browser-ui"],
  "permissions": [
    "core:default",
    "core:window:allow-start-dragging",
    "core:window:allow-set-focus",
    "core:window:allow-show",
    "core:window:allow-hide",
    "core:window:allow-close",
    "core:window:allow-minimize",
    "core:window:allow-toggle-maximize",
    "dialog:allow-open",
    "dialog:allow-save",
    "dialog:allow-message",
    "dialog:allow-ask",
    "dialog:allow-confirm",
    {}
  ]
}}
"#,
        local.join(",\n    ")
    );
    let _ = fs::create_dir_all("capabilities");
    if fs::read_to_string("capabilities/local-windows.json").ok().as_deref() != Some(cap.as_str()) {
        fs::write("capabilities/local-windows.json", cap).unwrap();
    }
    let all: &'static [&'static str] = Box::leak(cmds.iter().map(|c| &*Box::leak(c.clone().into_boxed_str())).collect::<Vec<&'static str>>().into_boxed_slice());
    let attrs = tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(all));
    tauri_build::try_build(attrs).expect("échec de tauri-build");
}
