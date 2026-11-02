//! Tests réseau (vrais serveurs). Lancer avec : cargo test -p grab-core --test network -- --ignored
//! Variable TG_YTDLP : chemin d'un yt-dlp à jour (la version ancienne est refusée par YouTube).
mod common;
use common::*;
use grab_core::*;
use std::path::Path;

async fn net_env() -> Env {
    env_with(|s| {
        if let Ok(p) = std::env::var("TG_YTDLP") {
            s.tool_paths.insert("yt-dlp".into(), p);
        }
    })
    .await
}

async fn grab(e: &Env, url: &str, o: TaskOptions) -> Task {
    let id = e.engine.add(AddRequest { url: url.into(), options: o, ..Default::default() }).unwrap();
    finished(&e.engine, id).await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn direct_mp4() {
    let e = net_env().await;
    let t = grab(&e, "https://test-videos.co.uk/vids/bigbuckbunny/mp4/h264/360/Big_Buck_Bunny_360_10s_1MB.mp4", Default::default()).await;
    let p = probe_file(Path::new(&t.output_path.unwrap())).await;
    assert_eq!(p.vcodec.as_deref(), Some("h264"));
    assert!((p.duration.unwrap() - 10.0).abs() < 0.5);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn hls_apple_bipbop_low_quality() {
    let e = net_env().await;
    let o = TaskOptions { max_resolution: Some(360), ..Default::default() };
    let t = grab(&e, "https://devstreaming-cdn.apple.com/videos/streaming/examples/bipbop_16x9/bipbop_16x9_variant.m3u8", o).await;
    let p = probe_file(Path::new(&t.output_path.unwrap())).await;
    assert_eq!((p.vcodec.as_deref(), p.acodec.as_deref(), p.height), (Some("h264"), Some("aac"), Some(360)));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn dash_akamai_low_quality() {
    let e = net_env().await;
    let o = TaskOptions { max_resolution: Some(360), ..Default::default() };
    let t = grab(&e, "https://dash.akamaized.net/akamai/bbb_30fps/bbb_30fps.mpd", o).await;
    let p = probe_file(Path::new(&t.output_path.unwrap())).await;
    assert_eq!((p.vcodec.as_deref(), p.acodec.as_deref(), p.height), (Some("h264"), Some("aac"), Some(360)));
    assert!(p.duration.unwrap() > 600.0);
}

const YT: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw";

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn youtube_video_mp3_and_subtitles() {
    let e = net_env().await;
    let t = grab(&e, YT, TaskOptions { name_template: Some("{title} video".into()), ..Default::default() }).await;
    let p = probe_file(Path::new(&t.output_path.unwrap())).await;
    assert_eq!((p.vcodec.as_deref(), p.acodec.as_deref()), (Some("h264"), Some("aac")));
    let t = grab(&e, YT, TaskOptions { audio_format: Some(AudioFormat::Mp3), duplicate: Some(DuplicatePolicy::Redownload), name_template: Some("{title} mp3".into()), ..Default::default() }).await;
    assert_eq!(probe_file(Path::new(&t.output_path.unwrap())).await.acodec.as_deref(), Some("mp3"));
    let o = TaskOptions { sub_mode: Some(SubMode::Required), sub_langs: Some(vec!["en".into()]), duplicate: Some(DuplicatePolicy::Redownload), name_template: Some("{title} sub".into()), ..Default::default() };
    grab(&e, YT, o).await;
    let srt = std::fs::read_to_string(e.out.join("Me at the zoo sub.en.srt")).unwrap();
    assert!(srt.contains("-->") && srt.contains("elephants"));
}
