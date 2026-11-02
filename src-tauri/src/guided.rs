//! Extraction guidée et connexion aux sites : une fenêtre avec deux webviews.
//! - `browser-ui` : l'interface locale (barre d'adresse, barre latérale), de confiance ;
//! - `browser-page` : la page web, NON fiable. Elle n'a accès qu'à `guided_report`
//!   (voir `capabilities/guided-remote.json`).
use crate::commands::other;
use crate::state::AppState;
use grab_core::{AddRequest, Error, TaskOptions};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Rect, State, TitleBarStyle, WebviewUrl, WindowEvent};
use url::Url;

type R<T> = std::result::Result<T, Error>;

const PAGE: &str = "browser-page";
const UI: &str = "browser-ui";
const INIT_JS: &str = include_str!("guided-init.js");
const MAX_MEDIA: usize = 4000;
const MAX_HISTORY: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct Detected {
    pub id: u64,
    /// "video", "audio", "hls", "dash", "image"
    pub kind: String,
    pub url: String,
    pub mime: Option<String>,
    pub page: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub at: i64,
}

#[derive(Default, Serialize, Deserialize)]
struct Persist {
    bookmarks: Vec<Bookmark>,
    history: Vec<HistoryEntry>,
}

struct Inner {
    media: Vec<Detected>,
    seen: HashSet<String>,
    next_id: u64,
    tokens: f64,
    refilled: Instant,
    page_url: String,
    page_title: String,
    muted: bool,
    layout: (f64, f64, f64, f64),
    mode: String,
    persist: Persist,
    file: PathBuf,
}

pub struct Guided(Mutex<Inner>);

impl Guided {
    pub fn new(data_dir: &std::path::Path) -> Self {
        let file = data_dir.join("guided.json");
        let persist = std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Guided(Mutex::new(Inner {
            media: vec![],
            seen: HashSet::new(),
            next_id: 1,
            tokens: 80.0,
            refilled: Instant::now(),
            page_url: String::new(),
            page_title: String::new(),
            muted: false,
            layout: (0.0, 96.0, 800.0, 500.0),
            mode: "guided".into(),
            persist,
            file,
        }))
    }
}

impl Inner {
    fn save(&self) {
        let _ = std::fs::write(&self.file, serde_json::to_string_pretty(&self.persist).unwrap_or_default());
    }
}

// --- Validation de ce que la page distante peut dire ---

/// Rapport d'une page non fiable. Tout est validé avant usage.
#[derive(Debug, Deserialize)]
pub struct Report {
    kind: String,
    url: String,
    mime: Option<String>,
    title: Option<String>,
    page: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    size: Option<u64>,
}

fn http_url(s: &str, max: usize) -> Option<Url> {
    if s.len() > max {
        return None;
    }
    let u = Url::parse(s).ok()?;
    matches!(u.scheme(), "http" | "https").then_some(u).filter(|u| u.host_str().is_some())
}

fn clean_text(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

fn ext_of(u: &Url) -> String {
    u.path().rsplit('.').next().filter(|e| e.len() <= 5 && !u.path().ends_with('/')).unwrap_or("").to_ascii_lowercase()
}

/// Décide si une ressource vaut la peine d'être proposée, et de quel type.
fn classify(kind: &str, url: &Url, mime: Option<&str>) -> Option<&'static str> {
    let ext = ext_of(url);
    let mime = mime.map(|m| m.split(';').next().unwrap_or("").trim().to_ascii_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "ts" | "m4s" | "m4f" | "cmfv" | "cmfa" | "aac" | "vtt" | "srt" | "js" | "css" | "json" | "html" | "woff" | "woff2") && !mime.contains("mpegurl") {
        return None;
    }
    if mime.contains("mpegurl") || ext == "m3u8" {
        return Some("hls");
    }
    if mime.contains("dash+xml") || ext == "mpd" {
        return Some("dash");
    }
    if mime.starts_with("image/") || (kind == "image" && !matches!(ext.as_str(), "svg" | "ico")) {
        return if kind == "image" || matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif" | "avif") { Some("image") } else { None };
    }
    if mime.starts_with("video/") && mime != "video/mp2t" || matches!(ext.as_str(), "mp4" | "webm" | "mov" | "m4v" | "mkv" | "flv" | "ogv" | "avi") {
        return Some("video");
    }
    if mime.starts_with("audio/") || matches!(ext.as_str(), "mp3" | "m4a" | "ogg" | "opus" | "flac" | "wav" | "oga") {
        return Some("audio");
    }
    if matches!(kind, "video" | "source") && !url.path().is_empty() {
        return Some("video");
    }
    if kind == "audio" {
        return Some("audio");
    }
    None
}

/// Seule commande exécutable par les pages distantes. Elle ne fait que valider et ranger ce qu'elle reçoit.
#[tauri::command]
pub fn guided_report(webview: tauri::Webview, app: AppHandle, guided: State<'_, Guided>, report: Report) -> R<()> {
    if webview.label() != PAGE {
        return Err(other("source non autorisée"));
    }
    let mut g = guided.0.lock().unwrap();
    // Débit limité : 40 rapports par seconde en régime établi, rafale de 80.
    let now = Instant::now();
    g.tokens = (g.tokens + now.duration_since(g.refilled).as_secs_f64() * 40.0).min(80.0);
    g.refilled = now;
    if g.tokens < 1.0 {
        return Ok(());
    }
    g.tokens -= 1.0;

    let Some(url) = http_url(&report.url, 4096) else { return Ok(()) };
    if report.kind == "page" {
        // L'adresse affichée est celle que le navigateur annonce, pas celle que la page prétend avoir.
        let real = webview.url().ok().map(|u| u.to_string()).unwrap_or_else(|| url.to_string());
        g.page_url = real.clone();
        g.page_title = report.title.as_deref().map(|t| clean_text(t, 300)).unwrap_or_default();
        let payload = serde_json::json!({ "url": real, "title": g.page_title });
        drop(g);
        let _ = app.emit_to(UI, "guided://page", payload);
        return Ok(());
    }
    if !matches!(report.kind.as_str(), "fetch" | "xhr" | "video" | "audio" | "source" | "image" | "resource") {
        return Ok(());
    }
    let mime = report.mime.as_deref().map(|m| clean_text(m, 120)).filter(|m| !m.is_empty());
    let Some(kind) = classify(&report.kind, &url, mime.as_deref()) else { return Ok(()) };
    let mut key = url.clone();
    key.set_fragment(None);
    let key = format!("{kind}|{key}");
    if g.media.len() >= MAX_MEDIA || !g.seen.insert(key) {
        return Ok(());
    }
    let id = g.next_id;
    g.next_id += 1;
    let item = Detected {
        id,
        kind: kind.to_string(),
        url: url.to_string(),
        mime,
        page: report.page.as_deref().and_then(|p| http_url(p, 4096)).map(|u| u.to_string()),
        width: report.width.filter(|w| *w < 100_000),
        height: report.height.filter(|h| *h < 100_000),
        size: report.size,
    };
    g.media.push(item.clone());
    drop(g);
    let _ = app.emit_to(UI, "guided://media", item);
    Ok(())
}

// --- Fenêtre ---

fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Transforme ce que l'utilisateur a tapé en adresse : lien, nom de domaine, ou recherche.
pub fn normalize_input(input: &str) -> R<Url> {
    let s = input.trim();
    if s.is_empty() {
        return Err(other("adresse vide"));
    }
    if let Some(u) = http_url(s, 4096) {
        return Ok(u);
    }
    if !s.contains(char::is_whitespace)
        && s.contains('.')
        && !s.contains("://")
        && let Some(u) = http_url(&format!("https://{s}"), 4096)
    {
        return Ok(u);
    }
    if s.contains("://") {
        return Err(other("seules les adresses http et https sont acceptées"));
    }
    let q: String = url::form_urlencoded::byte_serialize(s.as_bytes()).collect();
    Url::parse(&format!("https://duckduckgo.com/?q={q}")).map_err(other)
}

pub fn open_browser(app: &AppHandle, url: Option<&str>, mode: &str) -> R<()> {
    let mode = if mode == "login" { "login" } else { "guided" };
    let guided = app.state::<Guided>();
    guided.0.lock().unwrap().mode = mode.to_string();
    let start = match url {
        Some(u) if !u.is_empty() => Some(normalize_input(u)?),
        _ => None,
    };
    if let Some(win) = app.get_window("browser") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        let _ = app.emit_to(UI, "guided://mode", mode);
        if let Some(u) = start {
            navigate(app, u)?;
        }
        return Ok(());
    }
    let win = WindowBuilder::new(app, "browser")
        .title(if mode == "login" { "Connexion aux sites" } else { "Extraction guidée" })
        .inner_size(1220.0, 780.0)
        .min_inner_size(860.0, 520.0)
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()
        .map_err(other)?;
    let scale = win.scale_factor().map_err(other)?;
    let size: LogicalSize<f64> = win.inner_size().map_err(other)?.to_logical(scale);
    let ui_url = format!("index.html?mode={mode}");
    win.add_child(WebviewBuilder::new(UI, WebviewUrl::App(ui_url.into())), LogicalPosition::new(0.0, 0.0), size).map_err(other)?;

    // Suit la taille de la fenêtre : l'interface occupe tout, la page est replacée par `guided_layout`.
    let app2 = app.clone();
    win.on_window_event(move |ev| match ev {
        WindowEvent::Resized(_) => {
            if let Some(w) = app2.get_window("browser")
                && let (Ok(sz), Ok(sc)) = (w.inner_size(), w.scale_factor())
                && let Some(ui) = app2.get_webview(UI)
            {
                let l: LogicalSize<f64> = sz.to_logical(sc);
                let _ = ui.set_bounds(Rect { position: LogicalPosition::new(0.0, 0.0).into(), size: l.into() });
            }
        }
        WindowEvent::Destroyed => {
            let g = app2.state::<Guided>();
            let mut gi = g.0.lock().unwrap();
            let login = gi.mode == "login";
            gi.media.clear();
            gi.seen.clear();
            drop(gi);
            if login {
                let _ = crate::sites::export_cookies(&app2);
            }
        }
        _ => {}
    });
    if let Some(u) = start {
        navigate(app, u)?;
    }
    Ok(())
}

fn page_bounds(app: &AppHandle) -> Rect {
    let (x, y, w, h) = app.state::<Guided>().0.lock().unwrap().layout;
    Rect { position: LogicalPosition::new(x, y).into(), size: LogicalSize::new(w.max(1.0), h.max(1.0)).into() }
}

fn navigate(app: &AppHandle, url: Url) -> R<()> {
    if let Some(wv) = app.get_webview(PAGE) {
        return wv.navigate(url).map_err(other);
    }
    let win = app.get_window("browser").ok_or_else(|| other("fenêtre du navigateur absente"))?;
    let app_nav = app.clone();
    let app_load = app.clone();
    let app_title = app.clone();
    let app_new = app.clone();
    let b = page_bounds(app);
    let builder = WebviewBuilder::new(PAGE, WebviewUrl::External(url))
        .initialization_script_for_all_frames(INIT_JS)
        .on_navigation(move |u| {
            let ok = matches!(u.scheme(), "http" | "https" | "about" | "blob" | "data");
            if ok && matches!(u.scheme(), "http" | "https") {
                let _ = app_nav.emit_to(UI, "guided://loading", u.to_string());
            }
            ok
        })
        .on_new_window(move |u, _| {
            if matches!(u.scheme(), "http" | "https")
                && let Some(wv) = app_new.get_webview(PAGE)
            {
                let _ = wv.navigate(u);
            }
            NewWindowResponse::Deny
        })
        .on_document_title_changed(move |wv, title| {
            let t = clean_text(&title, 300);
            if let Ok(u) = wv.url() {
                let _ = app_title.emit_to(UI, "guided://page", serde_json::json!({ "url": u.to_string(), "title": t }));
            }
        })
        .on_page_load(move |wv, p| {
            let u = p.url().to_string();
            match p.event() {
                PageLoadEvent::Started => {
                    let g = app_load.state::<Guided>();
                    let mut gi = g.0.lock().unwrap();
                    gi.media.clear();
                    gi.seen.clear();
                    gi.page_url = u.clone();
                    gi.page_title.clear();
                    drop(gi);
                    let _ = app_load.emit_to(UI, "guided://reset", u);
                }
                PageLoadEvent::Finished => {
                    let g = app_load.state::<Guided>();
                    let mut gi = g.0.lock().unwrap();
                    if gi.muted {
                        let _ = wv.eval("window.__tgSetMuted && window.__tgSetMuted(true)");
                    }
                    if matches!(p.url().scheme(), "http" | "https") && gi.persist.history.last().map(|h| h.url.as_str()) != Some(u.as_str()) {
                        let at = unix_now();
                        let title = gi.page_title.clone();
                        gi.persist.history.push(HistoryEntry { url: u.clone(), title, at });
                        let n = gi.persist.history.len();
                        if n > MAX_HISTORY {
                            gi.persist.history.drain(..n - MAX_HISTORY);
                        }
                        gi.save();
                    }
                    drop(gi);
                    let _ = app_load.emit_to(UI, "guided://loaded", u);
                }
            }
        });
    win.add_child(builder, b.position, b.size).map_err(other)?;
    Ok(())
}

// --- Commandes de l'interface locale ---

#[derive(Serialize)]
pub struct BrowserState {
    media: Vec<Detected>,
    page_url: String,
    page_title: String,
    muted: bool,
    mode: String,
    bookmarks: Vec<Bookmark>,
    history: Vec<HistoryEntry>,
    has_page: bool,
}

#[tauri::command]
pub fn browser_state(app: AppHandle, guided: State<'_, Guided>) -> BrowserState {
    let g = guided.0.lock().unwrap();
    BrowserState {
        media: g.media.clone(),
        page_url: g.page_url.clone(),
        page_title: g.page_title.clone(),
        muted: g.muted,
        mode: g.mode.clone(),
        bookmarks: g.persist.bookmarks.clone(),
        history: g.persist.history.iter().rev().take(200).cloned().collect(),
        has_page: app.get_webview(PAGE).is_some(),
    }
}

#[tauri::command]
pub async fn guided_navigate(app: AppHandle, input: String) -> R<String> {
    let u = normalize_input(&input)?;
    let s = u.to_string();
    navigate(&app, u)?;
    Ok(s)
}

#[tauri::command]
pub fn guided_nav(app: AppHandle, action: String) -> R<()> {
    let Some(wv) = app.get_webview(PAGE) else { return Ok(()) };
    let js = match action.as_str() {
        "back" => "history.back()",
        "forward" => "history.forward()",
        "reload" => "location.reload()",
        "stop" => "window.stop()",
        _ => return Err(other("action inconnue")),
    };
    wv.eval(js).map_err(other)
}

#[tauri::command]
pub fn guided_mute(app: AppHandle, guided: State<'_, Guided>, muted: bool) {
    guided.0.lock().unwrap().muted = muted;
    if let Some(wv) = app.get_webview(PAGE) {
        let _ = wv.eval(format!("window.__tgSetMuted && window.__tgSetMuted({muted})"));
    }
}

/// Position de la zone de page (pixels logiques, relatifs à la fenêtre), calculée par l'interface.
#[tauri::command]
pub fn guided_layout(app: AppHandle, guided: State<'_, Guided>, x: f64, y: f64, w: f64, h: f64) {
    if ![x, y, w, h].iter().all(|v| v.is_finite()) {
        return;
    }
    guided.0.lock().unwrap().layout = (x.max(0.0), y.max(0.0), w.clamp(1.0, 20000.0), h.clamp(1.0, 20000.0));
    if let Some(wv) = app.get_webview(PAGE) {
        let _ = wv.set_bounds(page_bounds(&app));
    }
}

/// Masque la page quand une fenêtre de dialogue de l'interface doit passer devant (une vue native recouvre le HTML).
#[tauri::command]
pub fn guided_page_visible(app: AppHandle, visible: bool) {
    if let Some(wv) = app.get_webview(PAGE) {
        let _ = if visible { wv.show() } else { wv.hide() };
    }
}

#[tauri::command]
pub fn guided_send(st: State<'_, AppState>, urls: Vec<String>, referer: Option<String>) -> R<Vec<i64>> {
    let referer = referer.as_deref().and_then(|r| http_url(r, 4096)).map(|u| u.to_string());
    let mut ids = vec![];
    for u in urls.iter().take(200) {
        let Some(u) = http_url(u, 4096) else { continue };
        ids.push(st.engine.add(AddRequest { url: u.to_string(), options: TaskOptions { referer: referer.clone(), ..Default::default() }, ..Default::default() })?);
    }
    if ids.is_empty() {
        return Err(other("aucune adresse valide à envoyer"));
    }
    Ok(ids)
}

#[tauri::command]
pub fn guided_clear_media(guided: State<'_, Guided>) {
    let mut g = guided.0.lock().unwrap();
    g.media.clear();
    g.seen.clear();
}

#[tauri::command]
pub fn bookmark_toggle(guided: State<'_, Guided>, url: String, title: String) -> R<bool> {
    let u = http_url(&url, 4096).ok_or_else(|| other("adresse invalide"))?.to_string();
    let mut g = guided.0.lock().unwrap();
    let added = if let Some(i) = g.persist.bookmarks.iter().position(|b| b.url == u) {
        g.persist.bookmarks.remove(i);
        false
    } else {
        g.persist.bookmarks.push(Bookmark { url: u, title: clean_text(&title, 300) });
        true
    };
    g.save();
    Ok(added)
}

#[tauri::command]
pub fn guided_history_clear(guided: State<'_, Guided>) {
    let mut g = guided.0.lock().unwrap();
    g.persist.history.clear();
    g.save();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_urls_are_accepted() {
        assert!(http_url("https://a.example/x.mp4", 4096).is_some());
        assert!(http_url("javascript:alert(1)", 4096).is_none());
        assert!(http_url("file:///etc/passwd", 4096).is_none());
        assert!(http_url("data:text/html,x", 4096).is_none());
        assert!(http_url(&format!("https://a.example/{}", "x".repeat(5000)), 4096).is_none());
    }

    #[test]
    fn classification() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert_eq!(classify("fetch", &u("https://a.example/master.m3u8"), None), Some("hls"));
        assert_eq!(classify("xhr", &u("https://a.example/a"), Some("application/dash+xml")), Some("dash"));
        assert_eq!(classify("fetch", &u("https://a.example/v.mp4?x=1"), None), Some("video"));
        assert_eq!(classify("fetch", &u("https://a.example/seg1.ts"), None), None);
        assert_eq!(classify("fetch", &u("https://a.example/api/data"), Some("application/json")), None);
        assert_eq!(classify("image", &u("https://a.example/p.jpg"), None), Some("image"));
    }

    #[test]
    fn address_bar_input() {
        assert_eq!(normalize_input("example.com/x").unwrap().as_str(), "https://example.com/x");
        assert!(normalize_input("chats mignons").unwrap().as_str().starts_with("https://duckduckgo.com/?q=chats"));
        assert!(normalize_input("ftp://x.example/a").is_err());
        assert!(normalize_input("  ").is_err());
    }
}
