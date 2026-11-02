use crate::error::{Error, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_HTML_BYTES: usize = 4 * 1024 * 1024;

/// Accepte uniquement http et https.
pub fn validate_url(s: &str) -> Result<url::Url> {
    let s = s.trim();
    let u = url::Url::parse(s).map_err(|_| Error::InvalidUrl(format!("« {} » n'est pas une adresse valide", short(s))))?;
    match u.scheme() {
        "http" | "https" if u.host_str().is_some() => Ok(u),
        _ => Err(Error::InvalidUrl("seules les adresses http et https sont acceptées".into())),
    }
}

fn short(s: &str) -> String {
    s.chars().take(60).collect()
}

/// Retire des textes de journal tout ce qui ressemble à un secret : requêtes d'URL, cookies, jetons.
pub fn redact(s: &str) -> String {
    static URL: OnceLock<Regex> = OnceLock::new();
    static SECRET: OnceLock<Regex> = OnceLock::new();
    let url = URL.get_or_init(|| Regex::new(r#"(https?://[^\s"'?#<>]+)[?#][^\s"'<>]*"#).unwrap());
    let secret = SECRET.get_or_init(|| {
        Regex::new(r#"(?i)(cookie|authorization|token|signature|password|passwd)(["']?\s*[:=]\s*)[^\s"',;&]+"#).unwrap()
    });
    let a = url.replace_all(s, "$1?…");
    secret.replace_all(&a, "$1$2…").into_owned()
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Octets libres sur le volume qui contient `path` (ou son premier parent existant).
pub fn free_space(path: &Path) -> Option<u64> {
    let mut p = path.to_path_buf();
    while !p.exists() {
        if !p.pop() {
            return None;
        }
    }
    let c = std::ffi::CString::new(p.to_str()?).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::statvfs(c.as_ptr(), &mut st) };
    if r != 0 {
        return None;
    }
    Some(st.f_bavail as u64 * st.f_frsize)
}

pub fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

pub fn host_of(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string())).unwrap_or_default()
}

/// Compteur partagé entre les tâches de téléchargement et le rapporteur de progression.
#[derive(Default)]
pub struct Meter {
    pub done: AtomicU64,
    /// 0 = inconnu
    pub total: AtomicU64,
}

impl Meter {
    pub fn add(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }
    pub fn get(&self) -> (u64, Option<u64>) {
        let t = self.total.load(Ordering::Relaxed);
        (self.done.load(Ordering::Relaxed), (t > 0).then_some(t))
    }
}

/// Limiteur de débit par téléchargement, en ko/s. 0 = illimité.
pub struct Throttle {
    kbps: AtomicU64,
    state: Mutex<(Instant, u64)>,
}

impl Throttle {
    pub fn new(kbps: u32) -> Self {
        Self { kbps: AtomicU64::new(kbps as u64), state: Mutex::new((Instant::now(), 0)) }
    }
    pub fn set(&self, kbps: u32) {
        self.kbps.store(kbps as u64, Ordering::Relaxed);
    }
    pub fn kbps(&self) -> u64 {
        self.kbps.load(Ordering::Relaxed)
    }
    pub async fn wait(&self, n: usize) {
        let rate = self.kbps() * 1024;
        if rate == 0 {
            return;
        }
        let sleep = {
            let mut g = self.state.lock().unwrap();
            g.1 += n as u64;
            let target = Duration::from_secs_f64(g.1 as f64 / rate as f64);
            let el = g.0.elapsed();
            if el > target + Duration::from_secs(1) {
                *g = (Instant::now(), n as u64); // pas de crédit accumulé pendant les pauses
                Duration::ZERO
            } else {
                target.saturating_sub(el)
            }
        };
        if !sleep.is_zero() {
            tokio::time::sleep(sleep).await;
        }
    }
}

/// Écrit un fichier de façon atomique (fichier temporaire puis renommage).
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp-write");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn fmt_size(n: u64) -> String {
    let mut v = n as f64;
    for u in ["o", "Ko", "Mo", "Go", "To"] {
        if v < 1024.0 {
            return format!("{v:.1} {u}");
        }
        v /= 1024.0;
    }
    format!("{v:.1} To")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_validation() {
        assert!(validate_url("https://a.com/x").is_ok());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("ftp://a.com/x").is_err());
    }

    #[test]
    fn redaction() {
        let r = redact("GET https://cdn.x.com/v.mp4?token=abc&sig=zzz échoué, Cookie: sid=12345");
        assert!(!r.contains("abc") && !r.contains("zzz") && !r.contains("12345"), "{r}");
        assert!(r.contains("cdn.x.com/v.mp4"));
    }
}
