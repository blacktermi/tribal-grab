//! Enregistrement d'un flux (direct ou secours) avec ffmpeg. Arrêt propre par `q` : le fichier reste lisible.
use super::Ctx;
use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

pub struct RecordOpts {
    pub stop_at: Option<i64>,
    pub min_free_bytes: u64,
    pub user_agent: String,
}

pub struct Recorded {
    pub path: PathBuf,
    /// Pourquoi l'enregistrement s'est arrêté.
    pub reason: &'static str,
}

fn header_blob(h: &BTreeMap<String, String>) -> String {
    h.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("user-agent")).map(|(k, v)| format!("{k}: {v}\r\n")).collect()
}

/// Enregistre `input` (URL de manifeste ou de fichier) dans `out` sans réencoder.
/// Retourne `Ok` aussi après un arrêt demandé : le contenu déjà reçu est conservé.
pub async fn record(ctx: &Ctx, ffmpeg: &Path, input: &str, headers: &BTreeMap<String, String>, out: &Path, opts: &RecordOpts) -> Result<Recorded> {
    let mut args: Vec<String> = ["-y", "-hide_banner", "-loglevel", "error", "-nostats", "-progress", "pipe:1"].map(String::from).to_vec();
    args.extend(["-user_agent".into(), opts.user_agent.clone()]);
    let hb = header_blob(headers);
    if !hb.is_empty() {
        args.extend(["-headers".into(), hb]);
    }
    args.extend(["-reconnect", "1", "-reconnect_streamed", "1", "-reconnect_delay_max", "10"].map(String::from));
    args.extend(["-i".into(), input.to_string(), "-map".into(), "0:v?".into(), "-map".into(), "0:a?".into(), "-c".into(), "copy".into(), "-flush_packets".into(), "1".into()]);
    args.push(out.display().to_string());

    let mut child = Command::new(ffmpeg).args(&args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true).spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut b = Vec::new();
        let _ = (&mut stderr).take(100_000).read_to_end(&mut b).await;
        String::from_utf8_lossy(&b).into_owned()
    });
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    let mut reason = "fin du flux";
    let mut quit = false;
    loop {
        tokio::select! {
            l = lines.next_line() => match l? {
                Some(l) => if let Some(v) = l.strip_prefix("total_size=")
                    && let Ok(n) = v.trim().parse::<u64>() { ctx.meter.done.store(n, Ordering::Relaxed); },
                None => break,
            },
            _ = ctx.cancel.cancelled(), if !quit => { reason = "arrêt demandé"; quit = true; let _ = stdin.write_all(b"q").await; let _ = stdin.flush().await; }
            _ = tick.tick() => {
                if quit { continue; }
                if opts.stop_at.is_some_and(|t| crate::util::now() >= t) { reason = "heure d'arrêt atteinte"; quit = true; let _ = stdin.write_all(b"q").await; let _ = stdin.flush().await; }
                else if out.parent().and_then(crate::util::free_space).is_some_and(|f| f < opts.min_free_bytes) { reason = "disque presque plein"; quit = true; let _ = stdin.write_all(b"q").await; let _ = stdin.flush().await; }
            }
        }
        if quit {
            // ffmpeg doit se terminer seul ; sinon on le force après un délai.
            // `q`, puis SIGINT (ffmpeg finalise aussi sur SIGINT), puis arrêt forcé.
            if tokio::time::timeout(Duration::from_secs(4), child.wait()).await.is_err() {
                if let Some(pid) = child.id() {
                    unsafe { libc::kill(pid as i32, libc::SIGINT) };
                }
                if tokio::time::timeout(Duration::from_secs(12), child.wait()).await.is_err() {
                    let _ = child.kill().await;
                }
            }
            break;
        }
    }
    let status = child.wait().await.ok();
    let stderr = err_task.await.unwrap_or_default();
    let has_data = out.metadata().map(|m| m.len() > 0).unwrap_or(false);
    if !quit && status.is_some_and(|s| !s.success()) {
        let msg = crate::util::redact(stderr.lines().last().unwrap_or("erreur inconnue"));
        return Err(Error::Other(format!("ffmpeg a échoué : {msg}")));
    }
    if !has_data {
        let detail = crate::util::redact(stderr.lines().last().unwrap_or("ffmpeg n'a rien écrit"));
        return Err(Error::Other(format!("aucune donnée enregistrée ({detail}; code {:?}, fichier présent : {})", status.and_then(|s| s.code()), out.exists())));
    }
    Ok(Recorded { path: out.to_path_buf(), reason })
}
