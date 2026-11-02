//! Repli : téléchargement par yt-dlp (protocoles inconnus ou échec du téléchargeur natif).
use super::{Ctx, PartDir};
use crate::error::{Error, Result};
use crate::model::Settings;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

/// Sélecteur de format yt-dlp d'après la résolution maximale.
pub fn selector(max_height: Option<u32>, audio_only: bool) -> String {
    if audio_only {
        return "ba/b".into();
    }
    match max_height {
        Some(h) => format!("bv*[height<={h}]+ba/b[height<={h}]/wv*+ba/w"),
        None => "bv*+ba/b".into(),
    }
}

pub async fn download(ctx: &Ctx, s: &Settings, data_dir: &Path, part: &PartDir, url: &str, selector: &str, merge_ext: &str) -> Result<PathBuf> {
    let bin = crate::tools::require("yt-dlp", s, data_dir)?;
    let mut args = crate::extract::ytdlp::base_args(s, data_dir);
    args.extend([
        "--newline".into(),
        "--no-playlist".into(),
        "--progress-template".into(),
        "download:TGP %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s".into(),
        "-f".into(),
        selector.into(),
        "--merge-output-format".into(),
        merge_ext.into(),
        "-N".into(),
        s.connections.clamp(1, 16).to_string(),
        "-o".into(),
        part.path("ytdlp.%(ext)s").display().to_string(),
    ]);
    if s.speed_limit_kbps > 0 {
        args.extend(["-r".into(), format!("{}K", ctx.throttle.kbps())]);
    }
    args.extend(["--".into(), url.to_string()]);

    let mut child = Command::new(&bin).args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true).spawn()?;
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stderr = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut b = Vec::new();
        let _ = (&mut stderr).take(200_000).read_to_end(&mut b).await;
        String::from_utf8_lossy(&b).into_owned()
    });
    loop {
        tokio::select! {
            l = lines.next_line() => match l? {
                Some(l) => {
                    if let Some(rest) = l.strip_prefix("TGP ") {
                        let v: Vec<u64> = rest.split_whitespace().map(|x| x.parse::<f64>().map(|f| f as u64).unwrap_or(0)).collect();
                        if v.len() == 3 {
                            ctx.meter.done.store(v[0], Ordering::Relaxed);
                            ctx.meter.total.store(if v[1] > 0 { v[1] } else { v[2] }, Ordering::Relaxed);
                        }
                    }
                }
                None => break,
            },
            _ = ctx.cancel.cancelled() => {
                let _ = child.kill().await;
                return Err(Error::Cancelled);
            }
        }
    }
    let status = child.wait().await?;
    let stderr = err_task.await.unwrap_or_default();
    if !status.success() {
        return Err(crate::extract::ytdlp::map_error(&stderr));
    }
    let found = std::fs::read_dir(&part.dir)?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy();
            n.starts_with("ytdlp.") && !n.ends_with(".part") && !n.ends_with(".ytdl") && !n.contains(".f")
        })
        .ok_or_else(|| Error::Other("yt-dlp n'a produit aucun fichier".into()))?;
    Ok(found)
}
