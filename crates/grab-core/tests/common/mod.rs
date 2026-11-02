//! Outils de test : petit serveur HTTP local (plages, coupures), médias générés par ffmpeg, aides pour le moteur.
#![allow(dead_code)]
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::response::Response;
use grab_core::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

pub const FFMPEG: &str = "/opt/homebrew/bin/ffmpeg";
pub const FFPROBE: &str = "/opt/homebrew/bin/ffprobe";

pub struct Entry {
    pub data: Vec<u8>,
    pub ctype: String,
    pub headers: Vec<(String, String)>,
}

pub struct Shared {
    pub files: RwLock<HashMap<String, Entry>>,
    /// Refuse (403) toutes les requêtes tant que c'est vrai.
    pub deny: AtomicBool,
    /// Refuse les requêtes de plage « utiles » (hors sonde 0-0) à partir de la n-ième.
    pub deny_after: AtomicUsize,
    pub chunk_requests: AtomicUsize,
    /// Ignore `Range` et n'annonce pas `Accept-Ranges`.
    pub no_range: AtomicBool,
    /// Répond 503 aux n prochaines requêtes.
    pub fail_next: AtomicUsize,
    /// Refuse tout agent utilisateur de navigateur.
    pub reject_browsers: AtomicBool,
    /// Coupe la prochaine plage après ce nombre d'octets.
    pub cut_next: AtomicUsize,
    /// Délai avant chaque réponse, pour simuler un serveur lent.
    pub delay_ms: AtomicUsize,
    /// Plages servies : (début, fin).
    pub ranges: Mutex<Vec<(u64, u64)>>,
    pub hits: Mutex<Vec<String>>,
}

pub struct Server {
    pub base: String,
    pub shared: Arc<Shared>,
}

impl Server {
    pub fn add(&self, path: &str, data: Vec<u8>, ctype: &str) {
        self.shared.files.write().unwrap().insert(path.to_string(), Entry { data, ctype: ctype.into(), headers: vec![] });
    }
    pub fn add_with_headers(&self, path: &str, data: Vec<u8>, ctype: &str, headers: Vec<(String, String)>) {
        self.shared.files.write().unwrap().insert(path.to_string(), Entry { data, ctype: ctype.into(), headers });
    }
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
    /// Publie tous les fichiers d'un dossier sous `prefix/`.
    pub fn add_dir(&self, dir: &Path, prefix: &str) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let ctype = match name.rsplit('.').next().unwrap_or("") {
                "m3u8" => "application/vnd.apple.mpegurl",
                "ts" => "video/mp2t",
                "mpd" => "application/dash+xml",
                "m4s" | "mp4" => "video/mp4",
                _ => "application/octet-stream",
            };
            self.add(&format!("{prefix}/{name}"), std::fs::read(e.path()).unwrap(), ctype);
        }
    }
}

pub async fn serve() -> Server {
    let shared = Arc::new(Shared {
        files: Default::default(),
        deny: AtomicBool::new(false),
        deny_after: AtomicUsize::new(usize::MAX),
        chunk_requests: AtomicUsize::new(0),
        no_range: AtomicBool::new(false),
        fail_next: AtomicUsize::new(0),
        delay_ms: AtomicUsize::new(0),
        cut_next: AtomicUsize::new(0),
        reject_browsers: AtomicBool::new(false),
        ranges: Default::default(),
        hits: Default::default(),
    });
    let app = axum::Router::new().fallback(handler).with_state(shared.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Server { base: format!("http://127.0.0.1:{port}"), shared }
}

fn parse_range(h: &str, len: u64) -> Option<(u64, u64)> {
    let r = h.strip_prefix("bytes=")?;
    let (a, b) = r.split_once('-')?;
    let start: u64 = a.parse().ok()?;
    let end: u64 = if b.is_empty() { len - 1 } else { b.parse::<u64>().ok()?.min(len - 1) };
    (start <= end && start < len).then_some((start, end))
}

async fn handler(State(s): State<Arc<Shared>>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    s.hits.lock().unwrap().push(path.clone());
    let resp = |code: u16| Response::builder().status(code).body(Body::empty()).unwrap();
    if s.deny.load(Ordering::SeqCst) {
        return resp(403);
    }
    let browser = req.headers().get(header::USER_AGENT).and_then(|v| v.to_str().ok()).is_some_and(|u| u.contains("Mozilla"));
    if browser && s.reject_browsers.load(Ordering::SeqCst) {
        return resp(403);
    }
    let delay = s.delay_ms.load(Ordering::SeqCst);
    if delay > 0 {
        tokio::time::sleep(Duration::from_millis(delay as u64)).await;
    }
    if s.fail_next.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
        return resp(503);
    }
    let files = s.files.read().unwrap();
    let Some(e) = files.get(&path) else { return resp(404) };
    let len = e.data.len() as u64;
    let ranged = !s.no_range.load(Ordering::SeqCst);
    let range = req.headers().get(header::RANGE).and_then(|v| v.to_str().ok()).filter(|_| ranged).and_then(|v| parse_range(v, len));
    let mut b = Response::builder().header(header::CONTENT_TYPE, &e.ctype);
    if ranged {
        b = b.header(header::ACCEPT_RANGES, "bytes");
    }
    for (k, v) in &e.headers {
        b = b.header(k.as_str(), v.as_str());
    }
    match range {
        Some((a, z)) => {
            if z > a {
                let n = s.chunk_requests.fetch_add(1, Ordering::SeqCst);
                if n >= s.deny_after.load(Ordering::SeqCst) {
                    s.deny.store(true, Ordering::SeqCst); // coupure franche : plus rien ne passe
                    return resp(403);
                }
                s.ranges.lock().unwrap().push((a, z));
            }
            let cut = if z > a { s.cut_next.swap(0, Ordering::SeqCst) } else { 0 };
            let end = if cut > 0 { (a as usize + cut).min(z as usize + 1) } else { z as usize + 1 };
            let body = if req.method() == Method::HEAD { Body::empty() } else { Body::from(e.data[a as usize..end].to_vec()) };
            let b = b.status(StatusCode::PARTIAL_CONTENT).header(header::CONTENT_RANGE, format!("bytes {a}-{z}/{len}"));
            // Réponse coupée : hyper refuse un corps plus court que le `Content-Length` annoncé.
            let b = if cut > 0 { b } else { b.header(header::CONTENT_LENGTH, z - a + 1) };
            b.body(body).unwrap()
        }
        None => {
            let body = if req.method() == Method::HEAD { Body::empty() } else { Body::from(e.data.clone()) };
            b.status(200).header(header::CONTENT_LENGTH, len).body(body).unwrap()
        }
    }
}

// --- Médias générés par ffmpeg ---

pub fn have_ffmpeg() -> bool {
    Path::new(FFMPEG).exists() && Path::new(FFPROBE).exists()
}

fn ffmpeg(args: &[&str]) {
    let out = std::process::Command::new(FFMPEG).args(["-y", "-hide_banner", "-loglevel", "error"]).args(args).output().unwrap();
    assert!(out.status.success(), "ffmpeg : {}", String::from_utf8_lossy(&out.stderr));
}

const SRC: [&str; 8] = ["-f", "lavfi", "-i", "testsrc=d=6:s=160x90:r=10", "-f", "lavfi", "-i", "sine=frequency=440:d=6"];
const ENC: [&str; 9] = ["-c:v", "libx264", "-preset", "ultrafast", "-g", "20", "-pix_fmt", "yuv420p", "-c:a"];

pub fn make_webm(path: &Path) {
    let mut a: Vec<&str> = SRC.to_vec();
    a.extend(["-c:v", "libvpx-vp9", "-deadline", "realtime", "-cpu-used", "8", "-c:a", "libopus", "-shortest", path.to_str().unwrap()]);
    ffmpeg(&a);
}

pub fn make_mp4(path: &Path) {
    let mut a: Vec<&str> = SRC.to_vec();
    a.extend(&ENC);
    a.extend(["aac", "-shortest", path.to_str().unwrap()]);
    ffmpeg(&a);
}

pub fn make_hls(dir: &Path, encrypt: bool) {
    std::fs::create_dir_all(dir).unwrap();
    let seg = dir.join("seg%d.ts");
    let idx = dir.join("index.m3u8");
    let keyinfo = dir.join("keyinfo.txt");
    let mut a: Vec<&str> = SRC.to_vec();
    a.extend(&ENC);
    a.extend(["aac", "-f", "hls", "-hls_time", "2", "-hls_playlist_type", "vod", "-hls_segment_filename", seg.to_str().unwrap()]);
    if encrypt {
        std::fs::write(dir.join("key.bin"), [0x42u8; 16]).unwrap();
        std::fs::write(&keyinfo, format!("key.bin\n{}\n", dir.join("key.bin").display())).unwrap();
        a.extend(["-hls_key_info_file", keyinfo.to_str().unwrap()]);
    }
    a.push(idx.to_str().unwrap());
    ffmpeg(&a);
}

pub fn make_dash(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let mpd = dir.join("manifest.mpd");
    let mut a: Vec<&str> = SRC.to_vec();
    a.extend(&ENC);
    a.extend(["aac", "-f", "dash", "-seg_duration", "2", "-use_template", "1", "-use_timeline", "1", "-adaptation_sets", "id=0,streams=v id=1,streams=a", mpd.to_str().unwrap()]);
    ffmpeg(&a);
}

pub async fn probe_file(p: &Path) -> post::Probed {
    post::ffprobe(Path::new(FFPROBE), p).await.unwrap_or_else(|e| panic!("ffprobe {} : {e}", p.display()))
}

// --- Moteur ---

pub struct Env {
    pub dir: tempfile::TempDir,
    pub engine: Engine,
    pub out: PathBuf,
}

pub async fn env_with(f: impl FnOnce(&mut Settings)) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let data = dir.path().join("data");
    let engine = Engine::start(&data).await.unwrap();
    let mut s = engine.settings();
    s.download_dir = out.display().to_string();
    s.min_free_mb = 0;
    s.retry_max = 0;
    s.duplicate_policy = DuplicatePolicy::Redownload;
    f(&mut s);
    engine.set_settings(s).unwrap();
    Env { dir, engine, out }
}

pub async fn env() -> Env {
    env_with(|_| {}).await
}

pub async fn wait_until(e: &Engine, id: i64, secs: u64, what: &str, pred: impl Fn(&Task) -> bool) -> Task {
    let end = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(t) = e.task(id)
            && pred(&t) {
                return t;
            }
        if std::time::Instant::now() > end {
            panic!("délai dépassé en attendant : {what} ; tâche = {:?}", e.task(id).map(|t| (t.state, t.error, t.progress.stage)));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Attend la fin (terminé, échoué, annulé).
pub async fn done(e: &Engine, id: i64) -> Task {
    wait_until(e, id, 120, "fin de la tâche", |t| t.state.is_done()).await
}

pub async fn finished(e: &Engine, id: i64) -> Task {
    let t = done(e, id).await;
    assert_eq!(t.state, TaskState::Finished, "erreur : {:?}", t.error);
    t
}

pub fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}
