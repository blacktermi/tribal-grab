//! Extraction par `yt-dlp -J`.
use crate::error::{Error, Result};
use crate::model::*;
use crate::tools;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub fn proxy_arg(s: &Settings) -> Option<String> {
    let p = s.proxy.as_ref().filter(|p| !p.url.trim().is_empty())?;
    match &p.username {
        Some(u) if !u.is_empty() => {
            let pw = crate::secrets::get_proxy_password().unwrap_or_default();
            let (scheme, rest) = p.url.split_once("://").unwrap_or(("http", &p.url));
            let enc = |x: &str| percent_encoding::utf8_percent_encode(x, percent_encoding::NON_ALPHANUMERIC).to_string();
            Some(format!("{scheme}://{}:{}@{rest}", enc(u), enc(&pw)))
        }
        _ => Some(p.url.clone()),
    }
}

/// Arguments communs à toutes les invocations de yt-dlp (réseau, cookies, runtime JS).
pub fn base_args(s: &Settings, data_dir: &Path) -> Vec<String> {
    let mut a: Vec<String> = vec!["--no-warnings".into(), "--no-colors".into(), "--ignore-config".into()];
    a.extend(tools::js_runtime_args(s, data_dir));
    if let Some(p) = proxy_arg(s) {
        a.extend(["--proxy".into(), p]);
    }
    if let Some(b) = s.cookies_browser.as_ref().filter(|b| !b.is_empty()) {
        a.extend(["--cookies-from-browser".into(), b.clone()]);
    } else if let Some(f) = s.cookies_file.as_ref().filter(|f| !f.is_empty()) {
        a.extend(["--cookies".into(), f.clone()]);
    }
    if let Some(x) = s.xff.as_ref().filter(|x| !x.is_empty()) {
        a.extend(["--xff".into(), x.clone()]);
    }
    if let Some(u) = s.user_agent.as_ref().filter(|u| !u.is_empty()) {
        a.extend(["--user-agent".into(), u.clone()]);
    }
    a.extend(["--socket-timeout".into(), s.timeout_secs.max(5).to_string()]);
    a
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from)
}

fn headers_of(v: &Value) -> BTreeMap<String, String> {
    v.get("http_headers")
        .and_then(Value::as_object)
        .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
        .unwrap_or_default()
}

fn prettify_site(key: &str) -> String {
    match key.to_lowercase().as_str() {
        "youtube" => "YouTube".into(),
        "soundcloud" => "SoundCloud".into(),
        "vimeo" => "Vimeo".into(),
        "twitter" => "X".into(),
        _ => key.to_string(),
    }
}

pub fn map_error(stderr: &str) -> Error {
    let clean = crate::util::redact(stderr);
    let line = clean.lines().rev().find(|l| l.contains("ERROR")).unwrap_or_else(|| clean.lines().last().unwrap_or("erreur inconnue"));
    let msg = line.trim().trim_start_matches("ERROR:").trim().to_string();
    let l = msg.to_lowercase();
    if l.contains("drm") {
        Error::Drm
    } else if l.contains("unsupported url") {
        Error::Unsupported(msg)
    } else if l.contains("sign in") || l.contains("confirm you") || l.contains("login") {
        Error::Extract(format!("{msg} (essayez les cookies d'un navigateur dans les réglages Réseau)"))
    } else {
        Error::Extract(msg)
    }
}

pub async fn extract(s: &Settings, data_dir: &Path, url: &str) -> Result<Resolved> {
    let bin = tools::require("yt-dlp", s, data_dir)?;
    let mut args = base_args(s, data_dir);
    args.extend(["-J".into(), "--flat-playlist".into(), "--".into(), url.to_string()]);
    let out = tools::run_capture(&bin, &args, Duration::from_secs(180), 256 * 1024 * 1024).await?;
    if out.code != Some(0) {
        return Err(map_error(&out.stderr));
    }
    let v: Value = serde_json::from_slice(&out.stdout)?;
    parse_json(&v, url)
}

pub fn parse_json(v: &Value, page_url: &str) -> Result<Resolved> {
    if v.get("_type").and_then(Value::as_str) == Some("playlist") {
        let entries: Vec<LinkCandidate> = v
            .get("entries")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|e| !e.is_null())
                    .filter_map(|e| {
                        let url = str_of(e, "url").or_else(|| str_of(e, "webpage_url"))?;
                        Some(LinkCandidate {
                            url,
                            title: str_of(e, "title"),
                            duration: e.get("duration").and_then(Value::as_f64),
                            thumbnail: str_of(e, "thumbnail"),
                            size: None,
                            kind: str_of(e, "ie_key"),
                            recognized: true,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if entries.is_empty() {
            return Err(Error::Extract("la playlist est vide ou inaccessible".into()));
        }
        return Ok(Resolved::Playlist { title: str_of(v, "title").unwrap_or_else(|| "Playlist".into()), entries });
    }
    Ok(Resolved::Single(Box::new(parse_video(v, page_url)?)))
}

fn parse_format(f: &Value, top_headers: &BTreeMap<String, String>) -> Option<Format> {
    let ext = str_of(f, "ext").unwrap_or_default();
    let note = str_of(f, "format_note").unwrap_or_default();
    if ext == "mhtml" || note == "storyboard" {
        return None;
    }
    let proto = str_of(f, "protocol").unwrap_or_else(|| "https".into());
    let url = str_of(f, "url")?;
    let mut headers = top_headers.clone();
    headers.extend(headers_of(f));
    let (protocol, plan) = if proto == "https" || proto == "http" {
        (Protocol::Http, None)
    } else if proto.starts_with("m3u8") {
        (Protocol::Hls, None)
    } else if let Some(frags) = f.get("fragments").and_then(Value::as_array).filter(|a| !a.is_empty()) {
        let base = str_of(f, "fragment_base_url").unwrap_or_default();
        let segments = frags
            .iter()
            .filter_map(|fr| {
                let u = str_of(fr, "url").or_else(|| str_of(fr, "path").map(|p| format!("{base}{p}")))?;
                Some(Segment { url: u, ..Default::default() })
            })
            .collect();
        (Protocol::Segments, Some(Arc::new(SegmentPlan { segments, ..Default::default() })))
    } else {
        (Protocol::Ytdlp, None)
    };
    let codec = |k: &str| str_of(f, k);
    Some(Format {
        id: str_of(f, "format_id").unwrap_or_else(|| "0".into()),
        url,
        protocol,
        ext,
        vcodec: codec("vcodec"),
        acodec: codec("acodec"),
        width: f.get("width").and_then(Value::as_u64).map(|x| x as u32),
        height: f.get("height").and_then(Value::as_u64).map(|x| x as u32),
        fps: f.get("fps").and_then(Value::as_f64),
        tbr: f.get("tbr").and_then(Value::as_f64),
        filesize: f.get("filesize").and_then(Value::as_u64).or_else(|| f.get("filesize_approx").and_then(Value::as_f64).map(|x| x as u64)),
        language: str_of(f, "language"),
        label: str_of(f, "format_note"),
        headers,
        has_drm: f.get("has_drm").and_then(Value::as_bool).unwrap_or(false),
        plan,
    })
}

fn parse_subs(v: &Value, key: &str, auto: bool, out: &mut Vec<SubtitleTrack>) {
    const PREF: [&str; 5] = ["vtt", "srt", "ttml", "dfxp", "xml"];
    let Some(map) = v.get(key).and_then(Value::as_object) else { return };
    for (lang, list) in map {
        let Some(list) = list.as_array() else { continue };
        let best = PREF.iter().find_map(|p| list.iter().find(|e| e.get("ext").and_then(Value::as_str) == Some(p)));
        if let Some(e) = best
            && let Some(url) = str_of(e, "url") {
                out.push(SubtitleTrack { lang: lang.clone(), name: str_of(e, "name"), url, ext: str_of(e, "ext").unwrap_or_default(), auto });
            }
    }
}

pub fn parse_video(v: &Value, page_url: &str) -> Result<MediaInfo> {
    let headers = headers_of(v);
    let mut formats: Vec<Format> = v.get("formats").and_then(Value::as_array).map(|a| a.iter().filter_map(|f| parse_format(f, &headers)).collect()).unwrap_or_default();
    if formats.is_empty() && v.get("url").is_some() {
        formats.extend(parse_format(v, &headers));
    }
    if formats.is_empty() {
        return Err(Error::Extract("aucun format téléchargeable trouvé".into()));
    }
    if formats.iter().all(|f| f.has_drm) {
        return Err(Error::Drm);
    }
    formats.retain(|f| !f.has_drm);
    let mut subtitles = Vec::new();
    parse_subs(v, "subtitles", false, &mut subtitles);
    parse_subs(v, "automatic_captions", true, &mut subtitles);
    let playlist = str_of(v, "playlist_title").map(|title| PlaylistRef {
        title,
        index: v.get("playlist_index").and_then(Value::as_u64).unwrap_or(0) as u32,
        count: v.get("n_entries").and_then(Value::as_u64).unwrap_or(0) as u32,
    });
    let url = str_of(v, "webpage_url").unwrap_or_else(|| page_url.to_string());
    let id = str_of(v, "id").unwrap_or_default();
    Ok(MediaInfo {
        title: str_of(v, "title").unwrap_or_else(|| id.clone()),
        id,
        site: prettify_site(&str_of(v, "extractor_key").or_else(|| str_of(v, "extractor")).unwrap_or_else(|| "Web".into())),
        host: crate::util::host_of(&url),
        url,
        uploader: str_of(v, "uploader").or_else(|| str_of(v, "channel")),
        upload_date: str_of(v, "upload_date"),
        timestamp: v.get("timestamp").and_then(Value::as_i64),
        thumbnail: str_of(v, "thumbnail"),
        description: str_of(v, "description"),
        duration: v.get("duration").and_then(Value::as_f64),
        is_live: v.get("is_live").and_then(Value::as_bool).unwrap_or(false) || str_of(v, "live_status").as_deref() == Some("is_live"),
        formats,
        subtitles,
        playlist,
        headers,
        referer: None,
        is_file: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_video_json() {
        let j = serde_json::json!({
            "id": "abc", "title": "Titre", "extractor_key": "Youtube", "webpage_url": "https://www.youtube.com/watch?v=abc",
            "http_headers": {"User-Agent": "x"},
            "formats": [
              {"format_id": "sb0", "ext": "mhtml", "url": "https://x/sb", "protocol": "mhtml"},
              {"format_id": "18", "ext": "mp4", "url": "https://x/v.mp4", "protocol": "https", "vcodec": "avc1", "acodec": "mp4a", "height": 360, "tbr": 500.0, "filesize": 1000},
              {"format_id": "140", "ext": "m4a", "url": "https://x/a.m4a", "protocol": "https", "vcodec": "none", "acodec": "mp4a"},
              {"format_id": "h", "ext": "mp4", "url": "https://x/h.m3u8", "protocol": "m3u8_native", "vcodec": "avc1", "acodec": "none", "height": 720},
              {"format_id": "drm", "ext": "mp4", "url": "https://x/d", "protocol": "https", "has_drm": true}
            ],
            "subtitles": {"fr": [{"ext": "json3", "url": "https://x/s.json3"}, {"ext": "vtt", "url": "https://x/s.vtt", "name": "Français"}]},
            "automatic_captions": {"en": [{"ext": "vtt", "url": "https://x/a.vtt"}]}
        });
        let Resolved::Single(i) = parse_json(&j, "https://x").unwrap() else { panic!() };
        assert_eq!(i.site, "YouTube");
        assert_eq!(i.formats.len(), 3);
        assert_eq!(i.formats[2].protocol, Protocol::Hls);
        assert_eq!(i.subtitles.len(), 2);
        assert!(i.subtitles.iter().any(|s| s.lang == "en" && s.auto));
    }

    #[test]
    fn drm_is_per_format_and_maybe_is_clear() {
        let j = serde_json::json!({"id": "a", "title": "t", "formats": [
            {"format_id": "p", "ext": "mp4", "url": "https://x/p", "has_drm": true, "height": 1080, "vcodec": "avc1", "acodec": "mp4a"},
            {"format_id": "m", "ext": "mp4", "url": "https://x/m", "has_drm": "maybe", "height": 720, "vcodec": "avc1", "acodec": "mp4a"},
            {"format_id": "c", "ext": "mp4", "url": "https://x/c", "has_drm": false, "height": 480, "vcodec": "avc1", "acodec": "mp4a"}]});
        let Resolved::Single(i) = parse_json(&j, "https://x").unwrap() else { panic!() };
        let ids: Vec<_> = i.formats.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["m", "c"]);
    }

    #[test]
    fn all_drm_is_refused() {
        let j = serde_json::json!({"id": "a", "title": "t", "formats": [{"format_id": "1", "ext": "mp4", "url": "https://x", "has_drm": true}]});
        assert!(matches!(parse_json(&j, "https://x"), Err(Error::Drm)));
    }

    #[test]
    fn error_mapping() {
        assert!(matches!(map_error("ERROR: [generic] Unsupported URL: https://x.com/a"), Error::Unsupported(_)));
        assert!(matches!(map_error("ERROR: This video is DRM protected"), Error::Drm));
    }
}
