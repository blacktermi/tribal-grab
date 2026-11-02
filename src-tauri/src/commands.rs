//! Commandes Tauri appelées par les fenêtres locales. Aucune ne lance de programme choisi
//! par l'appelant : les chemins ouverts doivent désigner des fichiers connus du moteur.
use crate::prefs::{self, AppPrefs};
use crate::state::AppState;
use grab_core::history_io::Format;
use grab_core::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};

type R<T> = std::result::Result<T, Error>;

pub fn other(e: impl std::fmt::Display) -> Error {
    Error::Other(e.to_string())
}

// --- File d'attente ---

#[tauri::command]
pub fn tasks(st: State<'_, AppState>, sort: SortKey, desc: bool) -> Vec<Task> {
    st.engine.tasks(sort, desc)
}

#[tauri::command]
pub fn task_add_url(st: State<'_, AppState>, url: String) -> R<i64> {
    st.engine.add_url(&url)
}

#[tauri::command]
pub fn task_add_text(st: State<'_, AppState>, text: String) -> R<Vec<i64>> {
    st.engine.add_text(&text)
}

#[tauri::command]
pub fn task_add(st: State<'_, AppState>, req: AddRequest) -> R<i64> {
    st.engine.add(req)
}

#[tauri::command]
pub async fn probe(st: State<'_, AppState>, url: String) -> R<Resolved> {
    st.engine.probe(&url).await
}

#[tauri::command]
pub fn pause(st: State<'_, AppState>, id: i64) -> R<()> {
    st.engine.pause(id)
}
#[tauri::command]
pub fn resume(st: State<'_, AppState>, id: i64) -> R<()> {
    st.engine.resume(id)
}
#[tauri::command]
pub fn retry(st: State<'_, AppState>, id: i64) -> R<()> {
    st.engine.retry(id)
}
#[tauri::command]
pub fn cancel(st: State<'_, AppState>, id: i64) -> R<()> {
    st.engine.cancel(id)
}
#[tauri::command]
pub fn remove(st: State<'_, AppState>, id: i64) -> R<()> {
    st.engine.remove(id)
}
#[tauri::command]
pub fn pause_all(st: State<'_, AppState>) {
    st.engine.pause_all()
}
#[tauri::command]
pub fn resume_all(st: State<'_, AppState>) {
    st.engine.resume_all()
}
#[tauri::command]
pub fn cancel_all(st: State<'_, AppState>) {
    st.engine.cancel_all()
}
#[tauri::command]
pub fn start_queue(st: State<'_, AppState>) {
    st.engine.start_queue()
}
#[tauri::command]
pub fn clear_finished(st: State<'_, AppState>) {
    st.engine.clear_finished()
}
#[tauri::command]
pub fn clear_inactive(st: State<'_, AppState>) {
    st.engine.clear_inactive()
}
#[tauri::command]
pub fn reset_queue(st: State<'_, AppState>) {
    st.engine.reset_queue()
}
#[tauri::command]
pub fn set_priority(st: State<'_, AppState>, id: i64, priority: Priority) -> R<()> {
    st.engine.set_priority(id, priority)
}
#[tauri::command]
pub fn move_task(st: State<'_, AppState>, id: i64, index: usize) -> R<()> {
    st.engine.move_task(id, index)
}
#[tauri::command]
pub fn set_schedule(st: State<'_, AppState>, id: i64, scheduled_at: Option<i64>, stop_at: Option<i64>) -> R<()> {
    st.engine.set_schedule(id, scheduled_at, stop_at)
}
#[tauri::command]
pub fn stop_recording(st: State<'_, AppState>, id: i64) -> R<()> {
    st.engine.stop_recording(id)
}
#[tauri::command]
pub fn set_task_speed_limit(st: State<'_, AppState>, id: i64, kbps: u32) -> R<()> {
    st.engine.set_task_speed_limit(id, kbps)
}
#[tauri::command]
pub fn answer(st: State<'_, AppState>, id: i64, answer: Answer) -> R<()> {
    st.engine.answer(id, answer)
}

// --- Réglages ---

#[tauri::command]
pub fn get_settings(st: State<'_, AppState>) -> Settings {
    st.engine.settings()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, st: State<'_, AppState>, settings: Settings) -> R<Settings> {
    st.engine.set_settings(settings)?;
    let s = st.engine.settings();
    let _ = app.emit("settings://changed", &s);
    Ok(s)
}

#[tauri::command]
pub fn set_proxy_password(st: State<'_, AppState>, password: String) -> R<()> {
    st.engine.set_proxy_password(&password)
}

#[tauri::command]
pub fn get_app_prefs(st: State<'_, AppState>) -> AppPrefs {
    st.prefs.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_app_prefs(app: AppHandle, st: State<'_, AppState>, prefs: AppPrefs) -> R<AppPrefs> {
    let mut p = prefs;
    p.sanitize();
    p.save(&st.data_dir).map_err(|e| Error::Io(e.to_string()))?;
    *st.prefs.lock().unwrap() = p.clone();
    crate::mac::apply_prefs(&app, &p);
    let _ = app.emit("prefs://changed", &p);
    Ok(p)
}

// --- Historique ---

#[tauri::command]
pub fn history(st: State<'_, AppState>, query: String, sort: String, desc: bool, limit: i64) -> R<Vec<HistoryItem>> {
    st.engine.history(&query, &sort, desc, limit)
}
#[tauri::command]
pub fn history_delete(st: State<'_, AppState>, ids: Vec<i64>) -> R<()> {
    st.engine.history_delete(&ids)
}
#[tauri::command]
pub fn history_clear(st: State<'_, AppState>) -> R<()> {
    st.engine.history_clear()
}
#[tauri::command]
pub fn history_remove_missing(st: State<'_, AppState>) -> R<usize> {
    st.engine.history_remove_missing()
}
#[tauri::command]
pub fn history_redownload(st: State<'_, AppState>, id: i64) -> R<i64> {
    st.engine.history_redownload(id)
}

fn format_of(name: &str) -> R<Format> {
    match name {
        "json" => Ok(Format::Json),
        "csv" => Ok(Format::Csv),
        "xml" => Ok(Format::Xml),
        x => Err(other(format!("format inconnu : {x}"))),
    }
}

#[tauri::command]
pub fn history_export_file(st: State<'_, AppState>, format: String, path: String) -> R<()> {
    let data = st.engine.history_export(format_of(&format)?)?;
    std::fs::write(path, data).map_err(|e| Error::Io(e.to_string()))
}

#[tauri::command]
pub fn history_import_file(st: State<'_, AppState>, format: String, path: String) -> R<usize> {
    let data = read_small_text(Path::new(&path), 50 * 1024 * 1024)?;
    st.engine.history_import(format_of(&format)?, &data)
}

fn read_small_text(p: &Path, max: u64) -> R<String> {
    let meta = std::fs::metadata(p).map_err(|e| Error::Io(e.to_string()))?;
    if !meta.is_file() || meta.len() > max {
        return Err(other("fichier introuvable ou trop volumineux"));
    }
    std::fs::read_to_string(p).map_err(|e| Error::Io(format!("lecture impossible (texte UTF-8 attendu) : {e}")))
}

/// Importe tous les liens http(s) d'un fichier texte (un lien par ligne, ou texte libre).
#[tauri::command]
pub fn import_links_file(st: State<'_, AppState>, path: String) -> R<Vec<i64>> {
    let text = read_small_text(Path::new(&path), 5 * 1024 * 1024)?;
    st.engine.add_text(&text)
}

// --- Fichiers : uniquement ceux que l'application connaît ---

fn known_paths(st: &AppState) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = st.engine.tasks(SortKey::Order, false).into_iter().filter_map(|t| t.output_path).map(PathBuf::from).collect();
    if let Ok(h) = st.engine.history("", "date", true, 20000) {
        v.extend(h.into_iter().filter_map(|i| i.path).map(PathBuf::from));
    }
    v
}

/// Autorise un chemin s'il est une sortie connue ou s'il se trouve dans le dossier de téléchargement.
pub fn path_allowed(st: &AppState, path: &str) -> R<PathBuf> {
    let p = std::fs::canonicalize(path).map_err(|_| other("fichier introuvable"))?;
    let root = std::fs::canonicalize(st.engine.settings().download_dir).ok();
    let known = known_paths(st).into_iter().filter_map(|k| std::fs::canonicalize(k).ok()).any(|k| p == k || p.starts_with(&k));
    let in_root = root.is_some_and(|r| p.starts_with(r));
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(), "app" | "command" | "sh" | "zsh" | "terminal" | "workflow" | "scpt" | "pkg" | "dmg" | "jar" | "tool") {
        return Err(other("ce type de fichier ne peut pas être ouvert depuis Tribal Grab"));
    }
    if known || in_root { Ok(p) } else { Err(other("ce chemin n'appartient pas à vos téléchargements")) }
}

#[tauri::command]
pub fn reveal_in_finder(st: State<'_, AppState>, path: String) -> R<()> {
    let p = path_allowed(&st, &path)?;
    st.engine.reveal_in_finder(&p.to_string_lossy())
}

#[tauri::command]
pub fn open_path(st: State<'_, AppState>, path: String) -> R<()> {
    let p = path_allowed(&st, &path)?;
    st.engine.open_path(&p.to_string_lossy())
}

/// Aperçu Coup d'œil (`qlmanage -p`).
#[tauri::command]
pub fn quicklook(st: State<'_, AppState>, path: String) -> R<()> {
    let p = path_allowed(&st, &path)?;
    let mut slot = st.quicklook.lock().unwrap();
    if let Some(mut c) = slot.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    let child = std::process::Command::new("/usr/bin/qlmanage")
        .args(["-p", "--"])
        .arg(&p)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| Error::Io(e.to_string()))?;
    *slot = Some(child);
    Ok(())
}

#[tauri::command]
pub fn close_quicklook(st: State<'_, AppState>) {
    if let Some(mut c) = st.quicklook.lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

// --- Recherche et sites ---

#[tauri::command]
pub async fn search(st: State<'_, AppState>, query: String) -> R<Vec<SearchResult>> {
    st.engine.search(&query).await
}

#[tauri::command]
pub async fn sites(st: State<'_, AppState>, filter: String) -> R<SiteList> {
    st.engine.sites(&filter).await
}

// --- Intégrations ---

#[tauri::command]
pub fn integrations(st: State<'_, AppState>) -> Vec<IntegrationInfo> {
    st.engine.integrations()
}

/// Lit un fichier `.tgi` sans l'installer : l'interface demande confirmation avant `install_integration`.
#[tauri::command]
pub fn integration_read_file(path: String) -> R<serde_json::Value> {
    crate::opener::read_tgi(Path::new(&path))
}

#[tauri::command]
pub fn install_integration(st: State<'_, AppState>, json: String) -> R<IntegrationInfo> {
    st.engine.install_integration(&json)
}
#[tauri::command]
pub fn save_integration(st: State<'_, AppState>, info: IntegrationInfo) -> R<IntegrationInfo> {
    st.engine.save_integration(info)
}
#[tauri::command]
pub fn set_integration_enabled(st: State<'_, AppState>, name: String, enabled: bool) -> R<()> {
    st.engine.set_integration_enabled(&name, enabled)
}
#[tauri::command]
pub fn remove_integration(st: State<'_, AppState>, name: String) -> R<()> {
    st.engine.remove_integration(&name)
}
#[tauri::command]
pub fn export_integration_file(st: State<'_, AppState>, name: String, path: String) -> R<()> {
    let json = st.engine.export_integration(&name)?;
    std::fs::write(path, json).map_err(|e| Error::Io(e.to_string()))
}
#[tauri::command]
pub async fn test_integration(st: State<'_, AppState>, info: IntegrationInfo, url: String) -> R<grab_core::engine::IntegrationTest> {
    Ok(st.engine.test_integration(info, &url).await)
}

// --- Outils ---

#[tauri::command]
pub async fn tools(st: State<'_, AppState>) -> R<Vec<ToolStatus>> {
    Ok(st.engine.tools().await)
}
#[tauri::command]
pub fn set_tool_path(st: State<'_, AppState>, name: String, path: Option<String>) -> R<()> {
    if !matches!(name.as_str(), "yt-dlp" | "ffmpeg" | "ffprobe" | "deno" | "node") {
        return Err(other("outil inconnu"));
    }
    st.engine.set_tool_path(&name, path)
}
#[tauri::command]
pub async fn check_ytdlp_update(st: State<'_, AppState>) -> R<grab_core::update::UpdateInfo> {
    st.engine.check_ytdlp_update().await
}
#[tauri::command]
pub async fn update_ytdlp(st: State<'_, AppState>) -> R<String> {
    st.engine.update_ytdlp().await
}

#[derive(Serialize)]
pub struct Bootstrap {
    /// yt-dlp absent de `bin/yt-dlp-app` : l'interface lance `update_ytdlp`.
    pub needs_ytdlp: bool,
}

#[tauri::command]
pub fn bootstrap_status(st: State<'_, AppState>) -> Bootstrap {
    let dir = st.data_dir.join("bin/yt-dlp-app");
    let present = std::fs::read_dir(&dir).map(|d| d.count() > 0).unwrap_or(false);
    Bootstrap { needs_ytdlp: !present }
}

// --- Système ---

#[tauri::command]
pub fn read_clipboard() -> String {
    std::process::Command::new("/usr/bin/pbpaste")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).chars().take(1_000_000).collect())
        .unwrap_or_default()
}

#[tauri::command]
pub fn list_sounds() -> Vec<String> {
    prefs::system_sounds()
}

#[tauri::command]
pub fn play_sound(name: String) {
    prefs::play_sound(&name)
}

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub extension_dir: String,
    pub cookies_file: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle, st: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: st.data_dir.to_string_lossy().into(),
        extension_dir: st.data_dir.join("extension").to_string_lossy().into(),
        cookies_file: crate::sites::cookies_path(&st.data_dir).to_string_lossy().into(),
    }
}

/// Fenêtres : "prefs" (avec section), "library" (avec onglet), "browser" (avec adresse et mode).
#[tauri::command]
pub fn open_window(app: AppHandle, name: String, section: Option<String>, url: Option<String>) -> R<()> {
    match name.as_str() {
        "main" => crate::windows::show_main(&app),
        "prefs" => crate::windows::show_prefs(&app, section.as_deref()),
        "library" => crate::windows::show_library(&app, section.as_deref()),
        "browser" => crate::guided::open_browser(&app, url.as_deref(), section.as_deref().unwrap_or("guided"))?,
        x => return Err(other(format!("fenêtre inconnue : {x}"))),
    }
    Ok(())
}

/// Copie l'extension Chromium dans le dossier de données et l'affiche dans le Finder.
#[tauri::command]
pub fn extension_install(app: AppHandle, st: State<'_, AppState>) -> R<String> {
    let src = app.path().resource_dir().map_err(other)?.join("extension");
    let dst = st.data_dir.join("extension");
    copy_dir(&src, &dst).map_err(|e| Error::Io(format!("copie de l'extension impossible : {e}")))?;
    let _ = std::process::Command::new("/usr/bin/open").arg("-R").arg(dst.join("manifest.json")).spawn();
    Ok(dst.to_string_lossy().into())
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
