//! Téléchargeurs natifs. Un téléchargement partiel vit dans `<nom>.tgpart/` :
//! `manifest.json` (jamais d'URL ni de cookie) et les parties.
pub mod http;
pub mod live;
pub mod segments;
pub mod ytdlp_dl;

use crate::error::{Error, Result};
use crate::net::Net;
use crate::util::{Meter, Throttle};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Ctx {
    pub net: Net,
    pub meter: Arc<Meter>,
    pub throttle: Arc<Throttle>,
    pub cancel: CancellationToken,
    pub connections: usize,
}

impl Ctx {
    pub fn check(&self) -> Result<()> {
        if self.cancel.is_cancelled() { Err(Error::Cancelled) } else { Ok(()) }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamState {
    pub fingerprint: String,
    pub total: u64,
    pub chunk: u64,
    pub done: BTreeSet<u32>,
    /// Octets déjà écrits des parties commencées : une pause ne perd rien, même sur une connexion lente.
    #[serde(default)]
    pub partial: BTreeMap<u32, u64>,
    pub complete: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    title: String,
    streams: BTreeMap<String, StreamState>,
}

pub struct PartDir {
    pub dir: PathBuf,
    m: Mutex<Manifest>,
}

impl PartDir {
    pub fn open(dest: &Path, title: &str) -> Result<PartDir> {
        let dir = crate::naming::part_dir(dest);
        std::fs::create_dir_all(&dir)?;
        let mut m: Manifest = std::fs::read_to_string(dir.join("manifest.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        m.version = 1;
        m.title = title.to_string();
        let p = PartDir { dir, m: Mutex::new(m) };
        p.save()?;
        Ok(p)
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_vec_pretty(&*self.m.lock().unwrap())?;
        crate::util::write_atomic(&self.dir.join("manifest.json"), &json)
    }

    /// État d'un flux. Si l'empreinte a changé (fichier modifié côté serveur), on repart de zéro.
    pub fn stream(&self, key: &str, fingerprint: &str, total: u64, chunk: u64) -> StreamState {
        let mut m = self.m.lock().unwrap();
        let st = m.streams.entry(key.to_string()).or_default();
        if st.fingerprint != fingerprint || st.chunk != chunk || st.total != total {
            if let Ok(rd) = std::fs::read_dir(&self.dir) {
                for e in rd.flatten() {
                    let n = e.file_name().to_string_lossy().into_owned();
                    if n.starts_with(&format!("{key}.")) {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
            *st = StreamState { fingerprint: fingerprint.into(), total, chunk, ..Default::default() };
        }
        st.clone()
    }

    pub fn mark_done(&self, key: &str, idx: u32) {
        if let Some(s) = self.m.lock().unwrap().streams.get_mut(key) {
            s.done.insert(idx);
            s.partial.remove(&idx);
        }
    }

    pub fn partial_of(&self, key: &str, idx: u32) -> u64 {
        self.m.lock().unwrap().streams.get(key).and_then(|s| s.partial.get(&idx).copied()).unwrap_or(0)
    }

    pub fn all_done(&self, key: &str, chunks: u32) -> bool {
        self.m.lock().unwrap().streams.get(key).is_some_and(|s| s.done.len() == chunks as usize)
    }

    pub fn set_partial(&self, key: &str, idx: u32, bytes: u64) {
        if let Some(s) = self.m.lock().unwrap().streams.get_mut(key) {
            s.partial.insert(idx, bytes);
        }
    }

    pub fn set_complete(&self, key: &str) -> Result<()> {
        if let Some(s) = self.m.lock().unwrap().streams.get_mut(key) {
            s.complete = true;
        }
        self.save()
    }
}

/// Termine proprement : déplace le fichier assemblé vers sa destination (même volume : atomique).
pub fn finalize_move(src: &Path, dest: &Path) -> Result<()> {
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    if std::fs::rename(src, dest).is_err() {
        // Volume différent : copie vers un fichier temporaire voisin, puis renommage.
        let tmp = dest.with_extension("tg-tmp");
        std::fs::copy(src, &tmp)?;
        std::fs::rename(&tmp, dest)?;
        let _ = std::fs::remove_file(src);
    }
    Ok(())
}
