//! Ouverture depuis l'extérieur : schéma `tribalgrab://`, fichiers `.tgi` et `.tgpart`, arguments de lancement.
use crate::commands::other;
use crate::state::AppState;
use grab_core::{Error, IntegrationInfo};
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

type R<T> = std::result::Result<T, Error>;

/// Lit un `.tgi` pour confirmation. Retourne `{ json, info }` ; rien n'est installé.
pub fn read_tgi(path: &Path) -> R<serde_json::Value> {
    let meta = std::fs::metadata(path).map_err(|e| Error::Io(e.to_string()))?;
    if !meta.is_file() || meta.len() > 1024 * 1024 {
        return Err(other("fichier d'intégration introuvable ou trop volumineux (1 Mo maximum)"));
    }
    let json = std::fs::read_to_string(path).map_err(|e| Error::Io(e.to_string()))?;
    let info: IntegrationInfo = serde_json::from_str(&json).map_err(|e| other(format!("fichier .tgi invalide : {e}")))?;
    if info.name.trim().is_empty() || info.url_regex.trim().is_empty() {
        return Err(other("fichier .tgi invalide : nom et expression régulière d'adresse obligatoires"));
    }
    Ok(serde_json::json!({ "json": json, "info": info, "path": path.to_string_lossy() }))
}

/// `tribalgrab://add?url=…&referer=…` : seule action permise, et seulement avec des adresses http ou https.
pub fn handle_scheme(app: &AppHandle, raw: &str) {
    let Ok(u) = Url::parse(raw) else { return };
    if u.scheme() != "tribalgrab" {
        return;
    }
    let action = u.host_str().unwrap_or("").to_string();
    let action = if action.is_empty() { u.path().trim_matches('/').to_string() } else { action };
    if action != "add" {
        tracing::warn!("action du schéma tribalgrab ignorée : {action}");
        return;
    }
    let Some(target) = u.query_pairs().find(|(k, _)| k == "url").map(|(_, v)| v.into_owned()) else { return };
    let ok = Url::parse(&target).is_ok_and(|t| matches!(t.scheme(), "http" | "https") && t.host_str().is_some());
    if !ok || target.len() > 8192 {
        tracing::warn!("tribalgrab://add refusé : adresse non http(s)");
        return;
    }
    let referer = u.query_pairs().find(|(k, _)| k == "referer").map(|(_, v)| v.into_owned()).filter(|r| r.len() <= 8192 && Url::parse(r).is_ok_and(|t| matches!(t.scheme(), "http" | "https")));
    let st = app.state::<AppState>();
    let mut req = grab_core::AddRequest { url: target, ..Default::default() };
    req.options.referer = referer;
    match st.engine.add(req) {
        Ok(id) => tracing::info!("ajouté depuis le schéma tribalgrab : tâche {id}"),
        Err(e) => tracing::warn!("ajout refusé : {e}"),
    }
}

pub fn handle_path(app: &AppHandle, path: &Path) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "tgi" => match read_tgi(path) {
            Ok(v) => {
                crate::windows::show_main(app);
                // Rien n'est installé : l'interface affiche le code et demande confirmation.
                let _ = app.emit_to("main", "tgi://confirm", v);
            }
            Err(e) => {
                let _ = app.emit_to("main", "app://error", e.to_string());
            }
        },
        "tgpart" => {
            let st = app.state::<AppState>();
            let target = path.to_string_lossy().trim_end_matches('/').trim_end_matches(".tgpart").to_string();
            let found = st.engine.tasks(grab_core::SortKey::Order, false).into_iter().find(|t| t.target.as_deref() == Some(target.as_str()));
            match found {
                Some(t) => {
                    let _ = st.engine.resume(t.id);
                    crate::windows::show_main(app);
                }
                None => {
                    let _ = app.emit_to("main", "app://error", "Aucune tâche de la file ne correspond à ce téléchargement partiel.".to_string());
                    crate::windows::show_main(app);
                }
            }
        }
        _ => {}
    }
}

pub fn handle_args(app: &AppHandle, args: &[String]) {
    for a in args.iter().skip(1) {
        if a.starts_with("tribalgrab:") {
            handle_scheme(app, a);
        } else if let Some(p) = a.strip_prefix("file://") {
            handle_path(app, Path::new(p));
        } else if Path::new(a).exists() {
            handle_path(app, Path::new(a));
        }
    }
}

pub fn handle_opened(app: &AppHandle, urls: &[Url]) {
    for u in urls {
        match u.scheme() {
            "tribalgrab" => handle_scheme(app, u.as_str()),
            "file" => {
                if let Ok(p) = u.to_file_path() {
                    handle_path(app, &p);
                }
            }
            _ => {}
        }
    }
}
