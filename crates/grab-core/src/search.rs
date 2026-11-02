//! Recherche par les préfixes de yt-dlp et liste des sites.
use crate::error::{Error, Result};
use crate::model::*;
use crate::tools;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

/// Moteurs connus et leur préfixe yt-dlp.
pub const ENGINES: [(&str, &str); 6] = [("youtube", "ytsearch"), ("soundcloud", "scsearch"), ("google", "gvsearch"), ("yahoo", "yvsearch"), ("niconico", "nicosearch"), ("bilibili", "bilisearch")];

pub async fn search(s: &Settings, data_dir: &Path, query: &str, engines: &[String], per_engine: u32) -> Result<Vec<SearchResult>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let bin = tools::require("yt-dlp", s, data_dir)?;
    let mut all = Vec::new();
    let mut last_err = None;
    for name in engines {
        let Some((_, prefix)) = ENGINES.iter().find(|(n, _)| n == name) else { continue };
        let mut args = crate::extract::ytdlp::base_args(s, data_dir);
        // La requête est un seul argument : aucun shell, aucune interprétation.
        args.extend(["-J".into(), "--flat-playlist".into(), "--".into(), format!("{prefix}{per_engine}:{q}")]);
        match tools::run_capture(&bin, &args, Duration::from_secs(60), 32 * 1024 * 1024).await {
            Ok(c) if c.code == Some(0) => {
                let v: Value = serde_json::from_slice(&c.stdout)?;
                for e in v["entries"].as_array().cloned().unwrap_or_default() {
                    let Some(url) = e["url"].as_str().or(e["webpage_url"].as_str()) else { continue };
                    all.push(SearchResult {
                        title: e["title"].as_str().unwrap_or(url).to_string(),
                        url: url.to_string(),
                        engine: name.clone(),
                        duration: e["duration"].as_f64(),
                        thumbnail: e["thumbnails"].as_array().and_then(|t| t.last()).and_then(|t| t["url"].as_str()).map(String::from),
                        uploader: e["uploader"].as_str().or(e["channel"].as_str()).map(String::from),
                    });
                }
            }
            Ok(c) => last_err = Some(crate::extract::ytdlp::map_error(&c.stderr)),
            Err(e) => last_err = Some(e),
        }
    }
    if all.is_empty()
        && let Some(e) = last_err {
            return Err(e);
        }
    Ok(all)
}

pub async fn list_sites(s: &Settings, data_dir: &Path) -> Result<Vec<String>> {
    let bin = tools::require("yt-dlp", s, data_dir)?;
    let c = tools::run_capture(&bin, &["--list-extractors".to_string()], Duration::from_secs(60), 8 * 1024 * 1024).await?;
    if c.code != Some(0) {
        return Err(Error::Extract(crate::util::redact(c.stderr.trim())));
    }
    let mut v: Vec<String> = String::from_utf8_lossy(&c.stdout).lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    v.sort_by_key(|a| a.to_lowercase());
    v.dedup();
    Ok(v)
}

pub fn filter_sites(all: &[String], filter: &str) -> SiteList {
    let f = filter.trim().to_lowercase();
    let sites: Vec<String> = all.iter().filter(|s| f.is_empty() || s.to_lowercase().contains(&f)).cloned().collect();
    SiteList { total: sites.len(), sites }
}
