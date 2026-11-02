//! Fichiers directs : médias reconnus par extension ou `Content-Type`, et fichiers de tout autre type.
use super::hls::percent_decode;
use crate::error::Result;
use crate::model::*;
use crate::net::Net;
use std::collections::BTreeMap;

pub const MEDIA_EXTS: [&str; 24] = [
    "mp4", "m4v", "mov", "webm", "mkv", "avi", "flv", "wmv", "mpg", "mpeg", "ts", "3gp", "mp3", "m4a", "aac", "flac", "wav", "ogg",
    "opus", "oga", "ogv", "wma", "aiff", "mka",
];

/// Extensions de pages web : elles ne désignent jamais un fichier à enregistrer.
const PAGE_EXTS: [&str; 10] = ["html", "htm", "xhtml", "shtml", "php", "asp", "aspx", "jsp", "jspx", "cgi"];

#[derive(Debug, Default, Clone)]
pub struct Head {
    pub content_type: Option<String>,
    pub length: Option<u64>,
    pub ranges: bool,
    pub filename: Option<String>,
    /// `Content-Disposition: attachment` : le serveur demande explicitement un enregistrement.
    pub attachment: bool,
    /// ETag ou Last-Modified : sert à vérifier qu'une reprise concerne le même fichier.
    pub validator: Option<String>,
}

pub fn url_ext(url: &str) -> Option<String> {
    let u = url::Url::parse(url).ok()?;
    let last = u.path_segments()?.next_back()?.to_string();
    let ext = last.rsplit_once('.')?.1.to_lowercase();
    valid_ext(&ext).then_some(ext)
}

/// De 1 à 12 caractères : `.torrent` ou `.sketch` sont de vraies extensions, `.v2-final` n'en est pas une.
fn valid_ext(e: &str) -> bool {
    (1..=12).contains(&e.len()) && e.chars().all(|c| c.is_ascii_alphanumeric())
}

pub fn is_direct_ext(ext: &str) -> bool {
    MEDIA_EXTS.contains(&ext)
}

pub fn ext_from_mime(m: &str) -> Option<&'static str> {
    Some(match m.split(';').next()?.trim() {
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "video/x-matroska" => "mkv",
        "video/mp2t" => "ts",
        "audio/mpeg" => "mp3",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/aac" => "aac",
        "audio/ogg" => "ogg",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/flac" => "flac",
        _ => return None,
    })
}

fn file_ext_from_mime(m: &str) -> Option<&'static str> {
    Some(match m.split(';').next()?.trim() {
        "application/pdf" => "pdf",
        "application/zip" | "application/x-zip-compressed" => "zip",
        "application/gzip" | "application/x-gzip" => "gz",
        "application/x-tar" => "tar",
        "application/x-7z-compressed" => "7z",
        "application/vnd.rar" | "application/x-rar-compressed" => "rar",
        "application/x-apple-diskimage" => "dmg",
        "application/json" => "json",
        "application/xml" | "text/xml" => "xml",
        "text/plain" => "txt",
        "text/csv" => "csv",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => return None,
    })
}

/// Tout ce qui n'est pas une page web est un fichier à enregistrer.
pub fn is_file_download(head: Option<&Head>, ext: Option<&str>) -> bool {
    let not_page = ext.is_some_and(|e| !PAGE_EXTS.contains(&e));
    let Some(h) = head else { return not_page };
    let ct = h.content_type.as_deref().unwrap_or("").to_lowercase();
    let html = ct.starts_with("text/html") || ct.starts_with("application/xhtml");
    h.attachment || (!ct.is_empty() && !html) || (ct.is_empty() && not_page)
}

/// Sépare un nom en base et extension ; un nom comme `.htaccess` n'a pas d'extension.
fn split_name(name: &str) -> (String, Option<String>) {
    match name.rsplit_once('.') {
        Some((stem, e)) if !stem.is_empty() && valid_ext(e) => (stem.to_string(), Some(e.to_string())),
        _ => (name.to_string(), None),
    }
}

pub fn info_from_file(url: &str, head: &Head, headers: &BTreeMap<String, String>, referer: Option<&str>) -> MediaInfo {
    let from_url = url::Url::parse(url).ok().and_then(|u| u.path_segments().and_then(|mut s| s.next_back().map(percent_decode))).unwrap_or_default();
    let raw = head.filename.clone().filter(|n| !n.trim().is_empty()).unwrap_or(from_url);
    let (stem, ext) = split_name(raw.trim());
    let mime = head.content_type.as_deref();
    let ext = ext.or_else(|| mime.and_then(|m| ext_from_mime(m).or_else(|| file_ext_from_mime(m))).map(String::from)).unwrap_or_else(|| "bin".into());
    MediaInfo {
        id: stem.clone(),
        title: if stem.is_empty() { crate::util::host_of(url) } else { stem },
        url: url.to_string(),
        site: "Fichier direct".into(),
        host: crate::util::host_of(url),
        headers: headers.clone(),
        referer: referer.map(String::from),
        is_file: true,
        formats: vec![Format {
            id: "file".into(),
            url: url.to_string(),
            protocol: Protocol::Http,
            ext,
            vcodec: Some("none".into()),
            acodec: Some("none".into()),
            filesize: head.length,
            headers: headers.clone(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn filename_from_disposition(cd: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?i)filename\*\s*=\s*[^']*'[^']*'([^;]+)|filename\s*=\s*"?([^";]+)"?"#).ok()?;
    let c = re.captures(cd)?;
    let raw = c.get(1).or(c.get(2))?.as_str().trim();
    Some(percent_decode(raw))
}

/// Une seule requête `Range: bytes=0-0` : donne taille, type et prise en charge des plages.
pub async fn probe(net: &Net, url: &str, headers: &BTreeMap<String, String>) -> Result<Head> {
    let resp = net.request(url, headers).header("Range", "bytes=0-0").send().await?;
    let st = resp.status();
    if !(st.is_success()) {
        return Err(crate::error::Error::Network(format!("le serveur a répondu {st}")));
    }
    let h = resp.headers();
    let get = |k: &str| h.get(k).and_then(|v| v.to_str().ok()).map(String::from);
    let mut head = Head { content_type: get("content-type"), validator: get("etag").or_else(|| get("last-modified")), filename: get("content-disposition").and_then(|c| filename_from_disposition(&c)), attachment: get("content-disposition").is_some_and(|c| c.trim().to_lowercase().starts_with("attachment")), ..Default::default() };
    if st.as_u16() == 206 {
        head.ranges = true;
        head.length = get("content-range").and_then(|c| c.rsplit('/').next().and_then(|t| t.parse().ok()));
    } else {
        head.length = get("content-length").and_then(|l| l.parse().ok());
    }
    Ok(head)
}

pub fn info_from_direct(url: &str, head: &Head, headers: &BTreeMap<String, String>, referer: Option<&str>) -> MediaInfo {
    let ext = url_ext(url).filter(|e| is_direct_ext(e)).or_else(|| head.content_type.as_deref().and_then(ext_from_mime).map(String::from)).unwrap_or_else(|| "mp4".into());
    let from_url = url::Url::parse(url).ok().and_then(|u| u.path_segments().and_then(|mut s| s.next_back().map(percent_decode))).unwrap_or_default();
    let raw = head.filename.clone().unwrap_or(from_url);
    let stem = raw.rsplit_once('.').filter(|(_, e)| e.len() <= 5).map(|x| x.0.to_string()).unwrap_or(raw);
    let is_audio = matches!(ext.as_str(), "mp3" | "m4a" | "aac" | "flac" | "wav" | "ogg" | "opus" | "oga" | "wma" | "aiff" | "mka");
    MediaInfo {
        id: stem.clone(),
        title: if stem.is_empty() { crate::util::host_of(url) } else { stem },
        url: url.to_string(),
        site: "Fichier direct".into(),
        host: crate::util::host_of(url),
        headers: headers.clone(),
        referer: referer.map(String::from),
        formats: vec![Format {
            id: "direct".into(),
            url: url.to_string(),
            protocol: Protocol::Http,
            ext,
            vcodec: Some(if is_audio { "none".into() } else { "unknown".into() }),
            acodec: Some("unknown".into()),
            filesize: head.length,
            headers: headers.clone(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ext_detection() {
        assert_eq!(url_ext("https://a.com/b/Video%20x.MP4?x=1").as_deref(), Some("mp4"));
        assert_eq!(url_ext("https://a.com/watch"), None);
        let i = info_from_direct("https://a.com/b/Mon%20film.mp4", &Head::default(), &Default::default(), None);
        assert_eq!(i.title, "Mon film");
    }
    #[test]
    fn every_file_type_is_recognized() {
        let head = |ct: &str| Head { content_type: Some(ct.into()), ..Default::default() };
        assert!(is_file_download(Some(&head("application/zip")), Some("zip")));
        assert!(is_file_download(Some(&head("application/octet-stream")), None));
        assert!(!is_file_download(Some(&head("text/html; charset=utf-8")), None));
        assert!(!is_file_download(Some(&head("text/html")), Some("php")));
        assert!(is_file_download(Some(&Head { attachment: true, ..head("text/html") }), None));
        assert!(is_file_download(None, Some("torrent")));
        assert!(!is_file_download(None, Some("html")));
        assert!(!is_file_download(None, None));
    }
    #[test]
    fn file_keeps_its_real_name() {
        let i = info_from_file("https://a.com/d/Mon%20archive.tar.gz?t=1", &Head::default(), &Default::default(), None);
        assert_eq!((i.title.as_str(), i.formats[0].ext.as_str(), i.is_file), ("Mon archive.tar", "gz", true));
        let h = Head { filename: Some("rapport.pdf".into()), ..Default::default() };
        assert_eq!(info_from_file("https://a.com/dl?id=3", &h, &Default::default(), None).title, "rapport");
        let h = Head { content_type: Some("application/pdf".into()), ..Default::default() };
        assert_eq!(info_from_file("https://a.com/doc", &h, &Default::default(), None).formats[0].ext, "pdf");
        assert_eq!(info_from_file("https://a.com/doc", &Head::default(), &Default::default(), None).formats[0].ext, "bin");
        assert_eq!(split_name(".htaccess"), (".htaccess".to_string(), None));
    }
    #[test]
    fn disposition() {
        assert_eq!(filename_from_disposition("attachment; filename=\"a b.mp4\"").as_deref(), Some("a b.mp4"));
        assert_eq!(filename_from_disposition("attachment; filename*=UTF-8''%C3%A9t%C3%A9.mp4").as_deref(), Some("été.mp4"));
    }
}
