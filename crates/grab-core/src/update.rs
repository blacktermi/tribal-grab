//! Mise à jour de yt-dlp depuis les publications officielles GitHub, avec vérification SHA256.
use crate::error::{Error, Result};
use crate::net::Net;
use crate::tools;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const ARCHIVE: &str = "yt-dlp_macos.zip";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current: Option<String>,
    pub latest: String,
    pub available: bool,
}

struct Release {
    tag: String,
    archive_url: String,
    sums_url: String,
}

async fn latest_release(net: &Net) -> Result<Release> {
    let text = net.fetch_text(LATEST, &[("Accept".to_string(), "application/vnd.github+json".to_string())].into(), 4 * 1024 * 1024).await?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let assets = v["assets"].as_array().cloned().unwrap_or_default();
    let find = |name: &str| assets.iter().find(|a| a["name"] == name).and_then(|a| a["browser_download_url"].as_str()).map(String::from);
    Ok(Release {
        tag: v["tag_name"].as_str().ok_or_else(|| Error::Network("réponse GitHub inattendue".into()))?.to_string(),
        archive_url: find(ARCHIVE).ok_or_else(|| Error::Network("archive macOS introuvable dans la publication".into()))?,
        sums_url: find("SHA2-256SUMS").ok_or_else(|| Error::Network("fichier SHA2-256SUMS introuvable dans la publication".into()))?,
    })
}

pub async fn check(net: &Net, current: Option<String>) -> Result<UpdateInfo> {
    let r = latest_release(net).await?;
    let available = current.as_deref().is_none_or(|c| c < r.tag.as_str());
    Ok(UpdateInfo { current, latest: r.tag, available })
}

pub fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let (h, n) = (it.next()?, it.next()?);
        (n.trim_start_matches('*') == name).then(|| h.to_lowercase())
    })
}

/// Télécharge, vérifie le SHA256, puis installe dans `<bin_dir>/yt-dlp-app`. Retourne la version installée.
/// On prend l'archive décompressée : le binaire en un seul fichier se redéploie à chaque lancement et macOS
/// le réanalyse alors à chaque fois (18 s contre 0,3 s, mesuré le 02/11/2026).
pub async fn install_ytdlp(net: &Net, bin_dir: &Path) -> Result<String> {
    let rel = latest_release(net).await?;
    let sums = net.fetch_text(&rel.sums_url, &Default::default(), 1024 * 1024).await?;
    let want = expected_hash(&sums, ARCHIVE).ok_or_else(|| Error::Network("somme SHA256 absente de la publication".into()))?;
    let data = net.fetch_bytes(&rel.archive_url, &Default::default(), 200 * 1024 * 1024).await?;
    let got = hex::encode(Sha256::digest(&data));
    if got != want {
        return Err(Error::Network("la somme SHA256 de yt-dlp ne correspond pas : mise à jour annulée".into()));
    }
    std::fs::create_dir_all(bin_dir)?;
    let zip = bin_dir.join("yt-dlp-app.zip");
    let tmp = bin_dir.join("yt-dlp-app.new");
    let app = bin_dir.join(tools::YTDLP_DIR);
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }
    std::fs::write(&zip, &data)?;
    let args = ["-x".to_string(), "-k".to_string(), zip.display().to_string(), tmp.display().to_string()];
    let unzipped = tools::run_capture(Path::new("/usr/bin/ditto"), &args, Duration::from_secs(120), 10_000).await;
    let _ = std::fs::remove_file(&zip);
    let unzipped = unzipped?;
    let exe = tmp.join(tools::YTDLP_EXE);
    if unzipped.code != Some(0) || !exe.is_file() {
        return Err(Error::Other(format!("décompression de yt-dlp impossible : {}", unzipped.stderr.trim())));
    }
    // L'analyse de macOS se fait une seule fois : autant la payer ici plutôt qu'au premier téléchargement.
    let _ = tools::version(&exe, "yt-dlp").await;
    if app.exists() {
        std::fs::remove_dir_all(&app)?;
    }
    std::fs::rename(&tmp, &app)?;
    Ok(rel.tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_sums() {
        let s = "aaaa  yt-dlp\nBBBB *yt-dlp_macos\ncccc  yt-dlp_macos.zip\n";
        assert_eq!(expected_hash(s, "yt-dlp_macos").as_deref(), Some("bbbb"));
        assert_eq!(expected_hash(s, "absent"), None);
    }
}
