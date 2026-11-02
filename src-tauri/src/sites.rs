//! Connexion aux sites : cookies de la session intégrée exportés au format Netscape
//! (lus par le moteur et par yt-dlp), et gestion des données par site.
use crate::commands::other;
use crate::state::AppState;
use grab_core::Error;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::webview::cookie::Cookie;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

type R<T> = std::result::Result<T, Error>;

pub fn cookies_path(data_dir: &Path) -> PathBuf {
    data_dir.join("cookies.txt")
}

fn netscape_line(c: &Cookie<'_>) -> Option<String> {
    let domain = c.domain()?.to_string();
    if domain.is_empty() || c.name().is_empty() {
        return None;
    }
    let sub = if domain.starts_with('.') { "TRUE" } else { "FALSE" };
    let expiry = match c.expires() {
        Some(tauri::webview::cookie::Expiration::DateTime(d)) => d.unix_timestamp().max(0),
        _ => 0,
    };
    let prefix = if c.http_only().unwrap_or(false) { "#HttpOnly_" } else { "" };
    let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    Some(format!(
        "{prefix}{domain}\t{sub}\t{}\t{}\t{expiry}\t{}\t{}",
        c.path().unwrap_or("/"),
        if c.secure().unwrap_or(false) { "TRUE" } else { "FALSE" },
        clean(c.name()),
        clean(c.value())
    ))
}

/// Écrit le fichier de cookies (droits 600) et le déclare dans les réglages du moteur.
pub fn export_cookies(app: &AppHandle) -> R<usize> {
    let st = app.state::<AppState>();
    let w = app.get_webview_window("main").ok_or_else(|| other("fenêtre principale introuvable"))?;
    let cookies = w.cookies().map_err(other)?;
    let mut out = String::from("# Netscape HTTP Cookie File\n# Généré par Tribal Grab. Ce fichier contient des identifiants de session : ne le partagez pas.\n");
    let mut n = 0;
    for c in &cookies {
        if let Some(l) = netscape_line(c) {
            out.push_str(&l);
            out.push('\n');
            n += 1;
        }
    }
    let path = cookies_path(&st.data_dir);
    std::fs::write(&path, out).map_err(|e| Error::Io(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    let mut s = st.engine.settings();
    let p = path.to_string_lossy().to_string();
    if s.cookies_file.as_deref() != Some(p.as_str()) {
        s.cookies_file = Some(p);
        st.engine.set_settings(s)?;
        let _ = app.emit("settings://changed", &st.engine.settings());
    }
    Ok(n)
}

#[derive(Serialize)]
pub struct LoginStatus {
    pub path: String,
    pub active: bool,
    pub cookies: usize,
    pub updated_at: Option<u64>,
}

#[tauri::command]
pub fn login_status(st: State<'_, AppState>) -> LoginStatus {
    let path = cookies_path(&st.data_dir);
    let p = path.to_string_lossy().to_string();
    let txt = std::fs::read_to_string(&path).unwrap_or_default();
    let cookies = txt.lines().filter(|l| !l.trim().is_empty() && (!l.starts_with('#') || l.starts_with("#HttpOnly_"))).count();
    let updated_at = std::fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
    LoginStatus { active: st.engine.settings().cookies_file.as_deref() == Some(p.as_str()), path: p, cookies, updated_at }
}

/// Enregistre les cookies de la session intégrée pour le moteur et yt-dlp.
#[tauri::command]
pub async fn login_save(app: AppHandle) -> R<usize> {
    export_cookies(&app)
}

/// Cesse d'utiliser le fichier de cookies de connexion (le fichier reste sur le disque).
#[tauri::command]
pub fn login_disable(app: AppHandle, st: State<'_, AppState>) -> R<()> {
    let mut s = st.engine.settings();
    if s.cookies_file.as_deref() == Some(cookies_path(&st.data_dir).to_string_lossy().as_ref()) {
        s.cookies_file = None;
        st.engine.set_settings(s)?;
        let _ = app.emit("settings://changed", &st.engine.settings());
    }
    Ok(())
}

#[derive(Serialize)]
pub struct SiteData {
    pub domain: String,
    pub cookies: usize,
}

#[tauri::command]
pub async fn site_data_list(app: AppHandle) -> R<Vec<SiteData>> {
    let w = app.get_webview_window("main").ok_or_else(|| other("fenêtre principale introuvable"))?;
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for c in w.cookies().map_err(other)? {
        if let Some(d) = c.domain() {
            *by.entry(d.trim_start_matches('.').to_string()).or_default() += 1;
        }
    }
    Ok(by.into_iter().map(|(domain, cookies)| SiteData { domain, cookies }).collect())
}

fn valid_domain(d: &str) -> bool {
    !d.is_empty() && d.len() < 254 && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') && !d.starts_with('.')
}

/// Efface les données d'un site. `kinds` : "cookies", "local" (stockage local), "session" (stockage de session).
#[tauri::command]
pub async fn site_data_clear(app: AppHandle, domain: String, kinds: Vec<String>) -> R<()> {
    if !valid_domain(&domain) {
        return Err(other("domaine invalide"));
    }
    let w = app.get_webview_window("main").ok_or_else(|| other("fenêtre principale introuvable"))?;
    if kinds.iter().any(|k| k == "cookies") {
        for c in w.cookies().map_err(other)? {
            if let Some(d) = c.domain() {
                let d = d.trim_start_matches('.');
                if d == domain || d.ends_with(&format!(".{domain}")) {
                    let _ = w.delete_cookie(c.clone());
                }
            }
        }
        export_cookies(&app).ok();
    }
    let local = kinds.iter().any(|k| k == "local");
    let session = kinds.iter().any(|k| k == "session");
    if local || session {
        clear_storage(&app, &domain, local, session)?;
    }
    Ok(())
}

/// Ouvre une fenêtre cachée sans aucune permission sur l'origine du site, vide son stockage, puis la ferme.
fn clear_storage(app: &AppHandle, domain: &str, local: bool, session: bool) -> R<()> {
    let url: url::Url = format!("https://{domain}/").parse().map_err(other)?;
    let label = format!("sitectl-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
    let js = format!(
        "try{{{}}}catch(e){{}}",
        [if local { "localStorage.clear();" } else { "" }, if session { "sessionStorage.clear();" } else { "" }].concat()
    );
    let label2 = label.clone();
    let app2 = app.clone();
    WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .visible(false)
        .on_page_load(move |wv, p| {
            if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
                let _ = wv.eval(&js);
                let (a, l) = (app2.clone(), label2.clone());
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                    if let Some(w) = a.get_webview_window(&l) {
                        let _ = w.destroy();
                    }
                });
            }
        })
        .build()
        .map_err(other)?;
    // Garde-fou : la fenêtre cachée est détruite même si la page ne se charge jamais.
    let (a, l) = (app.clone(), label);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        if let Some(w) = a.get_webview_window(&l) {
            let _ = w.destroy();
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn site_data_clear_all(app: AppHandle, st: State<'_, AppState>) -> R<()> {
    let w = app.get_webview_window("main").ok_or_else(|| other("fenêtre principale introuvable"))?;
    w.clear_all_browsing_data().map_err(other)?;
    let _ = std::fs::remove_file(cookies_path(&st.data_dir));
    let mut s = st.engine.settings();
    if s.cookies_file.as_deref() == Some(cookies_path(&st.data_dir).to_string_lossy().as_ref()) {
        s.cookies_file = None;
        st.engine.set_settings(s)?;
        let _ = app.emit("settings://changed", &st.engine.settings());
    }
    Ok(())
}
