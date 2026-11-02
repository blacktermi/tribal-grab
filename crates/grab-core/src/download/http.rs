//! Téléchargement HTTP avec plusieurs connexions (`Range`), reprise et limite de débit.
use super::{Ctx, PartDir};
use crate::error::{Error, Result};
use crate::extract::direct::{Head, probe};
use crate::net::NEUTRAL_UA;
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

pub const CHUNK: u64 = 4 * 1024 * 1024;
const ATTEMPTS: u32 = 5;
const SAVE_EVERY: Duration = Duration::from_secs(3);
/// Attentes de la dernière connexion quand le serveur reste saturé : environ 3 minutes au total.
const BUSY_WAITS: u32 = 8;

/// Télécharge `url` vers `<partdir>/<key>.bin` et retourne ce chemin.
pub async fn download(ctx: &Ctx, part: &Arc<PartDir>, key: &str, url: &str, headers: &BTreeMap<String, String>) -> Result<PathBuf> {
    ctx.check()?;
    let out = part.path(&format!("{key}.bin"));
    let mut headers = headers.clone();
    let head = match probe_patiently(ctx, url, &headers).await {
        Ok(h) => h,
        Err(_) => {
            let mut neutral = headers.clone();
            neutral.insert("User-Agent".into(), NEUTRAL_UA.into());
            match probe_patiently(ctx, url, &neutral).await {
                Ok(h) => {
                    headers = neutral;
                    h
                }
                // Certains serveurs refusent la sonde par plage mais servent très bien le fichier entier.
                Err(_) => Head::default(),
            }
        }
    };
    let total = head.length.unwrap_or(0);
    let fp = format!("{total}-{}", head.validator.clone().unwrap_or_default());
    if !head.ranges || total == 0 {
        return single(ctx, part, key, url, &headers, &out).await;
    }
    let state = part.stream(key, &fp, total, CHUNK);
    ctx.meter.total.store(total, Ordering::Relaxed);
    if state.complete && out.metadata().map(|m| m.len() == total).unwrap_or(false) {
        ctx.meter.done.store(total, Ordering::Relaxed);
        return Ok(out);
    }
    // Préallocation, puis octets déjà reçus par les parties terminées.
    {
        let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&out)?;
        if f.metadata()?.len() != total {
            f.set_len(total)?;
        }
    }
    let n_chunks = total.div_ceil(CHUNK) as u32;
    let already = state.done.iter().map(|&i| chunk_len(i, total)).sum::<u64>() + state.partial.values().sum::<u64>();
    ctx.meter.done.store(already, Ordering::Relaxed);

    let queue: VecDeque<u32> = (0..n_chunks).filter(|i| !state.done.contains(i)).collect();
    let queue = Arc::new(Mutex::new(queue));
    let workers = ctx.connections.max(1);
    let alive = Arc::new(AtomicUsize::new(workers));
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..workers {
        let (ctx, part, queue, alive) = (ctx.clone(), part.clone(), queue.clone(), alive.clone());
        let (key, url, headers, out) = (key.to_string(), url.to_string(), headers.clone(), out.clone());
        set.spawn(async move {
            let mut file = tokio::fs::OpenOptions::new().write(true).open(&out).await?;
            let mut waits = 0u32;
            loop {
                let Some(i) = queue.lock().unwrap().pop_front() else {
                    alive.fetch_sub(1, Ordering::SeqCst);
                    return Ok::<(), Error>(());
                };
                let have = part.partial_of(&key, i);
                match fetch_chunk(&ctx, &part, &key, &mut file, &url, &headers, i, total, have).await {
                    Ok(()) => {
                        waits = 0;
                        part.mark_done(&key, i);
                        if i % 8 == 0 {
                            let _ = part.save();
                        }
                    }
                    // Le serveur limite les connexions : celle-ci s'efface et les autres finissent le travail.
                    Err(e) if is_busy(&e) => {
                        queue.lock().unwrap().push_front(i);
                        if alive.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |a| (a > 1).then(|| a - 1)).is_ok() {
                            return Ok(());
                        }
                        waits += 1;
                        if waits > BUSY_WAITS {
                            return Err(e);
                        }
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(1 << waits.min(5))) => {}
                            _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
                        }
                    }
                    Err(e) => return Err(e),
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
    if !part.all_done(key, n_chunks) {
        return Err(Error::Network("téléchargement incomplet".into()));
    }
    std::fs::OpenOptions::new().write(true).open(&out)?.sync_all()?;
    part.set_complete(key)?;
    Ok(out)
}

/// 429 et 503 : le serveur demande de ralentir, le fichier reste disponible.
fn is_busy(e: &Error) -> bool {
    matches!(e, Error::Network(m) if m.contains("429") || m.contains("503"))
}

/// Attend quand le serveur est saturé ; toute autre erreur remonte tout de suite.
async fn probe_patiently(ctx: &Ctx, url: &str, headers: &BTreeMap<String, String>) -> Result<Head> {
    for wait in [2, 4, 8] {
        match probe(&ctx.net, url, headers).await {
            Err(e) if is_busy(&e) => {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
                    _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
                }
            }
            other => return other,
        }
    }
    probe(&ctx.net, url, headers).await
}

fn chunk_len(i: u32, total: u64) -> u64 {
    let start = i as u64 * CHUNK;
    CHUNK.min(total.saturating_sub(start))
}

/// Écrit la partie `i` au fil de l'eau : tout octet reçu reste acquis après une coupure, une pause ou un arrêt.
#[allow(clippy::too_many_arguments)]
async fn fetch_chunk(ctx: &Ctx, part: &Arc<PartDir>, key: &str, file: &mut tokio::fs::File, url: &str, headers: &BTreeMap<String, String>, i: u32, total: u64, mut have: u64) -> Result<()> {
    let start = i as u64 * CHUNK;
    let len = chunk_len(i, total);
    let mut last_err = Error::Network("échec du téléchargement".into());
    for attempt in 0..ATTEMPTS {
        ctx.check()?;
        if have >= len {
            return Ok(());
        }
        if attempt > 0 {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1 << attempt.min(4))) => {}
                _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
            }
        }
        let r: Result<()> = async {
            let resp = ctx.net.request(url, headers).header("Range", format!("bytes={}-{}", start + have, start + len - 1)).send().await?;
            let st = resp.status();
            if st.as_u16() != 206 {
                return Err(Error::Network(format!("le serveur a répondu {st} à une requête par plage")));
            }
            let mut resp = resp;
            file.seek(std::io::SeekFrom::Start(start + have)).await?;
            let mut saved = Instant::now();
            loop {
                let c = tokio::select! {
                    c = resp.chunk() => c?,
                    _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
                };
                let Some(c) = c else { break };
                if have + c.len() as u64 > len {
                    return Err(Error::Network("réponse plus longue que la partie demandée".into()));
                }
                ctx.throttle.wait(c.len()).await;
                file.write_all(&c).await?;
                have += c.len() as u64;
                ctx.meter.add(c.len() as u64);
                if saved.elapsed() >= SAVE_EVERY {
                    file.flush().await?;
                    part.set_partial(key, i, have);
                    let _ = part.save();
                    saved = Instant::now();
                }
            }
            if have != len {
                return Err(Error::Network("réponse incomplète du serveur".into()));
            }
            Ok(())
        }
        .await;
        file.flush().await?;
        part.set_partial(key, i, have);
        match r {
            Ok(()) => return Ok(()),
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(e) => {
                // Un refus (URL expirée, interdit, trop de connexions) ne sert à rien de le réessayer ici.
                if is_busy(&e) {
                    return Err(e);
                }
                if let Error::Network(m) = &e
                    && (m.contains("403") || m.contains("404") || m.contains("410")) {
                        return Err(e);
                    }
                last_err = e;
            }
        }
    }
    Err(last_err)
}

/// Serveur sans plages : un seul flux. Une coupure oblige à repartir de zéro, mais ne fait pas échouer le téléchargement.
async fn single(ctx: &Ctx, part: &Arc<PartDir>, key: &str, url: &str, headers: &BTreeMap<String, String>, out: &PathBuf) -> Result<PathBuf> {
    part.stream(key, "single", 0, 0);
    let mut last_err = Error::Network("échec du téléchargement".into());
    for attempt in 0..ATTEMPTS {
        ctx.check()?;
        if attempt > 0 {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1 << attempt.min(4))) => {}
                _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
            }
        }
        match single_once(ctx, url, headers, out).await {
            Ok(()) => {
                part.set_complete(key)?;
                return Ok(out.clone());
            }
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(e) => {
                if let Error::Network(m) = &e
                    && (m.contains("403") || m.contains("404") || m.contains("410")) {
                        return Err(e);
                    }
                last_err = e;
            }
        }
    }
    Err(last_err)
}

async fn single_once(ctx: &Ctx, url: &str, headers: &BTreeMap<String, String>, out: &PathBuf) -> Result<()> {
    ctx.meter.done.store(0, Ordering::Relaxed);
    let mut resp = ctx.net.request(url, headers).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Network(format!("le serveur a répondu {}", resp.status())));
    }
    ctx.meter.total.store(resp.content_length().unwrap_or(0), Ordering::Relaxed);
    let mut f = tokio::fs::File::create(out).await?;
    loop {
        let c = tokio::select! {
            c = resp.chunk() => c?,
            _ = ctx.cancel.cancelled() => return Err(Error::Cancelled),
        };
        let Some(c) = c else { break };
        ctx.throttle.wait(c.len()).await;
        ctx.meter.add(c.len() as u64);
        f.write_all(&c).await?;
    }
    f.sync_all().await?;
    Ok(())
}
