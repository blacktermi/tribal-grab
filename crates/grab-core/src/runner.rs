//! Exécution d'une tâche : résolution, choix des formats, téléchargement, post-traitement, historique.
use crate::download::{self, Ctx, PartDir, http, live, segments, ytdlp_dl};
use crate::engine::{Inner, RunHandle, STOP_CANCEL};
use crate::error::{Error, Result};
use crate::model::*;
use crate::quality::{self, Prefs, Selection};
use crate::util::{Meter, now};
use crate::{naming, post, subs, tools};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) enum Outcome {
    Done,
    Question(Question),
    Replaced,
    Skipped(String),
}

pub(crate) async fn run(inner: Arc<Inner>, id: i64, h: RunHandle) {
    let res = pipeline(&inner, id, &h).await;
    finish(&inner, id, &h, res).await;
    inner.wake.notify_one();
}

fn retriable(e: &Error) -> bool {
    !matches!(e, Error::Drm | Error::Unsupported(_) | Error::InvalidUrl(_) | Error::ToolMissing(_) | Error::DiskFull(_))
}

async fn finish(inner: &Arc<Inner>, id: i64, h: &RunHandle, res: Result<Outcome>) {
    let remove = {
        let mut st = inner.state.lock().unwrap();
        match st.tasks.get_mut(&id) {
            Some(e) => {
                e.run = None;
                e.remove_when_done
            }
            None => return,
        }
    };
    match res {
        Ok(Outcome::Done) => {}
        Ok(Outcome::Question(q)) => {
            inner.update(id, |t| {
                t.state = TaskState::NeedsInput;
                t.question = Some(q);
                t.progress.speed = 0;
            });
        }
        Ok(Outcome::Replaced) => inner.remove_entry(id),
        Ok(Outcome::Skipped(msg)) => {
            inner.update(id, |t| {
                t.state = TaskState::Cancelled;
                t.error = Some(msg);
            });
        }
        Err(Error::Cancelled) | Err(Error::Paused) => {
            if h.reason.load(Ordering::SeqCst) == STOP_CANCEL {
                if let Some(t) = inner.task(id) {
                    Inner::discard_partial(&t);
                }
                inner.update(id, |t| {
                    t.state = TaskState::Cancelled;
                    t.progress.speed = 0;
                });
            } else {
                inner.update(id, |t| {
                    t.state = TaskState::Paused;
                    t.progress.speed = 0;
                });
            }
        }
        Err(e) => {
            let s = inner.settings();
            let retries = inner.task(id).map(|t| t.retries).unwrap_or(0);
            let msg = e.to_string();
            if retriable(&e) && retries < s.retry_max {
                let wait = (s.retry_base_secs as i64 * (1i64 << retries.min(10))).min(600);
                inner.log(Some(id), format!("Échec : {msg}. Nouvelle tentative dans {wait} s."));
                inner.update(id, |t| {
                    t.state = TaskState::Scheduled;
                    t.scheduled_at = Some(now() + wait);
                    t.retries += 1;
                    t.error = Some(format!("Nouvelle tentative dans {wait} s : {msg}"));
                    t.progress.speed = 0;
                });
            } else {
                inner.update(id, |t| {
                    t.state = TaskState::Failed;
                    t.error = Some(msg.clone());
                    t.progress.speed = 0;
                });
                if s.notifications {
                    inner.emit(Event::Notification { title: "Échec du téléchargement".into(), body: msg });
                }
            }
        }
    }
    if remove {
        inner.remove_entry(id);
    }
}

async fn pipeline(inner: &Arc<Inner>, id: i64, h: &RunHandle) -> Result<Outcome> {
    let task = inner.task(id).ok_or(Error::NoTask(id))?;
    let s = inner.settings();
    inner.update(id, |t| {
        t.state = TaskState::Extracting;
        t.error = None;
        t.question = None;
        t.progress.speed = 0;
        t.progress.stage = "Analyse".into();
    });
    let ex = inner.extractor();
    let resolved = tokio::select! {
        r = ex.resolve(&task.url, task.options.referer.as_deref()) => r?,
        _ = h.cancel.cancelled() => return Err(Error::Cancelled),
    };
    match resolved {
        Resolved::Single(info) => process(inner, id, h, task, *info, s).await,
        Resolved::Playlist { title, entries } => multi(inner, &task, &s, title, entries, true),
        Resolved::Links(links) => multi(inner, &task, &s, "Médias détectés".into(), links, false),
        Resolved::Unsupported(msg) => {
            if s.unsupported_link == UnsupportedLink::Guided {
                inner.emit(Event::GuidedExtraction { id, url: task.url.clone() });
            }
            inner.log(Some(id), format!("Lien ignoré : {msg}"));
            Ok(Outcome::Skipped(format!("Lien non pris en charge : {msg}")))
        }
    }
}

fn multi(inner: &Arc<Inner>, task: &Task, s: &Settings, title: String, links: Vec<LinkCandidate>, as_playlist: bool) -> Result<Outcome> {
    if links.len() == 1 && as_playlist || s.automated_mode {
        let count = links.len() as u32;
        for (i, l) in links.iter().enumerate() {
            let mut o = task.options.clone();
            o.format_id = None;
            o.title_hint = l.title.clone();
            o.referer = if l.recognized { task.options.referer.clone() } else { Some(task.url.clone()) };
            if as_playlist {
                o.playlist = Some(PlaylistRef { title: title.clone(), index: i as u32 + 1, count });
            }
            let child = inner.new_task(&l.url, o, task.priority, None, None);
            inner.insert_task(child)?;
        }
        return Ok(Outcome::Replaced);
    }
    Ok(Outcome::Question(Question::Links { title, links }))
}

/// Réglages effectifs d'un téléchargement : options propres, puis réglages globaux.
struct Eff {
    max_height: Option<u32>,
    container: Container,
    audio_target: Option<AudioFormat>,
    post: PostMode,
    sub_mode: SubMode,
    sub_langs: Vec<String>,
    template: String,
}

impl Eff {
    fn new(o: &TaskOptions, s: &Settings) -> Eff {
        let mut post = if o.post_mode == PostMode::Default { s.post_mode } else { o.post_mode };
        if post == PostMode::Default {
            post = PostMode::Mp4;
        }
        let audio_target = o.audio_format.or(if post == PostMode::AudioOnly { Some(s.audio_format.unwrap_or(AudioFormat::M4a)) } else { s.audio_format });
        let container = match post {
            PostMode::Mp4 => Container::ForceMp4,
            PostMode::None => Container::Original,
            _ => s.container,
        };
        Eff {
            max_height: o.max_resolution.or(s.max_resolution),
            container,
            audio_target,
            post,
            sub_mode: o.sub_mode.unwrap_or(s.sub_mode),
            sub_langs: o.sub_langs.clone().unwrap_or_else(|| s.sub_langs.clone()),
            template: o.name_template.clone().unwrap_or_else(|| s.name_template.clone()),
        }
    }
}

fn mp4ish(f: &Format) -> bool {
    matches!(f.ext.as_str(), "mp4" | "m4a" | "m4v" | "mov")
        || f.vcodec.as_deref().is_some_and(|c| c.starts_with("avc") || c.starts_with("h264") || c.starts_with("hev") || c.starts_with("hvc"))
        || f.acodec.as_deref().is_some_and(|c| c.starts_with("mp4a"))
}

fn final_ext(sel: &Selection, audio_target: Option<AudioFormat>, container: Container, live_ts: bool) -> String {
    if let Some(t) = audio_target {
        return match t {
            AudioFormat::M4a => "m4a".into(),
            AudioFormat::Mp3 => "mp3".into(),
            AudioFormat::Original => {
                let e = sel.single.as_ref().map(|f| f.ext.as_str()).unwrap_or("m4a");
                if e == "mp4" { "m4a".into() } else { e.to_string() }
            }
        };
    }
    if live_ts {
        return "ts".into();
    }
    match (&sel.video, &sel.audio, &sel.single) {
        (Some(v), Some(a), _) => match container {
            Container::ForceMp4 => "mp4".into(),
            Container::PreferMp4 => if mp4ish(v) && mp4ish(a) { "mp4".into() } else { "mkv".into() },
            Container::Original => {
                if v.ext == "webm" && a.ext == "webm" {
                    "webm".into()
                } else if mp4ish(v) && mp4ish(a) {
                    "mp4".into()
                } else {
                    "mkv".into()
                }
            }
        },
        (Some(v), None, _) => if container == Container::Original { v.ext.clone() } else { "mp4".into() },
        (_, _, Some(f)) => {
            if !f.has_video() {
                return if f.ext == "mp4" { "m4a".into() } else { f.ext.clone() };
            }
            match container {
                Container::ForceMp4 => "mp4".into(),
                Container::PreferMp4 => if mp4ish(f) || f.ext == "ts" { "mp4".into() } else { f.ext.clone() },
                Container::Original => f.ext.clone(),
            }
        }
        _ => "mp4".into(),
    }
}

fn expand_home(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => crate::util::home_dir().join(rest),
        None => PathBuf::from(p),
    }
}

fn last_segment(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.path_segments().and_then(|mut s| s.next_back().map(String::from))).unwrap_or_default()
}

fn name_vars(info: &MediaInfo, task: &Task, title: &str, ext: &str, height: Option<u32>, pl: &Option<PlaylistRef>) -> BTreeMap<String, String> {
    let last = last_segment(&info.url);
    let last_dec = crate::extract::hls::percent_decode(&last);
    let orig = last_dec.rsplit_once('.').map(|x| x.0.to_string()).unwrap_or(last_dec.clone());
    let mut v = BTreeMap::new();
    let mut put = |k: &str, val: String| {
        v.insert(k.to_string(), val);
    };
    put("title", title.to_string());
    put("id", info.id.clone());
    put("site", info.site.clone());
    put("host", info.host.clone());
    put("uploader", info.uploader.clone().unwrap_or_default());
    put("upload_date", info.upload_date.clone().unwrap_or_default());
    put("timestamp", info.timestamp.map(|t| t.to_string()).unwrap_or_default());
    put("playlist", pl.as_ref().map(|p| p.title.clone()).unwrap_or_default());
    put("playlist_index", pl.as_ref().map(|p| p.index.to_string()).unwrap_or_default());
    put("playlist_count", pl.as_ref().map(|p| p.count.to_string()).unwrap_or_default());
    put("resolution", height.map(|h| format!("{h}p")).unwrap_or_default());
    put("ext", ext.to_string());
    put("original_name", orig);
    put("last_path", last_dec);
    put("download_id", task.id.to_string());
    v
}

#[derive(Default)]
struct Stage {
    label: String,
    base: f64,
    span: f64,
    /// Flux déjà terminés, flux en cours (estimé) et flux restants, en octets : la vidéo et l'audio sont souvent séparés.
    before: u64,
    current: u64,
    after: u64,
}

struct ReporterGuard(tokio::task::JoinHandle<()>);
impl Drop for ReporterGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn spawn_reporter(inner: Arc<Inner>, id: i64, meter: Arc<Meter>, stage: Arc<Mutex<Stage>>) -> ReporterGuard {
    ReporterGuard(tokio::spawn(async move {
        let mut prev = (0u64, std::time::Instant::now());
        let mut speed = 0.0f64;
        let mut first = true;
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let (done, total) = meter.get();
            let (label, base, span, before, current, after) = {
                let g = stage.lock().unwrap();
                (g.label.clone(), g.base, g.span, g.before, g.current, g.after)
            };
            let shown = before + done;
            // Après une reprise, le premier relevé contient tout ce qui était déjà là : ce n'est pas du débit.
            if !first {
                let dt = prev.1.elapsed().as_secs_f64().max(0.001);
                let inst = shown.saturating_sub(prev.0) as f64 / dt;
                speed = if speed == 0.0 { inst } else { speed * 0.6 + inst * 0.4 };
            }
            first = false;
            prev = (shown, std::time::Instant::now());
            let shown_total = total.or((current > 0).then_some(current)).map(|t| before + t.max(done) + after);
            let percent = match shown_total {
                Some(t) => shown as f64 / t as f64,
                None => base + span * total.map(|t| (done as f64 / t as f64).min(1.0)).unwrap_or(0.0),
            };
            let p = Progress {
                downloaded: shown,
                total: shown_total,
                percent: ((percent * 100.0) as f32).min(100.0),
                speed: speed as u64,
                eta_secs: shown_total.filter(|_| speed > 1.0).map(|t| (t.saturating_sub(shown) as f64 / speed) as u64),
                stage: label,
            };
            {
                let mut st = inner.state.lock().unwrap();
                if let Some(e) = st.tasks.get_mut(&id) {
                    e.task.progress = p.clone();
                }
            }
            inner.emit(Event::TaskProgress { id, progress: p });
        }
    }))
}

/// Taille annoncée, sinon déduite du débit et de la durée.
fn estimated_size(f: &Format, info: &MediaInfo) -> u64 {
    f.filesize.or_else(|| Some((f.tbr? * 125.0 * info.duration?) as u64)).unwrap_or(0)
}

fn headers_for(f: &Format, info: &MediaInfo) -> BTreeMap<String, String> {
    let mut h = info.headers.clone();
    h.extend(f.headers.clone());
    if let Some(r) = &info.referer {
        h.entry("Referer".into()).or_insert_with(|| r.clone());
    }
    h
}

async fn fetch_stream(ctx: &Ctx, part: &Arc<PartDir>, key: &str, f: &Format, info: &MediaInfo) -> Result<PathBuf> {
    let headers = headers_for(f, info);
    match f.protocol {
        Protocol::Http => http::download(ctx, part, key, &f.url, &headers).await,
        Protocol::Hls => segments::download_hls(ctx, part, key, &f.url, &headers).await,
        Protocol::Segments => segments::download(ctx, part, key, f.plan.as_ref().ok_or_else(|| Error::Extract("liste de segments absente".into()))?, &headers).await,
        Protocol::Ytdlp => Err(Error::Unsupported("protocole géré uniquement par yt-dlp".into())),
    }
}

#[derive(Default)]
struct Produced {
    video: Option<PathBuf>,
    audio: Option<PathBuf>,
    single: Option<PathBuf>,
    single_ext: String,
}

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

async fn download_native(ctx: &Ctx, part: &Arc<PartDir>, sel: &Selection, info: &MediaInfo, stage: &Arc<Mutex<Stage>>) -> Result<Produced> {
    let streams = sel.streams();
    let weights: Vec<f64> = streams.iter().map(|(_, f)| f.filesize.unwrap_or(1_000_000) as f64).collect();
    let total: f64 = weights.iter().sum();
    let sizes: Vec<u64> = streams.iter().map(|(_, f)| estimated_size(f, info)).collect();
    let mut acc = 0.0;
    let mut before = 0u64;
    let mut out = Produced::default();
    for (i, ((key, f), w)) in streams.iter().zip(&weights).enumerate() {
        {
            let mut g = stage.lock().unwrap();
            g.label = match *key {
                "video" => "Vidéo".into(),
                "audio" => "Audio".into(),
                _ => "Téléchargement".into(),
            };
            g.base = acc / total;
            g.span = w / total;
            g.before = before;
            g.current = sizes[i];
            g.after = sizes[i + 1..].iter().sum();
        }
        ctx.meter.done.store(0, Ordering::Relaxed);
        ctx.meter.total.store(0, Ordering::Relaxed);
        let p = fetch_stream(ctx, part, key, f, info).await.map_err(|e| match e {
            Error::Network(m) => Error::Network(format!("flux {key} (format {}) : {m}", f.id)),
            other => other,
        })?;
        acc += w;
        before += ctx.meter.done.load(Ordering::Relaxed);
        match *key {
            "video" => out.video = Some(p),
            "audio" => out.audio = Some(p),
            _ => {
                out.single = Some(p);
                out.single_ext = f.ext.clone();
            }
        }
    }
    Ok(out)
}

/// Repli : yt-dlp, puis ffmpeg.
#[allow(clippy::too_many_arguments)]
async fn download_fallback(ctx: &Ctx, s: &Settings, data_dir: &Path, part: &Arc<PartDir>, sel: &Selection, info: &MediaInfo, eff: &Eff, stage: &Arc<Mutex<Stage>>) -> Result<Produced> {
    {
        let mut g = stage.lock().unwrap();
        g.label = "Téléchargement (repli)".into();
        g.base = 0.0;
        g.span = 1.0;
        g.before = 0;
        g.current = 0;
        g.after = 0;
    }
    ctx.meter.done.store(0, Ordering::Relaxed);
    ctx.meter.total.store(0, Ordering::Relaxed);
    let selector = match (&sel.video, &sel.audio, &sel.single) {
        (Some(v), Some(a), _) => format!("{}+{}", v.id, a.id),
        (_, _, Some(f)) if f.protocol == Protocol::Ytdlp || !info.site.is_empty() && !matches!(info.site.as_str(), "HLS" | "DASH" | "Fichier direct") => f.id.clone(),
        _ => ytdlp_dl::selector(eff.max_height, eff.audio_target.is_some()),
    };
    let merge_ext = if eff.container == Container::Original { "mkv" } else { "mp4" };
    let from_ytdlp = ytdlp_dl::download(ctx, s, data_dir, part, &info.url, &selector, merge_ext).await;
    match from_ytdlp {
        Ok(p) => {
            let e = ext_of(&p);
            return Ok(Produced { single: Some(p), single_ext: e, ..Default::default() });
        }
        Err(e @ (Error::Cancelled | Error::Paused | Error::Drm | Error::DiskFull(_))) => return Err(e),
        Err(e) => tracing::warn!("repli yt-dlp impossible : {e}"),
    }
    // ffmpeg : seulement pour les flux HLS/DASH, qu'il sait lire seul.
    let f = sel.single.as_ref().or(sel.video.as_ref()).ok_or_else(|| Error::Other("aucun flux à récupérer".into()))?;
    let input = if matches!(info.site.as_str(), "HLS" | "DASH") { info.url.clone() } else { f.url.clone() };
    if !matches!(f.protocol, Protocol::Hls | Protocol::Segments | Protocol::Http) {
        return Err(Error::Other("aucun téléchargeur de repli disponible pour ce format".into()));
    }
    let ffmpeg = tools::require("ffmpeg", s, data_dir)?;
    let out = part.path("ffmpeg.mkv");
    let opts = live::RecordOpts { stop_at: None, min_free_bytes: 0, user_agent: ctx.net.user_agent.clone() };
    live::record(ctx, &ffmpeg, &input, &headers_for(f, info), &out, &opts).await?;
    Ok(Produced { single: Some(out), single_ext: "mkv".into(), ..Default::default() })
}

async fn fetch_sub(ctx_net: &crate::net::Net, t: &SubtitleTrack, headers: &BTreeMap<String, String>) -> Result<Vec<subs::Cue>> {
    let max = crate::util::MAX_MANIFEST_BYTES;
    if t.ext == "m3u8" {
        let list = ctx_net.fetch_text(&t.url, headers, max).await?;
        let plan = crate::extract::hls::build_plan(&t.url, &list)?;
        let mut all = String::from("WEBVTT\n\n");
        for seg in plan.segments.iter().take(5000) {
            let txt = ctx_net.fetch_text(&seg.url, headers, max).await?;
            all.push_str(txt.trim_start_matches('\u{feff}').trim_start_matches("WEBVTT").split_once("\n\n").map(|x| x.1).unwrap_or(""));
            all.push_str("\n\n");
        }
        return subs::parse(&all);
    }
    subs::parse(&ctx_net.fetch_text(&t.url, headers, max).await?)
}

struct SubFile {
    path: PathBuf,
    lang: String,
    ext: &'static str,
}

async fn prepare_subs(inner: &Arc<Inner>, id: i64, net: &crate::net::Net, info: &MediaInfo, s: &Settings, eff: &Eff, part: &PartDir) -> Result<Vec<SubFile>> {
    if eff.sub_mode == SubMode::None {
        return Ok(vec![]);
    }
    let chosen = subs::select(&info.subtitles, &eff.sub_langs, s.sub_label_regex.as_deref(), s.sub_auto, s.sub_multiple);
    if chosen.is_empty() {
        if eff.sub_mode == SubMode::Required || eff.sub_mode == SubMode::Only {
            return Err(Error::Other("aucun sous-titre disponible dans les langues demandées".into()));
        }
        return Ok(vec![]);
    }
    let (ext, is_vtt): (&'static str, bool) = if s.sub_out == SubOut::Vtt { ("vtt", true) } else { ("srt", false) };
    let mut out = Vec::new();
    for t in chosen {
        let r = fetch_sub(net, &t, &info.headers).await;
        match r {
            Ok(cues) => {
                let path = part.path(&format!("sub-{}.{ext}", naming::sanitize_component(&t.lang)));
                let text = if is_vtt { subs::to_vtt(&cues) } else { subs::to_srt(&cues) };
                std::fs::write(&path, text)?;
                out.push(SubFile { path, lang: t.lang.clone(), ext });
            }
            Err(e) => {
                if eff.sub_mode == SubMode::Required || eff.sub_mode == SubMode::Only {
                    return Err(Error::Other(format!("sous-titres « {} » indisponibles : {e}", t.lang)));
                }
                inner.log(Some(id), format!("Sous-titres « {} » ignorés : {e}", t.lang));
            }
        }
    }
    Ok(out)
}

fn date_iso(d: &str) -> String {
    if d.len() == 8 && d.chars().all(|c| c.is_ascii_digit()) { format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]) } else { d.to_string() }
}

#[allow(clippy::too_many_arguments)]
async fn process(inner: &Arc<Inner>, id: i64, h: &RunHandle, task: Task, mut info: MediaInfo, s: Settings) -> Result<Outcome> {
    let title = if info.title.trim().is_empty() { task.options.title_hint.clone().unwrap_or_else(|| info.id.clone()) } else { info.title.clone() };
    let pl = task.options.playlist.clone().or(info.playlist.clone());
    inner.update(id, |t| {
        t.title = Some(title.clone());
        t.site = Some(info.site.clone());
        t.thumbnail = info.thumbnail.clone();
        t.playlist = pl.as_ref().map(|p| p.title.clone());
    });
    let automated = s.automated_mode;
    let mut eff = Eff::new(&task.options, &s);

    // Doublons
    let policy = task.options.duplicate.unwrap_or(s.duplicate_policy);
    if policy != DuplicatePolicy::Redownload {
        let mut dup = inner.db.history_find_duplicate(&task.url, &info.site, Some(&info.id).filter(|i| !i.is_empty()).map(String::as_str))?;
        if dup.is_none() && info.url != task.url {
            dup = inner.db.history_find_duplicate(&info.url, &info.site, None)?;
        }
        if let Some(prev) = dup.filter(|p| p.exists) {
            return Ok(match policy {
                DuplicatePolicy::Ask if !automated => Outcome::Question(Question::Duplicate { previous: prev }),
                _ => Outcome::Skipped("Ignoré : déjà téléchargé".into()),
            });
        }
    }

    // Qualité
    let prefs = Prefs {
        max_height: eff.max_height,
        prefer_mp4: eff.container != Container::Original,
        audio_only: eff.audio_target.is_some(),
        languages: s.audio_languages.clone(),
        format_id: task.options.format_id.clone(),
    };
    if info.is_file {
        // Un fichier s'enregistre tel quel, quels que soient les réglages prévus pour la vidéo.
        eff.post = PostMode::None;
        eff.audio_target = None;
        eff.container = Container::Original;
        eff.sub_mode = SubMode::None;
    }
    if s.quality_prompt && !automated && !info.is_file && task.options.format_id.is_none() && !info.is_live && eff.audio_target.is_none() && eff.sub_mode != SubMode::Only {
        let c = quality::choices(&info);
        if c.len() > 1 {
            return Ok(Outcome::Question(Question::Quality { choices: c }));
        }
    }
    // Sous-titres : question seulement si l'utilisateur n'a exprimé aucune préférence de langue.
    if eff.sub_mode != SubMode::None && !info.subtitles.is_empty() && task.options.sub_langs.is_none() && s.sub_langs.is_empty() && !automated {
        return Ok(Outcome::Question(Question::Subtitles { tracks: info.subtitles.clone() }));
    }
    if s.ask_destination && task.options.dest_dir.is_none() && !automated {
        return Ok(Outcome::Question(Question::Destination { suggested: s.download_dir.clone() }));
    }

    let dest_dir = expand_home(task.options.dest_dir.as_deref().unwrap_or(&s.download_dir));
    std::fs::create_dir_all(&dest_dir)?;
    let net = inner.net.read().unwrap().clone();

    // Sous-titres seuls : pas de média.
    if eff.sub_mode == SubMode::Only {
        let tmp = PartDir::open(&naming::unique_path(&dest_dir, Path::new(&naming::sanitize_component(&title)), "tgtmp"), &title)?;
        let files = prepare_subs(inner, id, &net, &info, &s, &eff, &tmp).await?;
        let mut first = None;
        for f in &files {
            let stem = naming::render(&eff.template, &name_vars(&info, &task, &title, f.ext, None, &pl));
            let dest = naming::unique_path(&dest_dir, &PathBuf::from(format!("{}.{}", stem.display(), f.lang)), f.ext);
            naming::ensure_inside(&dest_dir, &dest)?;
            download::finalize_move(&f.path, &dest)?;
            first.get_or_insert(dest);
        }
        let _ = std::fs::remove_dir_all(&tmp.dir);
        let path = first.ok_or_else(|| Error::Other("aucun sous-titre enregistré".into()))?;
        complete(inner, id, &task, &info, &title, &s, &eff, &path, &pl).await;
        return Ok(Outcome::Done);
    }

    // Le refus du DRM se fait format par format : on écarte les formats protégés et on choisit parmi le reste.
    info.formats.retain(|f| !f.has_drm);
    if info.formats.is_empty() {
        return Err(Error::Drm);
    }
    let sel = loop {
        let sel = quality::select(&info, &prefs)?;
        let mut bad = Vec::new();
        for (_, f) in sel.streams() {
            if f.protocol == Protocol::Hls && !info.is_live {
                let text = net.fetch_text(&f.url, &headers_for(f, &info), crate::util::MAX_MANIFEST_BYTES).await;
                if let Ok(t) = text
                    && matches!(crate::extract::hls::build_plan(&f.url, &t), Err(Error::Drm)) {
                        bad.push(f.id.clone());
                    }
            }
        }
        if bad.is_empty() {
            break sel;
        }
        info.formats.retain(|f| !bad.contains(&f.id));
        if info.formats.is_empty() {
            return Err(Error::Drm);
        }
    };
    let live_mode = info.is_live;
    let ext = if info.is_file { sel.single.as_ref().map(|f| f.ext.clone()).unwrap_or_default() } else { final_ext(&sel, eff.audio_target, eff.container, live_mode && s.live_mpegts) };
    let height = sel.video.as_ref().or(sel.single.as_ref()).and_then(|f| f.height);

    // Destination : on garde la même pour reprendre au bon endroit.
    let vars = name_vars(&info, &task, &title, &ext, height, &pl);
    let has_ext_var = eff.template.contains("{ext}");
    let dest = match task.target.as_deref().map(PathBuf::from).filter(|p| naming::part_dir(p).exists()) {
        Some(p) => p,
        // Un fichier garde son vrai nom, sans passer par le modèle prévu pour les vidéos.
        None if info.is_file => naming::unique_path(&dest_dir, Path::new(&naming::sanitize_component(&title)), &ext),
        None => naming::unique_path(&dest_dir, &naming::render(&eff.template, &vars), if has_ext_var { "" } else { &ext }),
    };
    naming::ensure_inside(&dest_dir, &dest)?;
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    let is_file = info.is_file;
    inner.update(id, |t| {
        t.target = Some(dest.display().to_string());
        t.is_file = is_file;
    });

    // Espace disque
    let expected: u64 = sel.streams().iter().map(|(_, f)| estimated_size(f, &info)).sum();
    let needed = if info.is_file { expected } else { post::space_needed(Some(expected)) } + s.min_free_mb * 1_048_576;
    if let Some(free) = crate::util::free_space(&dest_dir)
        && free < needed {
            return Err(Error::DiskFull(format!("{} disponibles, {} nécessaires", crate::util::fmt_size(free), crate::util::fmt_size(needed))));
        }

    let part = Arc::new(PartDir::open(&dest, &title)?);
    let meter = Arc::new(Meter::default());
    let ctx = Ctx { net: net.clone(), meter: meter.clone(), throttle: h.throttle.clone(), cancel: h.cancel.clone(), connections: if s.fragmented { s.connections as usize } else { 1 } };
    let stage = Arc::new(Mutex::new(Stage { label: "Téléchargement".into(), span: 1.0, ..Default::default() }));
    let rep = spawn_reporter(inner.clone(), id, meter, stage.clone());

    let produced: Produced;
    if live_mode {
        inner.update(id, |t| t.state = TaskState::Recording);
        stage.lock().unwrap().label = "Enregistrement".into();
        let ffmpeg = tools::require("ffmpeg", &s, &inner.data_dir)?;
        let f = sel.single.as_ref().or(sel.video.as_ref()).ok_or_else(|| Error::Extract("aucun flux à enregistrer".into()))?;
        let input = if matches!(info.site.as_str(), "HLS" | "DASH") { info.url.clone() } else { f.url.clone() };
        let rec = part.path("rec.ts");
        let opts = live::RecordOpts { stop_at: task.stop_at, min_free_bytes: s.live_min_free_mb * 1_048_576, user_agent: net.user_agent.clone() };
        let r = live::record(&ctx, &ffmpeg, &input, &headers_for(f, &info), &rec, &opts).await?;
        inner.log(Some(id), format!("Enregistrement terminé : {}", r.reason));
        if h.reason.load(Ordering::SeqCst) == STOP_CANCEL {
            return Err(Error::Cancelled);
        }
        produced = Produced { single: Some(r.path), single_ext: "ts".into(), ..Default::default() };
    } else {
        inner.update(id, |t| {
            t.state = TaskState::Downloading;
            if expected > 0 {
                t.progress.total = Some(expected);
            }
        });
        produced = match download_native(&ctx, &part, &sel, &info, &stage).await {
            Ok(p) => p,
            Err(e @ (Error::Cancelled | Error::Paused | Error::Drm | Error::DiskFull(_))) => return Err(e),
            Err(e) if info.is_file => return Err(e),
            Err(e) => {
                inner.log(Some(id), format!("Téléchargeur natif en échec ({e}), essai du repli."));
                download_fallback(&ctx, &s, &inner.data_dir, &part, &sel, &info, &eff, &stage).await.map_err(|e2| if matches!(e2, Error::ToolMissing(_)) { e } else { e2 })?
            }
        };
    }

    // Post-traitement
    drop(rep);
    inner.update(id, |t| {
        t.state = TaskState::PostProcessing;
        t.progress.stage = "Traitement".into();
        t.progress.speed = 0;
        t.progress.eta_secs = None;
    });
    let sub_files = prepare_subs(inner, id, &net, &info, &s, &eff, &part).await?;
    let embed_subs = s.sub_out == SubOut::Embed && eff.audio_target.is_none() && matches!(ext.as_str(), "mp4" | "mkv" | "m4v");
    let want_thumb = (s.embed_thumbnail || s.save_thumbnail) && info.thumbnail.is_some();
    let mut thumb: Option<PathBuf> = None;
    if want_thumb {
        let ffmpeg = tools::require("ffmpeg", &s, &inner.data_dir);
        let r: Result<PathBuf> = async {
            let raw = net.fetch_bytes(info.thumbnail.as_deref().unwrap(), &BTreeMap::new(), 10 * 1024 * 1024).await?;
            let src = part.path("thumb.src");
            std::fs::write(&src, raw)?;
            let jpg = part.path("thumb.jpg");
            post::to_jpeg(&ffmpeg?, &src, &jpg).await?;
            Ok(jpg)
        }
        .await;
        match r {
            Ok(p) => thumb = Some(p),
            Err(e) => inner.log(Some(id), format!("Miniature ignorée : {e}")),
        }
    }
    let embed_thumb = s.embed_thumbnail && matches!(ext.as_str(), "mp4" | "m4a" | "mp3" | "m4v");
    let mut metadata = Vec::new();
    if s.embed_metadata && !info.is_file {
        metadata.push(("title".to_string(), title.clone()));
        if let Some(u) = &info.uploader {
            metadata.push(("artist".into(), u.clone()));
        }
        if let Some(d) = &info.upload_date {
            metadata.push(("date".into(), date_iso(d)));
        }
        if !matches!(info.site.as_str(), "HLS" | "DASH" | "Fichier direct") {
            metadata.push(("comment".into(), info.url.clone()));
        }
        if matches!(ext.as_str(), "mp4" | "m4v" | "m4a") {
            metadata.push(("media_type".into(), if eff.audio_target.is_some() { "1" } else { "9" }.into()));
        }
    }
    let work_out = part.path(&format!("final.{ext}"));
    let mut compat = post::Compat::default();
    if !live_mode && !s.reencode && !info.is_file && eff.audio_target.is_none() && matches!(ext.as_str(), "mp4" | "m4v" | "mov") {
        let ffprobe = tools::require("ffprobe", &s, &inner.data_dir)?;
        let video = produced.video.as_ref().or(produced.single.as_ref());
        let audio = produced.audio.as_ref().or(produced.single.as_ref());
        compat = post::compat_for(&ffprobe, video.map(|p| p.as_path()), audio.map(|p| p.as_path())).await;
        if compat.video.is_some() || compat.audio {
            inner.log(Some(id), "Conversion pour que le fichier s'ouvre dans QuickTime".to_string());
            inner.update(id, |t| t.progress.stage = "Conversion pour le Mac".into());
        }
    }
    let job = post::FinalizeJob {
        video: produced.video.clone(),
        audio: produced.audio.clone(),
        single: produced.single.clone(),
        single_ext: produced.single_ext.clone(),
        subs: if embed_subs { sub_files.iter().map(|f| (f.path.clone(), f.lang.clone())).collect() } else { vec![] },
        thumb: if embed_thumb { thumb.clone() } else { None },
        out: work_out.clone(),
        audio_target: eff.audio_target,
        source_acodec: sel.single.as_ref().or(sel.audio.as_ref()).and_then(|f| f.acodec.clone()),
        reencode: s.reencode && !info.is_file,
        compat,
        metadata,
    };
    let converted = post::needs_ffmpeg(&job);
    if converted {
        let ffmpeg = tools::require("ffmpeg", &s, &inner.data_dir)?;
        if live_mode {
            // Après l'arrêt d'un direct, le jeton est déjà annulé : on termine quand même.
            post::finalize(&ffmpeg, &job).await?;
        } else {
            tokio::select! {
                r = post::finalize(&ffmpeg, &job) => r?,
                _ = h.cancel.cancelled() => return Err(Error::Cancelled),
            }
        }
    } else {
        std::fs::rename(job.single.as_ref().unwrap(), &work_out)?;
    }
    if !work_out.exists() {
        return Err(Error::Other("le fichier final n'a pas été produit".into()));
    }
    if converted && (s.keep_original || (s.keep_source_on_convert && (job.compat.video.is_some() || job.compat.audio))) {
        let ffmpeg = tools::require("ffmpeg", &s, &inner.data_dir)?;
        let original = part.path("original.mkv");
        let stem = dest.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
        match post::mux_original(&ffmpeg, &job, &original).await {
            Ok(()) => download::finalize_move(&original, &dest.with_file_name(format!("{stem} (original).mkv")))?,
            Err(e) => inner.log(Some(id), format!("Fichier d'origine non conservé : {e}")),
        }
    }
    download::finalize_move(&work_out, &dest)?;

    // Fichiers voisins : sous-titres, miniature, JSON.
    let stem = dest.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
    for f in &sub_files {
        if !embed_subs {
            let _ = download::finalize_move(&f.path, &dest.with_file_name(format!("{stem}.{}.{}", naming::sanitize_component(&f.lang), f.ext)));
        }
    }
    if s.save_thumbnail
        && let Some(t) = &thumb {
            let _ = std::fs::copy(t, dest.with_file_name(format!("{stem}.jpg")));
        }
    if s.save_json {
        let j = serde_json::json!({"title": title, "id": info.id, "site": info.site, "uploader": info.uploader, "upload_date": info.upload_date,
            "duration": info.duration, "description": info.description, "thumbnail": info.thumbnail, "webpage_url": info.url});
        let _ = std::fs::write(dest.with_file_name(format!("{stem}.info.json")), serde_json::to_vec_pretty(&j)?);
    }
    let _ = std::fs::remove_dir_all(&part.dir);

    // Actions de fin
    match eff.post {
        PostMode::SendToApp => {
            if let Some(app) = s.send_to_app.as_deref().filter(|a| !a.is_empty())
                && let Err(e) = post::open_with_app(app, &dest).await {
                    inner.log(Some(id), e.to_string());
                }
        }
        PostMode::Custom if s.script_when == ScriptWhen::PerFile => {
            if let Some(script) = task.options.custom_script.clone().or(s.script.clone()).filter(|x| !x.trim().is_empty()) {
                let env = [
                    ("TG_FILE", dest.display().to_string()),
                    ("TG_TITLE", title.clone()),
                    ("TG_URL", info.url.clone()),
                    ("TG_SITE", info.site.clone()),
                    ("TG_PLAYLIST", pl.as_ref().map(|p| p.title.clone()).unwrap_or_default()),
                ];
                if let Err(e) = post::run_script(&script, &env).await {
                    inner.log(Some(id), e.to_string());
                }
            }
        }
        _ => {}
    }
    if s.add_to_music
        && !info.is_file
        && let Err(e) = post::add_to_music(&dest, false).await {
            inner.log(Some(id), e.to_string());
        }
    complete(inner, id, &task, &info, &title, &s, &eff, &dest, &pl).await;
    Ok(Outcome::Done)
}

#[allow(clippy::too_many_arguments)]
async fn complete(inner: &Arc<Inner>, id: i64, task: &Task, info: &MediaInfo, title: &str, s: &Settings, _eff: &Eff, dest: &Path, pl: &Option<PlaylistRef>) {
    let size = std::fs::metadata(dest).map(|m| m.len()).ok();
    let item = HistoryItem {
        url: task.url.clone(),
        media_id: Some(info.id.clone()).filter(|i| !i.is_empty()),
        title: title.to_string(),
        site: info.site.clone(),
        path: Some(dest.display().to_string()),
        size,
        thumbnail: info.thumbnail.clone(),
        finished_at: now(),
        ..Default::default()
    };
    if let Err(e) = inner.db.history_add(&item) {
        inner.log(Some(id), format!("Historique : {e}"));
    }
    inner.update(id, |t| {
        t.state = TaskState::Finished;
        t.output_path = Some(dest.display().to_string());
        t.progress.percent = 100.0;
        t.progress.speed = 0;
        t.progress.eta_secs = Some(0);
        t.progress.stage = "Terminé".into();
        if let Some(sz) = size {
            t.progress.downloaded = sz;
            t.progress.total = Some(sz);
        }
    });
    inner.state.lock().unwrap().finished_files.push((dest.display().to_string(), title.to_string(), task.url.clone()));
    if s.notifications {
        inner.emit(Event::Notification { title: "Téléchargement terminé".into(), body: title.to_string() });
    }
    let _ = pl;
}
