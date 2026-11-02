//! Recherche et lancement des outils externes (yt-dlp, ffmpeg, ffprobe, deno, node).
use crate::error::{Error, Result};
use crate::model::{Settings, ToolStatus};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub const TOOL_NAMES: [&str; 5] = ["yt-dlp", "ffmpeg", "ffprobe", "deno", "node"];
/// Dossier de `bin/` où `update::install_ytdlp` dépose yt-dlp décompressé.
pub const YTDLP_DIR: &str = "yt-dlp-app";
pub const YTDLP_EXE: &str = "yt-dlp_macos";

fn is_exec(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// Retourne le chemin et la provenance. Ordre : réglage, dossier de l'application, Homebrew,
/// /usr/local, ~/.local/node, puis PATH (une application lancée du Finder n'hérite pas du PATH).
pub fn find(name: &str, settings: &Settings, data_dir: &Path) -> Option<(PathBuf, &'static str)> {
    if let Some(p) = settings.tool_paths.get(name).filter(|p| !p.is_empty()) {
        let p = PathBuf::from(p);
        if is_exec(&p) {
            return Some((p, "réglage"));
        }
    }
    if name == "yt-dlp" {
        let p = data_dir.join("bin").join(YTDLP_DIR).join(YTDLP_EXE);
        if is_exec(&p) {
            return Some((p, "dossier de l'application"));
        }
    }
    let mut dirs: Vec<(PathBuf, &'static str)> = vec![
        (data_dir.join("bin"), "dossier de l'application"),
        ("/opt/homebrew/bin".into(), "Homebrew"),
        ("/usr/local/bin".into(), "/usr/local/bin"),
    ];
    let node_root = crate::util::home_dir().join(".local/node");
    if let Ok(rd) = std::fs::read_dir(&node_root) {
        let mut v: Vec<_> = rd.flatten().map(|e| e.path().join("bin")).collect();
        v.sort();
        v.reverse();
        dirs.extend(v.into_iter().map(|d| (d, "~/.local/node")));
    }
    if let Ok(rd) = std::fs::read_dir("/Library/Frameworks/Python.framework/Versions") {
        let mut v: Vec<_> = rd.flatten().map(|e| e.path().join("bin")).collect();
        v.sort();
        v.reverse();
        dirs.extend(v.into_iter().map(|d| (d, "Python.framework")));
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path).map(|d| (d, "PATH")));
    }
    dirs.into_iter().map(|(d, s)| (d.join(name), s)).find(|(p, _)| is_exec(p))
}

pub fn require(name: &str, settings: &Settings, data_dir: &Path) -> Result<PathBuf> {
    find(name, settings, data_dir).map(|x| x.0).ok_or_else(|| Error::ToolMissing(name.to_string()))
}

/// Arguments `--js-runtimes` pour yt-dlp : deno de préférence, sinon node.
pub fn js_runtime_args(settings: &Settings, data_dir: &Path) -> Vec<String> {
    for name in ["deno", "node"] {
        if let Some((p, _)) = find(name, settings, data_dir) {
            return vec!["--js-runtimes".into(), format!("{name}:{}", p.display())];
        }
    }
    vec![]
}

pub struct Captured {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

/// Lance un programme sans shell et capture sa sortie, avec limite de temps et de taille.
pub async fn run_capture(prog: &Path, args: &[String], timeout: Duration, max_out: usize) -> Result<Captured> {
    let mut child = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| Error::Other(format!("lancement de {} impossible : {e}", prog.display())))?;
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let work = async {
        let mut o = Vec::new();
        let mut e = Vec::new();
        let (a, b) = tokio::join!(
            async { (&mut out).take(max_out as u64).read_to_end(&mut o).await },
            async { (&mut err).take(1_000_000).read_to_end(&mut e).await }
        );
        a?;
        b?;
        let st = child.wait().await?;
        Ok::<_, std::io::Error>((st.code(), o, e))
    };
    match tokio::time::timeout(timeout, work).await {
        Ok(Ok((code, o, e))) => Ok(Captured { code, stdout: o, stderr: String::from_utf8_lossy(&e).into_owned() }),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Err(Error::Other(format!("{} ne répond pas (délai dépassé)", prog.file_name().unwrap_or_default().to_string_lossy()))),
    }
}

/// Les versions sont mises en cache (chemin, taille, date) : certains binaires démarrent lentement.
pub async fn version(path: &Path, name: &str) -> Option<String> {
    type Cache = std::sync::Mutex<std::collections::HashMap<(PathBuf, u64, u128), String>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let key = path.metadata().ok().map(|m| (path.to_path_buf(), m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0)));
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = key.as_ref().and_then(|k| cache.lock().unwrap().get(k).cloned()) {
        return Some(v);
    }
    let v = version_uncached(path, name).await;
    if let (Some(k), Some(v)) = (key, &v) {
        cache.lock().unwrap().insert(k, v.clone());
    }
    v
}

async fn version_uncached(path: &Path, name: &str) -> Option<String> {
    let arg = if name.starts_with("ff") { "-version" } else { "--version" };
    let c = run_capture(path, &[arg.to_string()], Duration::from_secs(40), 10_000).await.ok()?;
    let txt = String::from_utf8_lossy(&c.stdout).to_string();
    let first = txt.lines().next()?.trim().to_string();
    // « ffmpeg version 8.1 Copyright… » -> « 8.1 »
    Some(first.split_whitespace().skip_while(|w| *w != "version").nth(1).map(String::from).unwrap_or(first))
}

pub async fn status(settings: &Settings, data_dir: &Path) -> Vec<ToolStatus> {
    let mut v = Vec::new();
    for name in TOOL_NAMES {
        match find(name, settings, data_dir) {
            Some((p, src)) => v.push(ToolStatus {
                name: name.into(),
                version: version(&p, name).await,
                path: Some(p.display().to_string()),
                source: Some(src.into()),
            }),
            None => v.push(ToolStatus { name: name.into(), ..Default::default() }),
        }
    }
    v
}
