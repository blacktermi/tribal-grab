//! Post-traitement : fusion, conversion, métadonnées, miniature, sous-titres intégrés, scripts, envoi à une application.
use crate::error::{Error, Result};
use crate::model::*;
use crate::tools;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Réencodage matériel d'un flux vidéo que QuickTime ne lit pas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoTarget {
    pub hevc: bool,
    pub ten_bit: bool,
    pub kbps: u64,
}

#[derive(Default, Clone)]
pub struct FinalizeJob {
    pub video: Option<PathBuf>,
    pub audio: Option<PathBuf>,
    /// Flux unique (déjà muxé, ou audio seul).
    pub single: Option<PathBuf>,
    /// Extension réelle du flux unique (le fichier s'appelle `.bin`).
    pub single_ext: String,
    pub subs: Vec<(PathBuf, String)>,
    pub thumb: Option<PathBuf>,
    pub out: PathBuf,
    pub audio_target: Option<AudioFormat>,
    pub source_acodec: Option<String>,
    pub reencode: bool,
    pub compat: Compat,
    pub metadata: Vec<(String, String)>,
}

/// Ce qu'il faut changer pour qu'un MP4 s'ouvre dans QuickTime et le Finder.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Compat {
    pub video: Option<VideoTarget>,
    pub audio: bool,
    /// QuickTime refuse le HEVC étiqueté `hev1`, même si le flux est valide.
    pub hevc_tag: bool,
}

impl Compat {
    pub fn is_needed(&self) -> bool {
        self.video.is_some() || self.audio || self.hevc_tag
    }
}

fn apple_video(codec: &str) -> bool {
    matches!(codec, "h264" | "hevc" | "mpeg4" | "prores")
}

fn apple_audio(codec: &str) -> bool {
    matches!(codec, "aac" | "mp3" | "alac" | "ac3" | "eac3") || codec.starts_with("pcm_")
}

/// La qualité passe avant la taille : 2,5 fois le débit source, avec un plancher de 0,12 bit par pixel à 30 images/s.
fn video_target(p: &Probed, bytes: u64) -> VideoTarget {
    let (w, h) = (p.width.unwrap_or(1920) as u64, p.height.unwrap_or(1080) as u64);
    let ten_bit = p.pix_fmt.as_deref().is_some_and(|f| f.contains("10"));
    let source = p.duration.filter(|d| *d > 0.0).map(|d| (bytes as f64 * 8.0 / d / 1000.0) as u64).unwrap_or(0);
    let floor = w * h * 30 * 12 / 100 / 1000;
    VideoTarget { hevc: ten_bit || w.max(h) > 4096, ten_bit, kbps: (source * 25 / 10).max(floor).min(200_000) }
}

/// Lit les flux réellement téléchargés : les métadonnées des sites sont souvent absentes ou fausses.
pub async fn compat_for(ffprobe_bin: &Path, video: Option<&Path>, audio: Option<&Path>) -> Compat {
    let mut c = Compat::default();
    if let Some(p) = video
        && let Ok(v) = ffprobe(ffprobe_bin, p).await
        && let Some(codec) = v.vcodec.as_deref()
    {
        if !apple_video(codec) {
            c.video = Some(video_target(&v, p.metadata().map(|m| m.len()).unwrap_or(0)));
        }
        c.hevc_tag = codec == "hevc";
    }
    if let Some(p) = audio
        && let Ok(a) = ffprobe(ffprobe_bin, p).await
    {
        c.audio = a.acodec.as_deref().is_some_and(|x| !apple_audio(x));
    }
    c
}

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Y a-t-il quelque chose à faire avec ffmpeg, ou suffit-il de déplacer le fichier ?
pub fn needs_ffmpeg(j: &FinalizeJob) -> bool {
    if j.video.is_some() || j.audio.is_some() || !j.subs.is_empty() || j.thumb.is_some() || !j.metadata.is_empty() || j.audio_target.is_some() || j.reencode || j.compat.is_needed() {
        return true;
    }
    j.single_ext != ext_of(&j.out)
}

/// Construit les arguments ffmpeg (séparé de l'exécution pour pouvoir le tester).
pub fn ffmpeg_args(j: &FinalizeJob) -> Vec<String> {
    let mut a: Vec<String> = ["-y", "-hide_banner", "-loglevel", "error", "-nostdin"].map(String::from).to_vec();
    let mut inputs: Vec<&Path> = Vec::new();
    let mut maps: Vec<String> = Vec::new();
    let out_ext = ext_of(&j.out);
    let audio_only_out = matches!(out_ext.as_str(), "m4a" | "mp3" | "aac" | "ogg" | "opus" | "flac" | "wav");

    let main_count;
    if let (Some(v), Some(au)) = (&j.video, &j.audio) {
        inputs.extend([v.as_path(), au.as_path()]);
        maps.extend(["0:v:0".into(), "1:a:0".into()]);
        main_count = 2;
    } else {
        let s = j.single.as_ref().or(j.video.as_ref()).or(j.audio.as_ref()).expect("flux principal manquant");
        inputs.push(s.as_path());
        if audio_only_out {
            maps.push("0:a:0".into());
        } else {
            maps.extend(["0:v?".into(), "0:a?".into()]);
        }
        main_count = 1;
    }
    let sub_first = main_count;
    for (p, _) in &j.subs {
        inputs.push(p.as_path());
    }
    let thumb_idx = j.thumb.as_ref().map(|t| {
        inputs.push(t.as_path());
        inputs.len() - 1
    });
    for i in &inputs {
        a.extend(["-i".into(), i.display().to_string()]);
    }
    for m in &maps {
        a.extend(["-map".into(), m.clone()]);
    }
    for (i, _) in j.subs.iter().enumerate() {
        a.extend(["-map".into(), (sub_first + i).to_string()]);
    }
    if let Some(t) = thumb_idx {
        a.extend(["-map".into(), t.to_string()]);
    }

    // Codecs
    if audio_only_out {
        a.push("-vn".into());
        match j.audio_target {
            Some(AudioFormat::Mp3) => a.extend(["-c:a", "libmp3lame", "-q:a", "2"].map(String::from)),
            _ => {
                let aac_src = j.source_acodec.as_deref().is_some_and(|c| c.starts_with("mp4a") || c.starts_with("aac"));
                if (out_ext == "m4a" && !aac_src) || j.reencode {
                    a.extend(["-c:a", "aac", "-b:a", "192k"].map(String::from));
                } else {
                    a.extend(["-c:a", "copy"].map(String::from));
                }
            }
        }
        if let Some(_t) = thumb_idx {
            a.extend(["-c:v", "mjpeg", "-disposition:v", "attached_pic"].map(String::from));
        }
        if out_ext == "mp3" {
            a.extend(["-id3v2_version", "3"].map(String::from));
        }
    } else {
        if j.reencode {
            a.extend(["-c:v", "libx264", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k"].map(String::from));
        } else {
            match j.compat.video {
                Some(t) => {
                    let (encoder, tag) = if t.hevc { ("hevc_videotoolbox", "hvc1") } else { ("h264_videotoolbox", "avc1") };
                    a.extend(["-c:v".into(), encoder.into(), "-b:v".into(), format!("{}k", t.kbps), "-allow_sw".into(), "1".into(), "-tag:v:0".into(), tag.into()]);
                    if t.ten_bit {
                        a.extend(["-profile:v", "main10", "-pix_fmt", "p010le"].map(String::from));
                    } else {
                        a.extend(["-pix_fmt", "yuv420p"].map(String::from));
                    }
                }
                None => {
                    a.extend(["-c:v", "copy"].map(String::from));
                    if j.compat.hevc_tag {
                        a.extend(["-tag:v:0", "hvc1"].map(String::from));
                    }
                }
            }
            if j.compat.audio {
                a.extend(["-c:a", "aac", "-b:a", "256k"].map(String::from));
            } else {
                a.extend(["-c:a", "copy"].map(String::from));
            }
        }
        if !j.subs.is_empty() {
            a.extend(["-c:s".into(), if out_ext == "mp4" || out_ext == "m4v" { "mov_text".into() } else { "srt".into() }]);
            for (i, (_, lang)) in j.subs.iter().enumerate() {
                a.extend([format!("-metadata:s:s:{i}"), format!("language={}", iso3(lang))]);
            }
        }
        if thumb_idx.is_some() {
            // La miniature est le seul flux vidéo supplémentaire : copie en MJPEG attaché.
            let vi = if j.video.is_some() || j.single.is_some() { 1 } else { 0 };
            a.extend([format!("-c:v:{vi}"), "mjpeg".into(), format!("-disposition:v:{vi}"), "attached_pic".into()]);
        }
        if matches!(out_ext.as_str(), "mp4" | "m4v" | "mov") {
            a.extend(["-movflags", "+faststart"].map(String::from));
        }
    }
    for (k, v) in &j.metadata {
        a.extend(["-metadata".into(), format!("{k}={v}")]);
    }
    a.push(j.out.display().to_string());
    a
}

/// Code de langue sur 3 lettres pour les pistes de sous-titres intégrées (ffmpeg accepte « fre », pas « fr »).
fn iso3(l: &str) -> String {
    let base = l.split(['-', '_']).next().unwrap_or(l).to_lowercase();
    match base.as_str() {
        "fr" => "fre", "en" => "eng", "es" => "spa", "de" => "ger", "it" => "ita", "pt" => "por", "ar" => "ara", "ja" => "jpn",
        "ko" => "kor", "zh" => "chi", "ru" => "rus", "nl" => "dut", "tr" => "tur", "pl" => "pol", "hi" => "hin",
        other if other.len() == 3 => return other.to_string(),
        _ => "und",
    }
    .to_string()
}

pub async fn run_ffmpeg(ffmpeg: &Path, args: &[String], timeout: Duration) -> Result<()> {
    let c = tools::run_capture(ffmpeg, args, timeout, 1000).await?;
    if c.code != Some(0) {
        let msg = crate::util::redact(c.stderr.lines().last().unwrap_or("erreur inconnue"));
        return Err(Error::Other(format!("ffmpeg a échoué : {msg}")));
    }
    Ok(())
}

/// Produit `job.out`. Retourne `Ok` seulement si le fichier existe.
pub async fn finalize(ffmpeg: &Path, job: &FinalizeJob) -> Result<()> {
    if !needs_ffmpeg(job) {
        std::fs::rename(job.single.as_ref().unwrap(), &job.out)?;
        return Ok(());
    }
    let mut j = job.clone();
    let timeout = Duration::from_secs(4 * 3600);
    match run_ffmpeg(ffmpeg, &ffmpeg_args(&j), timeout).await {
        Ok(()) => Ok(()),
        Err(first) => {
            // Repli : sans sous-titres ni miniature (souvent incompatibles), puis réencodage.
            if !j.reencode && !matches!(ext_of(&j.out).as_str(), "m4a" | "mp3") {
                tracing::warn!("copie impossible, réencodage : {first}");
                j.reencode = true;
                j.thumb = None;
                run_ffmpeg(ffmpeg, &ffmpeg_args(&j), timeout).await.map_err(|_| first)
            } else if j.thumb.is_some() {
                j.thumb = None;
                run_ffmpeg(ffmpeg, &ffmpeg_args(&j), timeout).await.map_err(|_| first)
            } else {
                Err(first)
            }
        }
    }
}

/// Garde les flux tels que téléchargés, sans perte. MKV accepte tous les codecs et VLC le lit.
pub async fn mux_original(ffmpeg: &Path, job: &FinalizeJob, out: &Path) -> Result<()> {
    let mut args: Vec<String> = ["-y", "-hide_banner", "-loglevel", "error", "-nostdin"].map(String::from).to_vec();
    let inputs: Vec<&PathBuf> = [&job.video, &job.audio, &job.single].into_iter().flatten().collect();
    for p in &inputs {
        args.extend(["-i".into(), p.display().to_string()]);
    }
    for i in 0..inputs.len() {
        args.extend(["-map".into(), format!("{i}:v?"), "-map".into(), format!("{i}:a?")]);
    }
    args.extend(["-c", "copy"].map(String::from));
    for (k, v) in &job.metadata {
        args.extend(["-metadata".into(), format!("{k}={v}")]);
    }
    args.push(out.display().to_string());
    run_ffmpeg(ffmpeg, &args, Duration::from_secs(4 * 3600)).await
}

#[derive(Debug, Clone, Default)]
pub struct Probed {
    pub duration: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pix_fmt: Option<String>,
}

pub async fn ffprobe(ffprobe: &Path, file: &Path) -> Result<Probed> {
    let args = ["-v", "error", "-show_entries", "stream=codec_type,codec_name,width,height,pix_fmt:format=duration", "-of", "json"].map(String::from);
    let mut args = args.to_vec();
    args.push(file.display().to_string());
    let c = tools::run_capture(ffprobe, &args, Duration::from_secs(60), 1_000_000).await?;
    if c.code != Some(0) {
        return Err(Error::Other(format!("ffprobe ne peut pas lire ce fichier : {}", crate::util::redact(c.stderr.trim()))));
    }
    let v: Value = serde_json::from_slice(&c.stdout)?;
    let mut p = Probed { duration: v["format"]["duration"].as_str().and_then(|d| d.parse().ok()), ..Default::default() };
    for s in v["streams"].as_array().cloned().unwrap_or_default() {
        match s["codec_type"].as_str() {
            Some("video") if p.vcodec.is_none() && s["codec_name"] != "mjpeg" => {
                p.vcodec = s["codec_name"].as_str().map(String::from);
                p.width = s["width"].as_u64().map(|x| x as u32);
                p.height = s["height"].as_u64().map(|x| x as u32);
                p.pix_fmt = s["pix_fmt"].as_str().map(String::from);
            }
            Some("audio") if p.acodec.is_none() => p.acodec = s["codec_name"].as_str().map(String::from),
            _ => {}
        }
    }
    Ok(p)
}

/// Convertit n'importe quelle image (webp, png…) en JPEG.
pub async fn to_jpeg(ffmpeg: &Path, src: &Path, dst: &Path) -> Result<()> {
    let args = ["-y", "-hide_banner", "-loglevel", "error", "-nostdin", "-i", &src.display().to_string(), "-frames:v", "1", "-q:v", "2", &dst.display().to_string()].map(String::from);
    run_ffmpeg(ffmpeg, &args, Duration::from_secs(60)).await
}

/// Exécute le script de l'utilisateur avec `/bin/zsh -c`. C'est la seule commande lancée dans un shell.
pub async fn run_script(script: &str, env: &[(&str, String)]) -> Result<()> {
    let mut cmd = tokio::process::Command::new("/bin/zsh");
    cmd.arg("-c").arg(script).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = tokio::time::timeout(Duration::from_secs(900), cmd.output()).await.map_err(|_| Error::Other("le script personnalisé a dépassé 15 minutes".into()))??;
    if !out.status.success() {
        let e = String::from_utf8_lossy(&out.stderr);
        return Err(Error::Other(format!("le script personnalisé a échoué : {}", e.lines().last().unwrap_or("code de sortie non nul"))));
    }
    Ok(())
}

pub async fn open_with_app(app: &str, file: &Path) -> Result<()> {
    let args = vec!["-a".to_string(), app.to_string(), file.display().to_string()];
    let c = tools::run_capture(Path::new("/usr/bin/open"), &args, Duration::from_secs(20), 1000).await?;
    if c.code != Some(0) {
        return Err(Error::Other(format!("impossible d'ouvrir « {app} » : {}", c.stderr.trim())));
    }
    Ok(())
}

/// Ajoute le fichier à Music (ou TV) par osascript ; le chemin passe en argument, jamais dans le script.
pub async fn add_to_music(file: &Path, tv: bool) -> Result<()> {
    let app = if tv { "TV" } else { "Music" };
    let args = vec![
        "-e".into(),
        "on run argv".into(),
        "-e".into(),
        format!("tell application \"{app}\" to add (POSIX file (item 1 of argv))"),
        "-e".into(),
        "end run".into(),
        file.display().to_string(),
    ];
    let c = tools::run_capture(Path::new("/usr/bin/osascript"), &args, Duration::from_secs(60), 1000).await?;
    if c.code != Some(0) {
        return Err(Error::Other(format!("ajout à {app} impossible : {}", c.stderr.trim())));
    }
    Ok(())
}

/// Ordre de grandeur de l'espace nécessaire pour un téléchargement (le flux plus sa copie finale).
pub fn space_needed(size: Option<u64>) -> u64 {
    size.map(|s| s.saturating_mul(2)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_merge_with_subs_and_metadata() {
        let j = FinalizeJob {
            video: Some("/p/v.bin".into()),
            audio: Some("/p/a.bin".into()),
            subs: vec![("/p/s.srt".into(), "fr".into())],
            out: "/p/out.mp4".into(),
            metadata: vec![("title".into(), "Mon titre".into())],
            ..Default::default()
        };
        let a = ffmpeg_args(&j).join(" ");
        assert!(a.contains("-map 0:v:0 -map 1:a:0 -map 2"), "{a}");
        assert!(a.contains("-c:s mov_text"));
        assert!(a.contains("language=fre"));
        assert!(a.contains("-metadata title=Mon titre"));
        assert!(a.ends_with("/p/out.mp4"));
    }

    #[test]
    fn args_audio_targets() {
        let mut j = FinalizeJob { single: Some("/p/x.bin".into()), out: "/p/o.mp3".into(), audio_target: Some(AudioFormat::Mp3), ..Default::default() };
        assert!(ffmpeg_args(&j).join(" ").contains("libmp3lame"));
        j.out = "/p/o.m4a".into();
        j.audio_target = Some(AudioFormat::M4a);
        j.source_acodec = Some("mp4a.40.2".into());
        assert!(ffmpeg_args(&j).join(" ").contains("-c:a copy"));
        j.source_acodec = Some("opus".into());
        assert!(ffmpeg_args(&j).join(" ").contains("-c:a aac"));
    }

    #[test]
    fn compat_uses_hardware_encoder_and_keeps_readable_audio() {
        let compat = Compat { video: Some(VideoTarget { hevc: false, ten_bit: false, kbps: 5000 }), ..Default::default() };
        let a = ffmpeg_args(&FinalizeJob { video: Some("/p/v.bin".into()), audio: Some("/p/a.bin".into()), out: "/p/o.mp4".into(), compat, ..Default::default() }).join(" ");
        assert!(a.contains("-c:v h264_videotoolbox -b:v 5000k"), "{a}");
        assert!(a.contains("-tag:v:0 avc1"));
        assert!(a.contains("-c:a copy"));
    }

    #[test]
    fn compat_converts_opus_and_retags_hevc_without_reencoding() {
        let compat = Compat { audio: true, hevc_tag: true, ..Default::default() };
        let j = FinalizeJob { single: Some("/p/x.mp4".into()), single_ext: "mp4".into(), out: "/q/y.mp4".into(), compat, ..Default::default() };
        assert!(needs_ffmpeg(&j));
        let a = ffmpeg_args(&j).join(" ");
        assert!(a.contains("-c:v copy -tag:v:0 hvc1"), "{a}");
        assert!(a.contains("-c:a aac"));
    }

    #[test]
    fn ten_bit_and_8k_go_to_hevc_with_a_sane_bitrate() {
        let p = |w, h, f: &str| Probed { width: Some(w), height: Some(h), pix_fmt: Some(f.into()), duration: Some(10.0), ..Default::default() };
        let hd = video_target(&p(1080, 1920, "yuv420p"), 2_000_000);
        assert!(!hd.hevc);
        assert!(hd.kbps >= 7400, "{}", hd.kbps);
        assert!(video_target(&p(3840, 2160, "yuv420p10le"), 1).ten_bit);
        assert!(video_target(&p(7680, 4320, "yuv420p"), 1).hevc);
    }

    #[test]
    fn skips_ffmpeg_when_nothing_to_do() {
        let j = FinalizeJob { single: Some("/p/x.mp4".into()), single_ext: "mp4".into(), out: "/q/y.mp4".into(), ..Default::default() };
        assert!(!needs_ffmpeg(&j));
        let j2 = FinalizeJob { single: Some("/p/x.bin".into()), single_ext: "ts".into(), out: "/q/y.mp4".into(), ..Default::default() };
        assert!(needs_ffmpeg(&j2));
    }
}
