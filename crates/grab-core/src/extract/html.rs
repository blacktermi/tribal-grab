//! Détection native dans le HTML : `<video>`, `<source>`, `og:video`, JSON-LD, URL de manifestes, liens vers des fichiers.
use crate::model::*;
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::Value;

/// Liens `<a>` à proposer au téléchargement. Les autres extensions passent par `<a download>`.
const FILE_EXTS: [&str; 40] = [
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "dmg", "pkg", "iso", "exe", "msi", "deb", "rpm", "apk", "ipa", "pdf", "doc", "docx",
    "xls", "xlsx", "ppt", "pptx", "csv", "epub", "mobi", "psd", "ai", "torrent", "ttf", "otf", "jpg", "jpeg", "png", "gif", "webp", "heic", "tiff", "svg",
];

pub struct Page {
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub media: Vec<LinkCandidate>,
}

fn abs(base: &url::Url, s: &str) -> Option<String> {
    let s = s.trim().replace("\\/", "/").replace("&amp;", "&");
    if s.is_empty() || s.starts_with("blob:") || s.starts_with("data:") {
        return None;
    }
    let u = base.join(&s).ok()?;
    matches!(u.scheme(), "http" | "https").then(|| u.to_string())
}

fn kind_of(url: &str) -> &'static str {
    let path = url.split(['?', '#']).next().unwrap_or("").to_lowercase();
    if super::direct::url_ext(url).is_some_and(|e| FILE_EXTS.contains(&e.as_str())) {
        "file"
    } else if path.ends_with(".m3u8") {
        "hls"
    } else if path.ends_with(".mpd") {
        "dash"
    } else if [".mp3", ".m4a", ".aac", ".ogg", ".wav", ".flac"].iter().any(|e| path.ends_with(e)) {
        "audio"
    } else {
        "video"
    }
}

fn walk_json(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            for (k, val) in o {
                if k == "contentUrl"
                    && let Some(s) = val.as_str() {
                        out.push(s.to_string());
                    }
                walk_json(val, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk_json(x, out)),
        _ => {}
    }
}

pub fn scan(page_url: &str, html: &str) -> Page {
    let base = url::Url::parse(page_url).unwrap_or_else(|_| url::Url::parse("https://invalid.test/").unwrap());
    let doc = Html::parse_document(html);
    let sel = |s: &str| Selector::parse(s).unwrap();
    let mut urls: Vec<String> = Vec::new();

    for (s, attr) in [
        ("video[src]", "src"),
        ("video source[src]", "src"),
        ("audio[src]", "src"),
        ("audio source[src]", "src"),
        ("meta[property='og:video']", "content"),
        ("meta[property='og:video:url']", "content"),
        ("meta[property='og:video:secure_url']", "content"),
        ("meta[property='og:audio']", "content"),
        ("meta[name='twitter:player:stream']", "content"),
        ("meta[itemprop='contentUrl']", "content"),
    ] {
        for el in doc.select(&sel(s)) {
            if let Some(u) = el.value().attr(attr).and_then(|v| abs(&base, v)) {
                urls.push(u);
            }
        }
    }
    for el in doc.select(&sel("a[href]")) {
        let Some(u) = el.value().attr("href").and_then(|v| abs(&base, v)) else { continue };
        let ext = super::direct::url_ext(&u);
        if el.value().attr("download").is_some() || ext.is_some_and(|e| FILE_EXTS.contains(&e.as_str()) || super::direct::is_direct_ext(&e)) {
            urls.push(u);
        }
    }
    for el in doc.select(&sel("script[type='application/ld+json']")) {
        let txt: String = el.text().collect();
        if let Ok(v) = serde_json::from_str::<Value>(&txt) {
            let mut found = Vec::new();
            walk_json(&v, &mut found);
            urls.extend(found.iter().filter_map(|u| abs(&base, u)));
        }
    }
    // URL de médias écrites dans les scripts ou le HTML brut.
    let re = Regex::new(r#"https?:(?:\\?/){2}[^\s"'<>\\]+?\.(?:m3u8|mpd|mp4|webm|mp3|m4a)(?:\?[^\s"'<>\\]*)?"#).unwrap();
    let normalized = html.replace("\\/", "/");
    for m in re.find_iter(&normalized) {
        if let Some(u) = abs(&base, m.as_str()) {
            urls.push(u);
        }
    }
    let mut seen = std::collections::HashSet::new();
    urls.retain(|u| seen.insert(u.clone()));

    let meta = |p: &str| doc.select(&sel(&format!("meta[property='{p}']"))).next().and_then(|e| e.value().attr("content")).map(|s| s.trim().to_string());
    let title = meta("og:title").or_else(|| doc.select(&sel("title")).next().map(|t| t.text().collect::<String>().trim().to_string())).filter(|t| !t.is_empty());
    let thumbnail = meta("og:image").and_then(|u| abs(&base, &u));
    Page {
        title,
        thumbnail,
        media: urls.into_iter().map(|u| LinkCandidate { kind: Some(kind_of(&u).into()), url: u, recognized: false, ..Default::default() }).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_media_in_html() {
        let html = r#"<html><head><title>Ma page</title><meta property="og:video" content="/v/a.mp4"><meta property="og:image" content="/i.jpg">
          <script type="application/ld+json">{"@type":"VideoObject","contentUrl":"https://cdn.test/ld.mp4"}</script></head>
          <body><video><source src="b.webm"></video>
          <script>var cfg = {"hls":"https:\/\/cdn.test\/live\/master.m3u8?t=1"};</script></body></html>"#;
        let p = scan("https://site.test/page/1", html);
        let urls: Vec<_> = p.media.iter().map(|m| m.url.as_str()).collect();
        assert!(urls.contains(&"https://site.test/v/a.mp4"), "{urls:?}");
        assert!(urls.contains(&"https://cdn.test/ld.mp4"));
        assert!(urls.contains(&"https://site.test/page/b.webm"));
        assert!(urls.contains(&"https://cdn.test/live/master.m3u8?t=1"));
        assert_eq!(p.title.as_deref(), Some("Ma page"));
        assert_eq!(p.thumbnail.as_deref(), Some("https://site.test/i.jpg"));
        let p = scan("https://site.test/d/", r#"<a href="a.zip">z</a><a href="/b.pdf?x=1">p</a><a href="c" download>c</a><a href="/page.html">n</a>"#);
        let urls: Vec<_> = p.media.iter().map(|m| (m.url.as_str(), m.kind.as_deref())).collect();
        assert!(urls.contains(&("https://site.test/d/a.zip", Some("file"))), "{urls:?}");
        assert!(urls.contains(&("https://site.test/b.pdf?x=1", Some("file"))));
        assert!(urls.iter().any(|u| u.0 == "https://site.test/d/c"));
        assert!(!urls.iter().any(|u| u.0.ends_with("page.html")));
    }
}
