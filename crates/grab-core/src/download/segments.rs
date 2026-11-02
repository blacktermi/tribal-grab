//! Téléchargement de segments (HLS, DASH, fragments) en parallèle, avec reprise et déchiffrement AES-128.
use super::{Ctx, PartDir};
use crate::error::{Error, Result};
use crate::model::{Segment, SegmentPlan};
use crate::util::MAX_MANIFEST_BYTES;
use cbc::cipher::{BlockModeDecrypt, KeyIvInit, block_padding::Pkcs7};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ATTEMPTS: u32 = 4;
const MAX_SEGMENT: usize = 256 * 1024 * 1024;

type KeyCache = Arc<Mutex<HashMap<String, [u8; 16]>>>;

/// Télécharge toutes les parties, puis les assemble dans `<partdir>/<key>.bin`.
pub async fn download(ctx: &Ctx, part: &Arc<PartDir>, key: &str, plan: &SegmentPlan, headers: &BTreeMap<String, String>) -> Result<PathBuf> {
    ctx.check()?;
    if plan.is_live {
        return Err(Error::Unsupported("flux en direct".into()));
    }
    let items: Vec<Segment> = plan.init.iter().cloned().chain(plan.segments.iter().cloned()).collect();
    if items.is_empty() {
        return Err(Error::Extract("aucun segment à télécharger".into()));
    }
    let out = part.path(&format!("{key}.bin"));
    let n = items.len() as u32;
    let state = part.stream(key, &format!("seg-{n}"), n as u64, 0);
    if state.complete && out.exists() {
        return Ok(out);
    }
    let seg_path = |i: u32| part.path(&format!("{key}.seg-{i:06}"));
    // Reprise : on ne garde que les parties dont le fichier existe vraiment.
    let mut done: Vec<bool> = (0..n).map(|i| state.done.contains(&i) && seg_path(i).exists()).collect();
    let bytes_done: u64 = (0..n).filter(|&i| done[i as usize]).map(|i| seg_path(i).metadata().map(|m| m.len()).unwrap_or(0)).sum();
    let count_done = done.iter().filter(|d| **d).count() as u64;
    ctx.meter.done.store(bytes_done, Ordering::Relaxed);
    let finished = Arc::new(AtomicU64::new(count_done));
    let update_total = {
        let (meter, finished) = (ctx.meter.clone(), finished.clone());
        move || {
            let c = finished.load(Ordering::Relaxed).max(1);
            meter.total.store(meter.done.load(Ordering::Relaxed) / c * n as u64, Ordering::Relaxed);
        }
    };
    update_total();

    let keys: KeyCache = Arc::default();
    let items = Arc::new(items);
    let done_flags = Arc::new(std::mem::take(&mut done));
    let next = Arc::new(AtomicU32::new(0));
    let workers = ctx.connections.clamp(4, 16);
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..workers {
        let (ctx, part, items, done_flags, next, keys, finished) = (ctx.clone(), part.clone(), items.clone(), done_flags.clone(), next.clone(), keys.clone(), finished.clone());
        let (headers, key, update_total) = (headers.clone(), key.to_string(), update_total.clone());
        set.spawn(async move {
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    return Ok::<(), Error>(());
                }
                if done_flags[i as usize] {
                    continue;
                }
                let data = fetch_segment(&ctx, &items[i as usize], &headers, &keys).await?;
                let path = part.path(&format!("{key}.seg-{i:06}"));
                let tmp = path.with_file_name(format!("{}.tmp", path.file_name().unwrap().to_string_lossy()));
                tokio::fs::write(&tmp, &data).await?;
                tokio::fs::rename(&tmp, &path).await?;
                part.mark_done(&key, i);
                finished.fetch_add(1, Ordering::Relaxed);
                update_total();
                if i % 20 == 0 {
                    let _ = part.save();
                }
            }
        });
    }
    let mut failure = None;
    while let Some(r) = set.join_next().await {
        match r {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                failure.get_or_insert(e);
                set.abort_all();
            }
            Err(e) if e.is_cancelled() => {}
            Err(e) => {
                failure.get_or_insert(Error::Other(e.to_string()));
            }
        }
    }
    let _ = part.save();
    if let Some(e) = failure {
        return Err(e);
    }
    ctx.check()?;
    // Assemblage dans l'ordre.
    let tmp = out.with_extension("assembling");
    {
        use std::io::Write;
        let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        for i in 0..n {
            let mut r = std::fs::File::open(seg_path(i))?;
            std::io::copy(&mut r, &mut w)?;
        }
        w.flush()?;
        w.get_ref().sync_all()?;
    }
    std::fs::rename(&tmp, &out)?;
    for i in 0..n {
        let _ = std::fs::remove_file(seg_path(i));
    }
    ctx.meter.total.store(out.metadata()?.len(), Ordering::Relaxed);
    ctx.meter.done.store(out.metadata()?.len(), Ordering::Relaxed);
    part.set_complete(key)?;
    Ok(out)
}

async fn fetch_key(ctx: &Ctx, url: &str, headers: &BTreeMap<String, String>, cache: &KeyCache) -> Result<[u8; 16]> {
    if let Some(k) = cache.lock().unwrap().get(url) {
        return Ok(*k);
    }
    let b = ctx.net.fetch_bytes(url, headers, 64 * 1024).await?;
    let k: [u8; 16] = b.as_slice().try_into().map_err(|_| Error::Extract("clé de chiffrement HLS invalide (16 octets attendus)".into()))?;
    cache.lock().unwrap().insert(url.to_string(), k);
    Ok(k)
}

pub fn decrypt(key: &[u8; 16], iv: &[u8; 16], data: &[u8]) -> Result<Vec<u8>> {
    let dec = cbc::Decryptor::<aes::Aes128>::new_from_slices(key, iv).map_err(|_| Error::Extract("clé AES invalide".into()))?;
    dec.decrypt_padded_vec::<Pkcs7>(data).map_err(|_| Error::Extract("déchiffrement AES-128 impossible (données ou clé incorrectes)".into()))
}

async fn fetch_segment(ctx: &Ctx, seg: &Segment, headers: &BTreeMap<String, String>, keys: &KeyCache) -> Result<Vec<u8>> {
    let mut last = Error::Network("échec du téléchargement d'un segment".into());
    for attempt in 0..ATTEMPTS {
        ctx.check()?;
        if attempt > 0 {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1 << attempt)) => {}
                _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
            }
        }
        let mut added = 0u64;
        let r: Result<Vec<u8>> = async {
            let mut rb = ctx.net.request(&seg.url, headers);
            if let Some((a, b)) = seg.range {
                rb = rb.header("Range", format!("bytes={a}-{b}"));
            }
            let mut resp = rb.send().await?;
            if !resp.status().is_success() {
                return Err(Error::Network(format!("le serveur a répondu {}", resp.status())));
            }
            let mut buf = Vec::new();
            loop {
                let c = tokio::select! {
                    c = resp.chunk() => c?,
                    _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
                };
                let Some(c) = c else { break };
                ctx.throttle.wait(c.len()).await;
                ctx.meter.add(c.len() as u64);
                added += c.len() as u64;
                buf.extend_from_slice(&c);
                if buf.len() > MAX_SEGMENT {
                    return Err(Error::Network("segment anormalement gros".into()));
                }
            }
            match &seg.key {
                Some(k) => decrypt(&fetch_key(ctx, &k.url, headers, keys).await?, &k.iv, &buf),
                None => Ok(buf),
            }
        }
        .await;
        match r {
            Ok(b) => return Ok(b),
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(e) => {
                ctx.meter.done.fetch_sub(added, Ordering::Relaxed);
                last = e;
            }
        }
    }
    Err(last)
}

/// Télécharge une playlist HLS (média) : récupère la liste puis les segments.
pub async fn download_hls(ctx: &Ctx, part: &Arc<PartDir>, key: &str, playlist_url: &str, headers: &BTreeMap<String, String>) -> Result<PathBuf> {
    let text = ctx.net.fetch_text(playlist_url, headers, MAX_MANIFEST_BYTES).await?;
    let plan = crate::extract::hls::build_plan(playlist_url, &text)?;
    download(ctx, part, key, &plan, headers).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use cbc::cipher::BlockModeEncrypt;

    #[test]
    fn aes_roundtrip() {
        let (key, iv) = ([7u8; 16], [9u8; 16]);
        let enc = cbc::Encryptor::<aes::Aes128>::new_from_slices(&key, &iv).unwrap().encrypt_padded_vec::<Pkcs7>(b"segment de test");
        assert_eq!(decrypt(&key, &iv, &enc).unwrap(), b"segment de test");
        assert!(decrypt(&[1u8; 16], &iv, &enc).is_err() || decrypt(&[1u8; 16], &iv, &enc).unwrap() != b"segment de test");
    }
}
