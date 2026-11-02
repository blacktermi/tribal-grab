//! Tests du moteur contre un serveur HTTP local (plages, coupures, HLS, DASH, doublons, sous-titres).
mod common;
use common::*;
use grab_core::*;
use sha2::{Digest, Sha256};

fn random_bytes(n: usize) -> Vec<u8> {
    let mut x: u64 = 0x9E3779B97F4A7C15;
    (0..n).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; (x >> 24) as u8 }).collect()
}

fn sha(b: &[u8]) -> String { hex::encode(Sha256::digest(b)) }

#[tokio::test(flavor = "multi_thread")]
async fn http_resume_after_cut_downloads_only_missing_chunks() {
    let srv = serve().await;
    let data = random_bytes(14 * 1024 * 1024); // 4 parties de 4 Mio
    srv.add("/big.mp4", data.clone(), "video/mp4");
    let e = env_with(|s| { s.connections = 1; s.embed_metadata = false; s.post_mode = PostMode::None; }).await;
    // Le serveur coupe (403) à partir de la 3e plage : 2 parties arrivent, le reste échoue.
    srv.shared.deny_after.store(2, std::sync::atomic::Ordering::SeqCst);
    let id = e.engine.add_url(&srv.url("/big.mp4")).unwrap();
    let t = done(&e.engine, id).await;
    assert_eq!(t.state, TaskState::Failed);
    let part = std::path::PathBuf::from(t.target.clone().unwrap()).with_extension("mp4.tgpart");
    let manifest = std::fs::read_to_string(part.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"done\""), "{manifest}");
    assert!(!manifest.contains("http"), "le manifeste ne doit contenir aucune URL");
    assert!(files_in(&e.out).iter().all(|f| !f.ends_with(".mp4")), "pas de fichier final tant que c'est incomplet");
    // Le serveur revient : reprise.
    srv.shared.deny_after.store(usize::MAX, std::sync::atomic::Ordering::SeqCst);
    srv.shared.deny.store(false, std::sync::atomic::Ordering::SeqCst);
    srv.shared.ranges.lock().unwrap().clear();
    e.engine.resume(id).unwrap();
    let t = finished(&e.engine, id).await;
    let out = std::fs::read(t.output_path.unwrap()).unwrap();
    assert_eq!(sha(&out), sha(&data));
    let got: Vec<u64> = srv.shared.ranges.lock().unwrap().iter().map(|r| r.0).collect();
    assert!(!got.contains(&0) && !got.contains(&(4 * 1024 * 1024)), "les parties déjà reçues ne sont pas redemandées : {got:?}");
    assert!(!part.exists(), "le dossier partiel est supprimé à la fin");
}

#[tokio::test(flavor = "multi_thread")]
async fn parallel_range_download_is_exact() {
    let srv = serve().await;
    let data = random_bytes(9 * 1024 * 1024 + 123);
    srv.add("/p.mp4", data.clone(), "video/mp4");
    let e = env_with(|s| { s.connections = 4; s.embed_metadata = false; s.post_mode = PostMode::None; }).await;
    let id = e.engine.add_url(&srv.url("/p.mp4")).unwrap();
    let t = finished(&e.engine, id).await;
    assert_eq!(sha(&std::fs::read(t.output_path.unwrap()).unwrap()), sha(&data));
}

#[tokio::test(flavor = "multi_thread")]
async fn hls_aes128_vod_is_decrypted_and_playable() {
    if !have_ffmpeg() { return; }
    let srv = serve().await;
    let d = tempfile::tempdir().unwrap();
    make_hls(d.path(), true);
    srv.add_dir(d.path(), "/h");
    let e = env().await;
    let id = e.engine.add_url(&srv.url("/h/index.m3u8")).unwrap();
    let t = finished(&e.engine, id).await;
    let p = probe_file(std::path::Path::new(&t.output_path.unwrap())).await;
    assert_eq!(p.vcodec.as_deref(), Some("h264"));
    assert_eq!(p.acodec.as_deref(), Some("aac"));
    assert!((p.duration.unwrap() - 6.0).abs() < 1.0);
}

#[tokio::test(flavor = "multi_thread")]
async fn dash_separate_audio_and_video_are_merged() {
    if !have_ffmpeg() { return; }
    let srv = serve().await;
    let d = tempfile::tempdir().unwrap();
    make_dash(d.path());
    srv.add_dir(d.path(), "/d");
    let e = env().await;
    let id = e.engine.add_url(&srv.url("/d/manifest.mpd")).unwrap();
    let t = finished(&e.engine, id).await;
    let p = probe_file(std::path::Path::new(&t.output_path.unwrap())).await;
    assert_eq!(p.vcodec.as_deref(), Some("h264"));
    assert_eq!(p.acodec.as_deref(), Some("aac"));
    assert!((p.duration.unwrap() - 6.0).abs() < 1.0);
}

#[tokio::test(flavor = "multi_thread")]
async fn duplicates_ask_skip_and_redownload() {
    if !have_ffmpeg() { return; }
    let srv = serve().await;
    let d = tempfile::tempdir().unwrap();
    make_mp4(&d.path().join("a.mp4"));
    srv.add("/a.mp4", std::fs::read(d.path().join("a.mp4")).unwrap(), "video/mp4");
    let e = env_with(|s| s.duplicate_policy = DuplicatePolicy::Ask).await;
    let id = e.engine.add_url(&srv.url("/a.mp4")).unwrap();
    finished(&e.engine, id).await;
    let id2 = e.engine.add_url(&srv.url("/a.mp4")).unwrap();
    let t = wait_until(&e.engine, id2, 30, "question doublon", |t| t.state == TaskState::NeedsInput).await;
    assert!(matches!(t.question, Some(Question::Duplicate { .. })));
    e.engine.answer(id2, Answer::Duplicate { action: DuplicateAction::Skip }).unwrap();
    assert_eq!(e.engine.task(id2).unwrap().state, TaskState::Cancelled);
    let id3 = e.engine.add_url(&srv.url("/a.mp4")).unwrap();
    wait_until(&e.engine, id3, 30, "question doublon", |t| t.state == TaskState::NeedsInput).await;
    e.engine.answer(id3, Answer::Duplicate { action: DuplicateAction::Redownload }).unwrap();
    finished(&e.engine, id3).await;
    assert_eq!(files_in(&e.out), ["a (1).mp4", "a.mp4"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn remote_name_cannot_leave_destination() {
    let srv = serve().await;
    srv.add_with_headers("/dl.mp4", random_bytes(2000), "video/mp4", vec![("Content-Disposition".into(), "attachment; filename=\"../../evil.mp4\"".into())]);
    let e = env_with(|s| { s.embed_metadata = false; s.post_mode = PostMode::None; }).await;
    let id = e.engine.add_url(&srv.url("/dl.mp4")).unwrap();
    let t = finished(&e.engine, id).await;
    let p = std::path::PathBuf::from(t.output_path.unwrap());
    assert_eq!(p.parent().unwrap(), e.out.as_path());
    assert!(!e.dir.path().join("evil.mp4").exists() && !e.out.parent().unwrap().join("evil.mp4").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn integration_subtitles_embed_and_mp3() {
    if !have_ffmpeg() { return; }
    let srv = serve().await;
    let d = tempfile::tempdir().unwrap();
    make_mp4(&d.path().join("c.mp4"));
    srv.add("/clip.mp4", std::fs::read(d.path().join("c.mp4")).unwrap(), "video/mp4");
    srv.add("/s-fr.vtt", b"WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nBonjour\n".to_vec(), "text/vtt");
    srv.add("/s-en.vtt", b"WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello\n".to_vec(), "text/vtt");
    let e = env_with(|s| { s.sub_langs = vec!["fr".into(), "en".into()]; s.sub_multiple = true; }).await;
    let code = format!("grab.result({{title: 'Clip', formats: [{{url: '{b}/clip.mp4', height: 180}}], subtitles: [{{lang: 'fr', url: '{b}/s-fr.vtt'}}, {{lang: 'en', url: '{b}/s-en.vtt'}}]}});", b = srv.base);
    let tgi = serde_json::json!({"name": "Local", "url_regex": "^http://127\\.0\\.0\\.1:\\d+/page", "code": code, "enabled": true}).to_string();
    e.engine.install_integration(&tgi).unwrap();
    // 1. Fichiers .srt à côté de la vidéo
    let mut o = TaskOptions { sub_mode: Some(SubMode::IfAvailable), name_template: Some("{title}-a".into()), ..Default::default() };
    let id = e.engine.add(AddRequest { url: srv.url("/page/1"), options: o.clone(), ..Default::default() }).unwrap();
    finished(&e.engine, id).await;
    let srt = std::fs::read_to_string(e.out.join("Clip-a.fr.srt")).unwrap_or_else(|_| panic!("fichiers : {:?}", files_in(&e.out)));
    assert!(srt.contains("00:00:01,000 --> 00:00:02,000") && srt.contains("Bonjour"));
    assert!(e.out.join("Clip-a.en.srt").exists());
    // 2. Intégrés au MP4
    let mut s = e.engine.settings();
    s.sub_out = SubOut::Embed;
    e.engine.set_settings(s).unwrap();
    o.name_template = Some("{title}-b".into());
    let id = e.engine.add(AddRequest { url: srv.url("/page/2"), options: o.clone(), ..Default::default() }).unwrap();
    let t = finished(&e.engine, id).await;
    let out = std::process::Command::new(FFPROBE).args(["-v", "error", "-select_streams", "s", "-show_entries", "stream=codec_name", "-of", "csv=p=0", &t.output_path.unwrap()]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.contains("mov_text")).count(), 2);
    // 3. MP3
    o.sub_mode = Some(SubMode::None);
    o.audio_format = Some(AudioFormat::Mp3);
    o.name_template = Some("{title}-c".into());
    let id = e.engine.add(AddRequest { url: srv.url("/page/3"), options: o.clone(), ..Default::default() }).unwrap();
    let t = finished(&e.engine, id).await;
    let p = e.out.join("Clip-c.mp3");
    assert_eq!(t.output_path.unwrap(), p.display().to_string());
    assert_eq!(probe_file(&p).await.acodec.as_deref(), Some("mp3"));
    // 4. Sous-titres seuls
    o.audio_format = None;
    o.sub_mode = Some(SubMode::Only);
    o.name_template = Some("{title}-d".into());
    let id = e.engine.add(AddRequest { url: srv.url("/page/4"), options: o, ..Default::default() }).unwrap();
    finished(&e.engine, id).await;
    assert!(e.out.join("Clip-d.fr.vtt").exists() || e.out.join("Clip-d.fr.srt").exists());
    assert!(!e.out.join("Clip-d.mp4").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn drm_hls_is_refused_but_clear_tracks_remain() {
    let srv = serve().await;
    srv.add("/w.m3u8", b"#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n#EXTINF:4,\ns0.ts\n#EXT-X-ENDLIST\n".to_vec(), "application/vnd.apple.mpegurl");
    let e = env().await;
    let id = e.engine.add_url(&srv.url("/w.m3u8")).unwrap();
    let t = done(&e.engine, id).await;
    assert_eq!(t.state, TaskState::Failed);
    assert!(t.error.unwrap().contains("DRM"));
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_non_http_and_restores_queue_after_restart() {
    let e = env().await;
    assert!(e.engine.add_url("file:///etc/passwd").is_err());
    assert!(e.engine.add_text("rien ici").is_err());
    let ids = e.engine.add_text("voir https://exemple.test/a.mp4, et (https://exemple.test/b.mp4).").unwrap();
    assert_eq!(ids.len(), 2);
    e.engine.pause_all();
    let mut s = e.engine.settings();
    s.max_concurrent = 7;
    e.engine.set_settings(s).unwrap();
    let data = e.dir.path().join("data");
    drop(e.engine);
    let e2 = Engine::start(&data).await.unwrap();
    assert_eq!(e2.tasks(SortKey::Order, false).len(), 2);
    assert_eq!(e2.settings().max_concurrent, 7);
}

#[tokio::test(flavor = "multi_thread")]
async fn any_file_type_is_saved_untouched_under_its_real_name() {
    let srv = serve().await;
    let e = env().await;
    for (path, ctype, name) in [
        ("/f/Mon%20archive.tar.gz", "application/gzip", "Mon archive.tar.gz"),
        ("/f/rapport.pdf", "application/pdf", "rapport.pdf"),
        ("/f/installeur.dmg", "application/octet-stream", "installeur.dmg"),
        ("/f/plan.sketch", "application/octet-stream", "plan.sketch"),
    ] {
        let data = random_bytes(9 * 1024 * 1024);
        srv.add(path, data.clone(), ctype);
        let id = e.engine.add_url(&srv.url(path)).unwrap();
        let t = finished(&e.engine, id).await;
        let out = std::path::PathBuf::from(t.output_path.unwrap());
        assert_eq!(out.file_name().unwrap().to_string_lossy(), name);
        assert_eq!(sha(&std::fs::read(&out).unwrap()), sha(&data));
        assert_eq!(t.progress.total, Some(data.len() as u64));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn attachment_without_extension_uses_server_name() {
    let srv = serve().await;
    let data = random_bytes(1024);
    srv.add_with_headers("/download", data.clone(), "text/html", vec![("Content-Disposition".into(), "attachment; filename=\"facture 12.pdf\"".into())]);
    let e = env().await;
    let id = e.engine.add_url(&srv.url("/download")).unwrap();
    let t = finished(&e.engine, id).await;
    assert!(t.output_path.unwrap().ends_with("facture 12.pdf"));
}

#[tokio::test(flavor = "multi_thread")]
async fn server_without_ranges_survives_failures() {
    let srv = serve().await;
    let data = random_bytes(6 * 1024 * 1024);
    srv.add("/s.zip", data.clone(), "application/zip");
    srv.shared.no_range.store(true, std::sync::atomic::Ordering::SeqCst);
    let e = env().await;
    // Deux refus : la sonde, puis un téléchargement. Le suivant doit passer.
    srv.shared.fail_next.store(2, std::sync::atomic::Ordering::SeqCst);
    let id = e.engine.add_url(&srv.url("/s.zip")).unwrap();
    let t = finished(&e.engine, id).await;
    assert_eq!(sha(&std::fs::read(t.output_path.unwrap()).unwrap()), sha(&data));
}

#[tokio::test(flavor = "multi_thread")]
async fn pause_keeps_received_data_and_resume_finishes_the_same_file() {
    let srv = serve().await;
    let data = random_bytes(40 * 1024 * 1024); // 10 parties
    srv.add("/gros.zip", data.clone(), "application/zip");
    srv.shared.delay_ms.store(150, std::sync::atomic::Ordering::SeqCst);
    let e = env_with(|s| s.connections = 2).await;
    let id = e.engine.add_url(&srv.url("/gros.zip")).unwrap();
    let t = wait_until(&e.engine, id, 30, "progression", |t| t.state == TaskState::Downloading && t.progress.downloaded >= 8 * 1024 * 1024).await;
    assert_eq!(t.progress.total, Some(data.len() as u64), "la taille totale est connue pendant le téléchargement");
    assert!(t.progress.percent > 0.0 && t.progress.percent < 100.0);
    e.engine.pause(id).unwrap();
    let t = wait_until(&e.engine, id, 10, "pause", |t| t.state == TaskState::Paused).await;
    assert!(t.output_path.is_none());
    let before = srv.shared.ranges.lock().unwrap().len();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(srv.shared.ranges.lock().unwrap().len(), before, "plus aucune requête pendant la pause");
    let received: Vec<u64> = srv.shared.ranges.lock().unwrap().iter().map(|r| r.0).collect();
    srv.shared.ranges.lock().unwrap().clear();
    srv.shared.delay_ms.store(0, std::sync::atomic::Ordering::SeqCst);
    e.engine.resume(id).unwrap();
    let t = finished(&e.engine, id).await;
    assert_eq!(sha(&std::fs::read(t.output_path.unwrap()).unwrap()), sha(&data));
    let again = srv.shared.ranges.lock().unwrap().len();
    assert!(again < 10, "la reprise ne repart pas de zéro : {again} parties redemandées, {} déjà reçues", received.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn cut_in_the_middle_of_a_part_keeps_every_received_byte() {
    let srv = serve().await;
    let data = random_bytes(3 * 1024 * 1024); // moins qu'une partie de 4 Mio
    srv.add("/c.zip", data.clone(), "application/zip");
    srv.shared.cut_next.store(1_500_000, std::sync::atomic::Ordering::SeqCst);
    let e = env_with(|s| s.connections = 1).await;
    let id = e.engine.add_url(&srv.url("/c.zip")).unwrap();
    let t = finished(&e.engine, id).await;
    assert_eq!(sha(&std::fs::read(t.output_path.unwrap()).unwrap()), sha(&data));
    let starts: Vec<u64> = srv.shared.ranges.lock().unwrap().iter().map(|r| r.0).collect();
    assert!(starts.iter().any(|&s| s > 0 && s < data.len() as u64), "la reprise repart de l'octet reçu : {starts:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn server_limiting_connections_is_slowed_down_for_not_failed() {
    let srv = serve().await;
    let data = random_bytes(21 * 1024 * 1024); // 6 parties
    srv.add("/limite.zip", data.clone(), "application/zip");
    let e = env_with(|s| s.connections = 8).await;
    // Comme un serveur qui répond 429 au-delà de quelques connexions.
    srv.shared.fail_next.store(7, std::sync::atomic::Ordering::SeqCst);
    let id = e.engine.add_url(&srv.url("/limite.zip")).unwrap();
    let t = finished(&e.engine, id).await;
    assert_eq!(sha(&std::fs::read(t.output_path.unwrap()).unwrap()), sha(&data));
}

#[tokio::test(flavor = "multi_thread")]
async fn server_refusing_browser_agents_still_delivers_the_file() {
    let srv = serve().await;
    let e = env().await;
    srv.shared.reject_browsers.store(true, std::sync::atomic::Ordering::SeqCst);
    // Avec et sans extension : le chemin de détection n'est pas le même.
    for path in ["/test/100MB.bin", "/fichier"] {
        let data = random_bytes(9 * 1024 * 1024);
        srv.add(path, data.clone(), "application/octet-stream");
        let id = e.engine.add_url(&srv.url(path)).unwrap();
        let t = finished(&e.engine, id).await;
        assert_eq!(sha(&std::fs::read(t.output_path.unwrap()).unwrap()), sha(&data));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn vp9_and_opus_become_a_file_quicktime_can_open() {
    if !have_ffmpeg() { return; }
    let srv = serve().await;
    let d = tempfile::tempdir().unwrap();
    make_webm(&d.path().join("clip.webm"));
    srv.add_dir(d.path(), "/w");
    let e = env().await;
    let id = e.engine.add_url(&srv.url("/w/clip.webm")).unwrap();
    let t = finished(&e.engine, id).await;
    let out = std::path::PathBuf::from(t.output_path.unwrap());
    assert_eq!(out.extension().unwrap(), "mp4");
    let p = probe_file(&out).await;
    assert_eq!(p.vcodec.as_deref(), Some("h264"));
    assert_eq!(p.acodec.as_deref(), Some("aac"));
    assert_eq!(p.height, Some(90));
    // L'original reste à côté, sans conversion, pour VLC.
    let stem = out.file_stem().unwrap().to_string_lossy().into_owned();
    let o = probe_file(&out.with_file_name(format!("{stem} (original).mkv"))).await;
    assert_eq!(o.vcodec.as_deref(), Some("vp9"));
    assert_eq!(o.acodec.as_deref(), Some("opus"));
}

#[tokio::test(flavor = "multi_thread")]
async fn disk_space_is_checked() {
    let srv = serve().await;
    srv.add("/s.mp4", random_bytes(5000), "video/mp4");
    let e = env_with(|s| s.min_free_mb = u64::MAX / 4_000_000).await;
    let id = e.engine.add_url(&srv.url("/s.mp4")).unwrap();
    let t = done(&e.engine, id).await;
    assert_eq!(t.state, TaskState::Failed);
    assert!(t.error.unwrap().contains("Espace disque"));
}

#[tokio::test(flavor = "multi_thread")]
async fn live_recording_can_be_stopped_and_is_playable() {
    if !have_ffmpeg() { return; }
    let srv = serve().await;
    let d = tempfile::tempdir().unwrap();
    make_hls(d.path(), false);
    srv.add_dir(d.path(), "/l");
    let pl = std::fs::read_to_string(d.path().join("index.m3u8")).unwrap().replace("#EXT-X-ENDLIST\n", "").replace("#EXT-X-PLAYLIST-TYPE:VOD\n", "");
    srv.add("/l/index.m3u8", pl.into_bytes(), "application/vnd.apple.mpegurl");
    let e = env().await;
    let id = e.engine.add_url(&srv.url("/l/index.m3u8")).unwrap();
    wait_until(&e.engine, id, 30, "enregistrement", |t| t.state == TaskState::Recording && t.progress.downloaded > 0).await;
    e.engine.stop_recording(id).unwrap();
    let t = finished(&e.engine, id).await;
    let p = probe_file(std::path::Path::new(&t.output_path.unwrap())).await;
    assert!(p.duration.unwrap() > 1.0);
}
