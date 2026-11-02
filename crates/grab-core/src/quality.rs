//! Choix des formats : résolution maximale, préférence MP4, audio seul, langue de la piste audio.
use crate::error::{Error, Result};
use crate::model::*;

#[derive(Debug, Clone, Default)]
pub struct Prefs {
    pub max_height: Option<u32>,
    pub prefer_mp4: bool,
    pub audio_only: bool,
    pub languages: Vec<String>,
    pub format_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub video: Option<Format>,
    pub audio: Option<Format>,
    /// Flux déjà complet (vidéo + audio) ou audio seul.
    pub single: Option<Format>,
}

impl Selection {
    pub fn streams(&self) -> Vec<(&'static str, &Format)> {
        let mut v = Vec::new();
        if let Some(f) = &self.video {
            v.push(("video", f));
        }
        if let Some(f) = &self.audio {
            v.push(("audio", f));
        }
        if let Some(f) = &self.single {
            v.push(("main", f));
        }
        v
    }
}

fn is_mp4ish(f: &Format) -> bool {
    matches!(f.ext.as_str(), "mp4" | "m4a" | "m4v") || f.vcodec.as_deref().is_some_and(|c| c.starts_with("avc") || c.starts_with("h264")) || f.acodec.as_deref().is_some_and(|c| c.starts_with("mp4a"))
}

fn lang_rank(f: &Format, langs: &[String]) -> usize {
    let fl = f.language.as_deref().unwrap_or("").to_lowercase();
    if fl.is_empty() {
        return langs.len();
    }
    langs.iter().position(|l| fl == l.to_lowercase() || fl.starts_with(&format!("{}-", l.to_lowercase()))).unwrap_or(langs.len() + 1)
}

fn best_audio<'a>(info: &'a MediaInfo, p: &Prefs) -> Option<&'a Format> {
    // Le format natif évite une conversion, mais un petit AAC ne doit pas battre un Opus bien plus riche.
    let quality = |f: &Format| f.tbr.unwrap_or(0.0) * if p.prefer_mp4 && is_mp4ish(f) { 1.34 } else { 1.0 };
    info.formats
        .iter()
        .filter(|f| f.has_audio() && !f.has_video())
        .min_by(|a, b| {
            lang_rank(a, &p.languages).cmp(&lang_rank(b, &p.languages)).then(quality(b).partial_cmp(&quality(a)).unwrap_or(std::cmp::Ordering::Equal))
        })
}

fn apple_readable(f: &Format) -> bool {
    f.vcodec.as_deref().is_some_and(|c| ["avc", "h264", "hvc", "hev"].iter().any(|p| c.starts_with(p)))
}

/// Toujours la meilleure qualité : résolution, puis images par seconde, puis débit. VP9 et AV1 demandent une conversion
/// pour QuickTime, mais ne perdent la priorité que si leur débit n'est pas nettement supérieur.
fn rank_video(f: &Format, p: &Prefs) -> (u32, u32, u64, bool, bool) {
    let readable = p.prefer_mp4 && apple_readable(f);
    let tbr = f.tbr.unwrap_or(0.0) * if readable { 1.5 } else { 1.0 };
    (f.height.unwrap_or(0), f.fps.unwrap_or(0.0).round() as u32, tbr as u64, readable, p.prefer_mp4 && is_mp4ish(f))
}

pub fn select(info: &MediaInfo, p: &Prefs) -> Result<Selection> {
    if info.formats.is_empty() {
        return Err(Error::Extract("aucun format disponible".into()));
    }
    if info.is_file {
        return Ok(Selection { single: info.formats.first().cloned(), ..Default::default() });
    }
    if let Some(id) = &p.format_id {
        let find = |i: &str| info.formats.iter().find(|f| f.id == i);
        let parts: Vec<&str> = id.split('+').collect();
        let first = find(parts[0]).ok_or_else(|| Error::Extract(format!("le format « {id} » n'existe plus")))?;
        if parts.len() == 2 {
            let second = find(parts[1]).ok_or_else(|| Error::Extract(format!("le format « {id} » n'existe plus")))?;
            return Ok(Selection { video: Some(first.clone()), audio: Some(second.clone()), single: None });
        }
        if first.has_video() && !first.has_audio() {
            return Ok(Selection { video: Some(first.clone()), audio: best_audio(info, p).cloned(), single: None });
        }
        return Ok(Selection { single: Some(first.clone()), ..Default::default() });
    }
    if p.audio_only {
        if let Some(a) = best_audio(info, p) {
            return Ok(Selection { single: Some(a.clone()), ..Default::default() });
        }
        // Pas de flux audio seul : on prend le meilleur flux complet, l'audio sera extrait ensuite.
        let m = info.formats.iter().filter(|f| f.has_audio()).max_by_key(|f| (f.tbr.unwrap_or(0.0) as u64, f.height.unwrap_or(0)));
        return m.map(|f| Selection { single: Some(f.clone()), ..Default::default() }).ok_or_else(|| Error::Extract("aucune piste audio disponible".into()));
    }
    let videos: Vec<&Format> = info.formats.iter().filter(|f| f.has_video()).collect();
    if videos.is_empty() {
        // Source audio uniquement.
        return best_audio(info, p).map(|a| Selection { single: Some(a.clone()), ..Default::default() }).ok_or_else(|| Error::Extract("aucun flux vidéo ou audio".into()));
    }
    // Limite de résolution ; si rien ne la respecte, la plus petite qualité.
    let within: Vec<&Format> = match p.max_height {
        Some(h) => {
            let ok: Vec<&Format> = videos.iter().copied().filter(|f| f.height.is_none_or(|x| x <= h)).collect();
            if ok.is_empty() {
                let min = videos.iter().filter_map(|f| f.height).min();
                videos.iter().copied().filter(|f| f.height == min).collect()
            } else {
                ok
            }
        }
        None => videos,
    };
    let best = within.into_iter().max_by_key(|f| rank_video(f, p)).unwrap();
    if best.has_audio() {
        Ok(Selection { single: Some(best.clone()), ..Default::default() })
    } else {
        Ok(Selection { video: Some(best.clone()), audio: best_audio(info, p).cloned(), single: None })
    }
}

fn human_size(n: u64) -> String {
    crate::util::fmt_size(n)
}

/// Une ligne par résolution (le meilleur format de chaque), plus l'audio seul.
pub fn choices(info: &MediaInfo) -> Vec<FormatChoice> {
    let p = Prefs { prefer_mp4: true, ..Default::default() };
    let mut by_height: std::collections::BTreeMap<u32, &Format> = Default::default();
    for f in info.formats.iter().filter(|f| f.has_video()) {
        let h = f.height.unwrap_or(0);
        if by_height.get(&h).is_none_or(|cur| rank_video(f, &p) > rank_video(cur, &p)) {
            by_height.insert(h, f);
        }
    }
    let audio = best_audio(info, &p).map(|a| a.filesize.unwrap_or(0)).unwrap_or(0);
    let mut out: Vec<FormatChoice> = by_height
        .iter()
        .rev()
        .map(|(h, f)| {
            let size = f.filesize.map(|s| s + if f.has_audio() { 0 } else { audio });
            FormatChoice {
                id: f.id.clone(),
                label: format!("{}{} · {}{}", if *h > 0 { format!("{h}p") } else { "Vidéo".into() }, "", f.ext.to_uppercase(), size.map(|s| format!(" · {}", human_size(s))).unwrap_or_default()),
                height: f.height,
                ext: f.ext.clone(),
                size,
                audio_only: false,
            }
        })
        .collect();
    if let Some(a) = best_audio(info, &p) {
        out.push(FormatChoice { id: a.id.clone(), label: format!("Audio seul · {}", a.ext.to_uppercase()), height: None, ext: a.ext.clone(), size: a.filesize, audio_only: true });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(id: &str, ext: &str, v: &str, a: &str, h: Option<u32>, tbr: f64) -> Format {
        Format { id: id.into(), url: format!("https://x/{id}"), ext: ext.into(), vcodec: Some(v.into()), acodec: Some(a.into()), height: h, tbr: Some(tbr), ..Default::default() }
    }

    fn info() -> MediaInfo {
        MediaInfo {
            formats: vec![
                f("18", "mp4", "avc1", "mp4a", Some(360), 500.0),
                f("247", "webm", "vp9", "none", Some(720), 1200.0),
                f("136", "mp4", "avc1", "none", Some(720), 1100.0),
                f("137", "mp4", "avc1", "none", Some(1080), 2500.0),
                f("251", "webm", "none", "opus", None, 130.0),
                f("140", "m4a", "none", "mp4a", None, 128.0),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn best_pairs_video_and_audio() {
        let s = select(&info(), &Prefs { prefer_mp4: true, ..Default::default() }).unwrap();
        assert_eq!(s.video.unwrap().id, "137");
        assert_eq!(s.audio.unwrap().id, "140");
    }

    #[test]
    fn max_resolution_and_mp4_preference() {
        let s = select(&info(), &Prefs { max_height: Some(720), prefer_mp4: true, ..Default::default() }).unwrap();
        assert_eq!(s.video.unwrap().id, "136");
        let s2 = select(&info(), &Prefs { max_height: Some(720), prefer_mp4: false, ..Default::default() }).unwrap();
        assert_eq!(s2.video.unwrap().id, "247"); // débit supérieur
    }

    #[test]
    fn same_height_prefers_codec_quicktime_can_read() {
        let mut i = info();
        i.formats.push(f("vp9mp4", "mp4", "vp09.00.40.08", "none", Some(1080), 3000.0));
        let s = select(&i, &Prefs { prefer_mp4: true, ..Default::default() }).unwrap();
        assert_eq!(s.video.unwrap().id, "137");
    }

    #[test]
    fn much_higher_bitrate_beats_native_codec() {
        let mut i = info();
        i.formats.push(f("premium", "webm", "vp9", "none", Some(1080), 9000.0));
        let s = select(&i, &Prefs { prefer_mp4: true, ..Default::default() }).unwrap();
        assert_eq!(s.video.unwrap().id, "premium");
    }

    #[test]
    fn sixty_fps_beats_thirty_at_same_height() {
        let mut i = info();
        i.formats.push(Format { fps: Some(60.0), ..f("299", "webm", "vp9", "none", Some(1080), 2000.0) });
        i.formats.iter_mut().find(|x| x.id == "137").unwrap().fps = Some(30.0);
        let s = select(&i, &Prefs { prefer_mp4: true, ..Default::default() }).unwrap();
        assert_eq!(s.video.unwrap().id, "299");
    }

    #[test]
    fn small_aac_does_not_beat_rich_opus_but_close_aac_does() {
        let s = select(&info(), &Prefs { audio_only: true, prefer_mp4: true, ..Default::default() }).unwrap();
        assert_eq!(s.single.unwrap().id, "140");
        let mut i = info();
        i.formats.iter_mut().find(|x| x.id == "140").unwrap().tbr = Some(48.0);
        let s = select(&i, &Prefs { audio_only: true, prefer_mp4: true, ..Default::default() }).unwrap();
        assert_eq!(s.single.unwrap().id, "251");
    }

    #[test]
    fn higher_resolution_wins_even_if_it_needs_conversion() {
        let mut i = info();
        i.formats.push(f("313", "webm", "vp9", "none", Some(2160), 12000.0));
        let s = select(&i, &Prefs { prefer_mp4: true, ..Default::default() }).unwrap();
        assert_eq!(s.video.unwrap().id, "313");
    }

    #[test]
    fn format_without_codec_info_is_a_complete_stream() {
        let i = MediaInfo { formats: vec![Format { id: "1".into(), url: "https://x/1".into(), ext: "mp4".into(), ..Default::default() }], ..Default::default() };
        let s = select(&i, &Prefs::default()).unwrap();
        assert_eq!(s.single.unwrap().id, "1");
    }

    #[test]
    fn nothing_under_limit_takes_smallest() {
        let s = select(&info(), &Prefs { max_height: Some(144), ..Default::default() }).unwrap();
        assert_eq!(s.single.unwrap().id, "18");
    }

    #[test]
    fn audio_only_and_explicit_ids() {
        let s = select(&info(), &Prefs { audio_only: true, prefer_mp4: true, ..Default::default() }).unwrap();
        assert_eq!(s.single.unwrap().id, "140");
        let s = select(&info(), &Prefs { format_id: Some("136".into()), ..Default::default() }).unwrap();
        assert_eq!(s.video.unwrap().id, "136");
        assert!(select(&info(), &Prefs { format_id: Some("999".into()), ..Default::default() }).is_err());
    }

    #[test]
    fn audio_language() {
        let mut i = info();
        i.formats.push(Format { language: Some("fr".into()), ..f("a-fr", "m4a", "none", "mp4a", None, 96.0) });
        let s = select(&i, &Prefs { audio_only: true, languages: vec!["fr".into()], ..Default::default() }).unwrap();
        assert_eq!(s.single.unwrap().id, "a-fr");
    }

    #[test]
    fn choices_list() {
        let c = choices(&info());
        assert_eq!(c[0].height, Some(1080));
        assert!(c.last().unwrap().audio_only);
    }
}
