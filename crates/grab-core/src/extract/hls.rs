//! Analyse des playlists HLS : variantes, audio séparé, segments, chiffrement AES-128.
use crate::error::{Error, Result};
use crate::model::*;
use m3u8_rs::{AlternativeMediaType, KeyMethod, Playlist};
use std::collections::BTreeMap;

fn abs(base: &str, rel: &str) -> String {
    url::Url::parse(base).and_then(|b| b.join(rel)).map(|u| u.to_string()).unwrap_or_else(|_| rel.to_string())
}

fn split_codecs(c: &str) -> (Option<String>, Option<String>) {
    let (mut v, mut a) = (None, None);
    for p in c.split(',').map(str::trim) {
        let l = p.to_lowercase();
        if ["avc", "hvc", "hev", "av01", "vp09", "vp9", "dvh"].iter().any(|x| l.starts_with(x)) {
            v = Some(p.to_string());
        } else if ["mp4a", "ac-3", "ec-3", "opus", "flac", "mha"].iter().any(|x| l.starts_with(x)) {
            a = Some(p.to_string());
        }
    }
    (v, a)
}

fn is_drm_keyformat(kf: Option<&str>) -> bool {
    kf.map(str::to_lowercase).is_some_and(|k| k.contains("streamingkeydelivery") || k.contains("edef8ba9") || k.contains("9a04f079") || k.contains("playready") || k.contains("widevine"))
}

fn title_from_url(url: &str) -> String {
    let u = url::Url::parse(url).ok();
    let seg = u.as_ref().and_then(|u| u.path_segments()).and_then(|mut s| s.next_back().map(String::from)).unwrap_or_default();
    let name = percent_decode(&seg);
    let stem = name.rsplit_once('.').map(|x| x.0.to_string()).unwrap_or(name);
    if stem.is_empty() || stem == "master" || stem == "index" || stem == "playlist" {
        crate::util::host_of(url)
    } else {
        stem
    }
}

pub fn percent_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy().into_owned()
}

/// Transforme le texte d'une playlist (maître ou média) en `MediaInfo`.
pub fn parse_manifest(url: &str, text: &str, headers: &BTreeMap<String, String>, referer: Option<&str>) -> Result<MediaInfo> {
    let pl = m3u8_rs::parse_playlist_res(text.as_bytes()).map_err(|_| Error::Extract("playlist HLS invalide".into()))?;
    let mut info = MediaInfo {
        id: crate::util::host_of(url),
        title: title_from_url(url),
        url: url.to_string(),
        site: "HLS".into(),
        host: crate::util::host_of(url),
        headers: headers.clone(),
        referer: referer.map(String::from),
        ..Default::default()
    };
    match pl {
        Playlist::MediaPlaylist(m) => {
            // METHOD=NONE n'est jamais un refus ; seules les clés FairPlay ou Widevine comptent.
            let drm = m.segments.iter().filter_map(|s| s.key.as_ref()).any(|k| k.method != KeyMethod::None && is_drm_keyformat(k.keyformat.as_deref()));
            info.is_live = !m.end_list;
            info.formats.push(Format {
                id: "hls".into(),
                url: url.into(),
                protocol: Protocol::Hls,
                ext: "ts".into(),
                vcodec: Some("unknown".into()),
                acodec: Some("unknown".into()),
                has_drm: drm,
                headers: headers.clone(),
                ..Default::default()
            });
        }
        Playlist::MasterPlaylist(m) => {
            // Une clé de session protégée ne rend pas toutes les variantes illisibles :
            // la protection est jugée variante par variante, au téléchargement (voir `build_plan`).
            let mut audio_groups: std::collections::HashMap<String, Vec<&m3u8_rs::AlternativeMedia>> = Default::default();
            for alt in &m.alternatives {
                if alt.media_type == AlternativeMediaType::Audio && alt.uri.is_some() {
                    audio_groups.entry(alt.group_id.clone()).or_default().push(alt);
                }
            }
            let mut used_groups = std::collections::BTreeSet::new();
            for v in m.variants.iter().filter(|v| !v.is_i_frame) {
                let (vc, ac) = v.codecs.as_deref().map(split_codecs).unwrap_or((None, None));
                let separate_audio = v.audio.as_ref().is_some_and(|g| audio_groups.contains_key(g));
                if let Some(g) = &v.audio
                    && separate_audio {
                        used_groups.insert(g.clone());
                    }
                let h = v.resolution.as_ref().map(|r| r.height as u32);
                info.formats.push(Format {
                    id: format!("hls-{}p-{}k", h.unwrap_or(0), v.bandwidth / 1000),
                    url: abs(url, &v.uri),
                    protocol: Protocol::Hls,
                    ext: "ts".into(),
                    vcodec: Some(vc.unwrap_or_else(|| "unknown".into())),
                    acodec: Some(if separate_audio { "none".into() } else { ac.unwrap_or_else(|| "unknown".into()) }),
                    width: v.resolution.as_ref().map(|r| r.width as u32),
                    height: h,
                    tbr: Some(v.bandwidth as f64 / 1000.0),
                    headers: headers.clone(),
                    ..Default::default()
                });
            }
            let audio_codec = m.variants.iter().filter_map(|v| v.codecs.as_deref()).filter_map(|c| split_codecs(c).1).next();
            for g in used_groups {
                for alt in &audio_groups[&g] {
                    info.formats.push(Format {
                        id: format!("hls-audio-{}-{}", g, alt.language.as_deref().unwrap_or(&alt.name)),
                        url: abs(url, alt.uri.as_deref().unwrap_or_default()),
                        protocol: Protocol::Hls,
                        ext: "ts".into(),
                        vcodec: Some("none".into()),
                        acodec: Some(audio_codec.clone().unwrap_or_else(|| "unknown".into())),
                        language: alt.language.clone(),
                        label: Some(alt.name.clone()),
                        headers: headers.clone(),
                        ..Default::default()
                    });
                }
            }
            for alt in m.alternatives.iter().filter(|a| a.media_type == AlternativeMediaType::Subtitles) {
                if let Some(u) = &alt.uri {
                    info.subtitles.push(SubtitleTrack {
                        lang: alt.language.clone().unwrap_or_else(|| alt.name.clone()),
                        name: Some(alt.name.clone()),
                        url: abs(url, u),
                        ext: "m3u8".into(),
                        auto: false,
                    });
                }
            }
        }
    }
    if info.formats.is_empty() {
        return Err(Error::Extract("la playlist HLS ne contient aucun flux".into()));
    }
    Ok(info)
}

fn parse_iv(s: &str) -> Option<[u8; 16]> {
    let h = s.trim_start_matches("0x").trim_start_matches("0X");
    let padded = format!("{h:0>32}");
    let b = hex::decode(padded).ok()?;
    b.try_into().ok()
}

/// Construit la liste de segments d'une playlist média.
pub fn build_plan(url: &str, text: &str) -> Result<SegmentPlan> {
    let Playlist::MediaPlaylist(m) = m3u8_rs::parse_playlist_res(text.as_bytes()).map_err(|_| Error::Extract("playlist HLS invalide".into()))? else {
        return Err(Error::Extract("playlist média attendue".into()));
    };
    let mut plan = SegmentPlan { is_live: !m.end_list, ..Default::default() };
    let mut key: Option<(String, Option<[u8; 16]>)> = None;
    let mut ends: std::collections::HashMap<String, u64> = Default::default();
    for (i, s) in m.segments.iter().enumerate() {
        if let Some(k) = &s.key {
            match &k.method {
                KeyMethod::None => key = None,
                KeyMethod::AES128 => {
                    if is_drm_keyformat(k.keyformat.as_deref()) {
                        return Err(Error::Drm);
                    }
                    let uri = k.uri.as_deref().ok_or_else(|| Error::Extract("clé HLS sans adresse".into()))?;
                    key = Some((abs(url, uri), k.iv.as_deref().and_then(parse_iv)));
                }
                KeyMethod::SampleAES | KeyMethod::Other(_) => {
                    if is_drm_keyformat(k.keyformat.as_deref()) {
                        return Err(Error::Drm);
                    }
                    return Err(Error::Unsupported("chiffrement HLS SAMPLE-AES".into()));
                }
            }
        }
        if plan.init.is_none()
            && let Some(map) = &s.map {
                plan.init = Some(Segment { url: abs(url, &map.uri), range: map.byte_range.as_ref().map(|r| (r.offset.unwrap_or(0), r.offset.unwrap_or(0) + r.length - 1)), key: None });
            }
        let seg_url = abs(url, &s.uri);
        let range = s.byte_range.as_ref().map(|r| {
            let off = r.offset.unwrap_or_else(|| *ends.get(&seg_url).unwrap_or(&0));
            ends.insert(seg_url.clone(), off + r.length);
            (off, off + r.length - 1)
        });
        let seq = m.media_sequence + i as u64;
        let k = key.as_ref().map(|(u, iv)| SegKey { url: u.clone(), iv: iv.unwrap_or_else(|| (seq as u128).to_be_bytes()) });
        plan.segments.push(Segment { url: seg_url, range, key: k });
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MASTER: &str = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"Fr\",LANGUAGE=\"fr\",URI=\"audio_fr.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,CODECS=\"avc1.4d401e,mp4a.40.2\",AUDIO=\"a\"\nv360.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2000000,RESOLUTION=1280x720,CODECS=\"avc1.4d401f,mp4a.40.2\",AUDIO=\"a\"\nv720.m3u8\n";

    #[test]
    fn master_with_separate_audio() {
        let i = parse_manifest("https://x.test/a/master.m3u8", MASTER, &Default::default(), None).unwrap();
        assert_eq!(i.formats.len(), 3);
        let v = i.formats.iter().find(|f| f.height == Some(720)).unwrap();
        assert_eq!(v.acodec.as_deref(), Some("none"));
        assert_eq!(v.url, "https://x.test/a/v720.m3u8");
        assert!(i.formats.iter().any(|f| f.vcodec.as_deref() == Some("none") && f.language.as_deref() == Some("fr")));
    }

    #[test]
    fn media_plan_with_key_and_default_iv() {
        let t = "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:5\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n#EXTINF:4,\ns1.ts\n#EXTINF:4,\ns2.ts\n#EXT-X-ENDLIST\n";
        let p = build_plan("https://x.test/a/m.m3u8", t).unwrap();
        assert_eq!(p.segments.len(), 2);
        assert_eq!(p.segments[0].key.as_ref().unwrap().iv[15], 5);
        assert_eq!(p.segments[1].key.as_ref().unwrap().iv[15], 6);
        assert_eq!(p.segments[0].key.as_ref().unwrap().url, "https://x.test/a/k.bin");
        assert!(!p.is_live);
    }

    #[test]
    fn fairplay_is_refused() {
        let t = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n#EXTINF:4,\ns1.ts\n#EXT-X-ENDLIST\n";
        assert!(matches!(build_plan("https://x.test/m.m3u8", t), Err(Error::Drm)));
    }

    #[test]
    fn drm_is_judged_per_variant_and_none_is_clear() {
        // Clé de session FairPlay dans le maître : le maître reste utilisable.
        let m = format!("#EXTM3U\n#EXT-X-SESSION-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n{}", &MASTER[8..]);
        assert!(parse_manifest("https://x.test/a/master.m3u8", &m, &Default::default(), None).is_ok());
        // METHOD=NONE puis AES-128 : aucun refus.
        let t = "#EXTM3U\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:4,\ns0.ts\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n#EXTINF:4,\ns1.ts\n#EXT-X-ENDLIST\n";
        let p = build_plan("https://x.test/m.m3u8", t).unwrap();
        assert!(p.segments[0].key.is_none() && p.segments[1].key.is_some());
        let i = parse_manifest("https://x.test/m.m3u8", t, &Default::default(), None).unwrap();
        assert!(!i.formats[0].has_drm);
        // Widevine : la variante est refusée, mais une autre variante en clair reste possible.
        let w = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES-CTR,URI=\"data:x\",KEYFORMAT=\"urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed\"\n#EXTINF:4,\ns0.ts\n#EXT-X-ENDLIST\n";
        assert!(matches!(build_plan("https://x.test/w.m3u8", w), Err(Error::Drm)));
        assert!(parse_manifest("https://x.test/w.m3u8", w, &Default::default(), None).unwrap().formats[0].has_drm);
    }

    #[test]
    fn live_detected() {
        let t = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\ns1.ts\n";
        let i = parse_manifest("https://x.test/live.m3u8", t, &Default::default(), None).unwrap();
        assert!(i.is_live);
    }
}
