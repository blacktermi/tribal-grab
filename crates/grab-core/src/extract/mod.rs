//! Résolution d'une URL en médias, dans l'ordre de la spec :
//! intégration, fichier direct, manifeste, yt-dlp, détection HTML, non pris en charge.
pub mod dash;
pub mod direct;
pub mod hls;
pub mod html;
pub mod ytdlp;

use crate::error::{Error, Result};
use crate::integrations::Integrations;
use crate::model::*;
use crate::net::Net;
use crate::util::{MAX_HTML_BYTES, MAX_MANIFEST_BYTES};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub struct Extractor {
    pub net: Net,
    pub settings: Settings,
    pub data_dir: PathBuf,
    pub integrations: Arc<Integrations>,
}

#[derive(PartialEq)]
enum ManifestKind {
    Hls,
    Dash,
}

fn manifest_kind(url: &str, content_type: Option<&str>) -> Option<ManifestKind> {
    let ext = direct::url_ext(url);
    let ct = content_type.unwrap_or("").to_lowercase();
    if ext.as_deref() == Some("m3u8") || ct.contains("mpegurl") {
        Some(ManifestKind::Hls)
    } else if ext.as_deref() == Some("mpd") || ct.contains("dash+xml") {
        Some(ManifestKind::Dash)
    } else {
        None
    }
}

impl Extractor {
    fn headers_for(&self, referer: Option<&str>) -> BTreeMap<String, String> {
        let mut h = BTreeMap::new();
        if let Some(r) = referer {
            h.insert("Referer".into(), r.to_string());
            if let Ok(u) = url::Url::parse(r) {
                h.insert("Origin".into(), u.origin().ascii_serialization());
            }
        }
        h
    }

    pub async fn resolve(&self, url: &str, referer: Option<&str>) -> Result<Resolved> {
        let parsed = crate::util::validate_url(url)?;
        let url = parsed.as_str();
        // 1. Intégration personnalisée
        if let Some(integ) = self.integrations.find(url) {
            let out = crate::integrations::run(&integ, url, &self.net).await;
            return out.result.map(|m| Resolved::Single(Box::new(m)));
        }
        // 2-3. Fichier direct ou manifeste
        if let Some(r) = self.resolve_media_url(url, referer).await? {
            return Ok(r);
        }
        // 4. yt-dlp
        let ytdlp_err = match ytdlp::extract(&self.settings, &self.data_dir, url).await {
            Ok(r) => return Ok(r),
            Err(e @ (Error::Drm | Error::Cancelled)) => return Err(e),
            Err(e) => e,
        };
        // 5. Détection native dans le HTML
        if let Some(r) = self.resolve_html(url).await {
            return Ok(r);
        }
        // 6. Rien trouvé
        match ytdlp_err {
            Error::Unsupported(m) => Ok(Resolved::Unsupported(m)),
            Error::ToolMissing(_) => Ok(Resolved::Unsupported("aucun média détecté dans la page (yt-dlp n'est pas installé)".into())),
            e => Err(e),
        }
    }

    /// Fichier direct, ou manifeste HLS/DASH. `Ok(None)` : ce n'est ni l'un ni l'autre.
    pub async fn resolve_media_url(&self, url: &str, referer: Option<&str>) -> Result<Option<Resolved>> {
        let mut headers = self.headers_for(referer);
        let ext = direct::url_ext(url);
        let by_ext = ext.as_deref();
        let mut head = None;
        if by_ext.is_none_or(|e| e != "m3u8" && e != "mpd") {
            let p = tokio::time::timeout(Duration::from_secs(8), direct::probe(&self.net, url, &headers)).await;
            head = p.ok().and_then(|r| r.ok());
            if head.is_none() {
                let mut neutral = headers.clone();
                neutral.insert("User-Agent".into(), crate::net::NEUTRAL_UA.into());
                let p = tokio::time::timeout(Duration::from_secs(8), direct::probe(&self.net, url, &neutral)).await;
                head = p.ok().and_then(|r| r.ok());
                if head.is_some() {
                    headers = neutral;
                }
            }
        }
        let ct = head.as_ref().and_then(|h| h.content_type.clone());
        if let Some(kind) = manifest_kind(url, ct.as_deref()) {
            let text = self.net.fetch_text(url, &headers, MAX_MANIFEST_BYTES).await?;
            let info = match kind {
                ManifestKind::Hls => hls::parse_manifest(url, &text, &headers, referer)?,
                ManifestKind::Dash => dash::parse_manifest(url, &text, &headers, referer)?,
            };
            return Ok(Some(Resolved::Single(Box::new(info))));
        }
        let ct_media = ct.as_deref().is_some_and(|c| c.starts_with("video/") || c.starts_with("audio/"));
        let ext_media = by_ext.is_some_and(direct::is_direct_ext);
        if ct_media || ext_media {
            let h = head.unwrap_or_default();
            return Ok(Some(Resolved::Single(Box::new(direct::info_from_direct(url, &h, &headers, referer)))));
        }
        if direct::is_file_download(head.as_ref(), by_ext) {
            let h = head.unwrap_or_default();
            return Ok(Some(Resolved::Single(Box::new(direct::info_from_file(url, &h, &headers, referer)))));
        }
        Ok(None)
    }

    async fn resolve_html(&self, url: &str) -> Option<Resolved> {
        let html = self.net.fetch_text(url, &BTreeMap::new(), MAX_HTML_BYTES).await.ok()?;
        let page = html::scan(url, &html);
        match page.media.len() {
            0 => None,
            1 => {
                let r = self.resolve_media_url(&page.media[0].url, Some(url)).await.ok().flatten()?;
                Some(match r {
                    Resolved::Single(m) if m.is_file => Resolved::Single(m),
                    Resolved::Single(mut m) => {
                        if let Some(t) = &page.title {
                            m.title = t.clone();
                        }
                        m.thumbnail = page.thumbnail.clone();
                        m.site = crate::util::host_of(url);
                        Resolved::Single(m)
                    }
                    other => other,
                })
            }
            _ => Some(Resolved::Links(page.media.into_iter().map(|mut c| {
                c.title = c.title.or_else(|| page.title.clone());
                c.thumbnail = page.thumbnail.clone();
                c
            }).collect())),
        }
    }
}
