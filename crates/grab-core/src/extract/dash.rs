//! Analyse des manifestes DASH : SegmentTemplate, SegmentList, SegmentBase / BaseURL.
use crate::error::{Error, Result};
use crate::model::*;
use crate::xml::{self, Node};
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::Arc;

fn join(base: &str, rel: &str) -> String {
    url::Url::parse(base).and_then(|b| b.join(rel)).map(|u| u.to_string()).unwrap_or_else(|_| rel.to_string())
}

pub fn iso_duration(s: &str) -> Option<f64> {
    let re = Regex::new(r"^P(?:(\d+(?:\.\d+)?)D)?(?:T(?:(\d+(?:\.\d+)?)H)?(?:(\d+(?:\.\d+)?)M)?(?:(\d+(?:\.\d+)?)S)?)?$").ok()?;
    let c = re.captures(s)?;
    let g = |i| c.get(i).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(0.0);
    Some(g(1) * 86400.0 + g(2) * 3600.0 + g(3) * 60.0 + g(4))
}

fn has_content_protection(n: &Node) -> bool {
    n.name == "ContentProtection" || n.children.iter().any(has_content_protection)
}

/// Remplace `$RepresentationID$`, `$Number$`, `$Bandwidth$`, `$Time$` (avec `%05d` éventuel).
fn subst(t: &str, rep_id: &str, bw: &str, number: u64, time: u64) -> String {
    let mut out = String::new();
    let mut parts = t.split('$');
    out.push_str(parts.next().unwrap_or(""));
    let mut inside = true;
    for p in parts {
        if inside {
            if p.is_empty() {
                out.push('$');
            } else {
                let (name, fmt) = p.split_once('%').map(|(a, b)| (a, Some(b))).unwrap_or((p, None));
                let val = match name {
                    "RepresentationID" => rep_id.to_string(),
                    "Bandwidth" => bw.to_string(),
                    "Number" => number.to_string(),
                    "Time" => time.to_string(),
                    _ => String::new(),
                };
                let width = fmt.and_then(|f| f.trim_start_matches('0').strip_suffix('d')).and_then(|w| w.parse::<usize>().ok()).unwrap_or(0);
                out.push_str(&format!("{val:0>width$}"));
            }
        } else {
            out.push_str(p);
        }
        inside = !inside;
    }
    out
}

fn range_of(s: &str) -> Option<(u64, u64)> {
    let (a, b) = s.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn ext_for(mime: &str, is_video: bool) -> &'static str {
    if mime.contains("webm") {
        "webm"
    } else if is_video {
        "mp4"
    } else {
        "m4a"
    }
}

pub fn parse_manifest(url: &str, text: &str, headers: &BTreeMap<String, String>, referer: Option<&str>) -> Result<MediaInfo> {
    let mpd = xml::parse(text)?;
    if mpd.name != "MPD" {
        return Err(Error::Extract("ce n'est pas un manifeste DASH".into()));
    }
    let dynamic = mpd.attr("type") == Some("dynamic");
    let total = mpd.attr("mediaPresentationDuration").and_then(iso_duration);
    let period = mpd.kids("Period").next().ok_or_else(|| Error::Extract("manifeste DASH sans période".into()))?;
    let period_dur = period.attr("duration").and_then(iso_duration).or(total);

    let mut base = url.to_string();
    for n in [Some(&mpd), Some(period)].into_iter().flatten() {
        if let Some(b) = n.child("BaseURL") {
            base = join(&base, b.text.trim());
        }
    }

    let title = mpd
        .child("ProgramInformation")
        .and_then(|p| p.child("Title"))
        .map(|t| t.text.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| super::hls::percent_decode(url::Url::parse(url).ok().and_then(|u| u.path_segments().and_then(|mut s| s.next_back().map(String::from))).unwrap_or_default().rsplit_once('.').map(|x| x.0).unwrap_or("dash")));

    let mut info = MediaInfo {
        id: crate::util::host_of(url),
        title,
        url: url.into(),
        site: "DASH".into(),
        host: crate::util::host_of(url),
        duration: period_dur,
        is_live: dynamic,
        headers: headers.clone(),
        referer: referer.map(String::from),
        ..Default::default()
    };

    let mut seen_ids: std::collections::HashSet<String> = Default::default();
    let mut protected = 0usize;
    for aset in period.kids("AdaptationSet") {
        let aset_base = aset.child("BaseURL").map(|b| join(&base, b.text.trim())).unwrap_or(base.clone());
        let mime_as = aset.attr("mimeType").unwrap_or("");
        let ctype = aset.attr("contentType").unwrap_or_else(|| mime_as.split('/').next().unwrap_or(""));
        let aset_protected = aset.children.iter().any(|c| c.name == "ContentProtection");
        for rep in aset.kids("Representation") {
            // Protection évaluée par AdaptationSet et par Representation : les pistes en clair sont conservées.
            if aset_protected || has_content_protection(rep) {
                protected += 1;
                continue;
            }
            let mime = rep.attr("mimeType").unwrap_or(mime_as);
            let kind = if ctype.is_empty() { mime.split('/').next().unwrap_or("") } else { ctype };
            let codecs = rep.attr("codecs").or(aset.attr("codecs")).unwrap_or("").to_string();
            let rep_base = rep.child("BaseURL").map(|b| join(&aset_base, b.text.trim())).unwrap_or(aset_base.clone());
            let id = rep.attr("id").unwrap_or("0").to_string();
            let bw = rep.attr("bandwidth").unwrap_or("0");

            if kind == "text" || mime.contains("ttml") || mime.contains("vtt") {
                if rep.child("SegmentTemplate").is_none() && aset.child("SegmentTemplate").is_none() {
                    let ext = if mime.contains("ttml") { "ttml" } else if mime.contains("vtt") { "vtt" } else { continue };
                    info.subtitles.push(SubtitleTrack { lang: aset.attr("lang").unwrap_or("und").into(), name: aset.attr("label").map(String::from), url: rep_base, ext: ext.into(), auto: false });
                }
                continue;
            }
            if kind != "video" && kind != "audio" {
                continue;
            }
            let is_video = kind == "video";
            let mut fid = id.clone();
            if !seen_ids.insert(fid.clone()) {
                fid = format!("{}-{}", id, seen_ids.len());
                seen_ids.insert(fid.clone());
            }
            let mut f = Format {
                id: fid,
                url: url.into(),
                ext: ext_for(mime, is_video).into(),
                vcodec: Some(if is_video { if codecs.is_empty() { "unknown".into() } else { codecs.clone() } } else { "none".into() }),
                acodec: Some(if is_video { "none".into() } else if codecs.is_empty() { "unknown".into() } else { codecs.clone() }),
                width: rep.attr("width").or(aset.attr("width")).and_then(|v| v.parse().ok()),
                height: rep.attr("height").or(aset.attr("height")).and_then(|v| v.parse().ok()),
                tbr: bw.parse::<f64>().ok().map(|b| b / 1000.0),
                language: aset.attr("lang").map(String::from),
                label: aset.attr("label").map(String::from),
                headers: headers.clone(),
                ..Default::default()
            };

            // Les attributs de SegmentTemplate peuvent être sur l'AdaptationSet ou la Representation.
            let tpl_nodes: Vec<&Node> = [aset.child("SegmentTemplate"), rep.child("SegmentTemplate")].into_iter().flatten().collect();
            let list = rep.child("SegmentList").or(aset.child("SegmentList"));
            if !tpl_nodes.is_empty() {
                let get = |k: &str| tpl_nodes.iter().rev().find_map(|n| n.attr(k));
                let timescale: f64 = get("timescale").and_then(|v| v.parse().ok()).unwrap_or(1.0);
                let start: u64 = get("startNumber").and_then(|v| v.parse().ok()).unwrap_or(1);
                let media = get("media").ok_or_else(|| Error::Extract("SegmentTemplate sans attribut media".into()))?;
                let mut plan = SegmentPlan { is_live: dynamic, ..Default::default() };
                if let Some(i) = get("initialization") {
                    plan.init = Some(Segment { url: join(&rep_base, &subst(i, &id, bw, 0, 0)), ..Default::default() });
                }
                let timeline = tpl_nodes.iter().rev().find_map(|n| n.child("SegmentTimeline"));
                if dynamic {
                    // Direct : enregistré par ffmpeg, pas de liste de segments.
                } else if let Some(tl) = timeline {
                    let mut number = start;
                    let mut t: u64 = 0;
                    let limit = period_dur.map(|d| (d * timescale) as u64);
                    for s in tl.kids("S") {
                        if let Some(st) = s.attr("t").and_then(|v| v.parse().ok()) {
                            t = st;
                        }
                        let d: u64 = s.attr("d").and_then(|v| v.parse().ok()).unwrap_or(0);
                        if d == 0 {
                            continue;
                        }
                        let r: i64 = s.attr("r").and_then(|v| v.parse().ok()).unwrap_or(0);
                        let reps = if r < 0 { limit.map(|l| (l.saturating_sub(t)).div_ceil(d)).unwrap_or(1) } else { r as u64 + 1 };
                        for _ in 0..reps {
                            plan.segments.push(Segment { url: join(&rep_base, &subst(media, &id, bw, number, t)), ..Default::default() });
                            number += 1;
                            t += d;
                        }
                    }
                } else {
                    let d: f64 = get("duration").and_then(|v| v.parse().ok()).ok_or_else(|| Error::Extract("SegmentTemplate sans durée".into()))?;
                    let seg_secs = d / timescale;
                    let n = period_dur.map(|p| (p / seg_secs).ceil() as u64).ok_or_else(|| Error::Extract("durée du manifeste inconnue".into()))?;
                    for i in 0..n {
                        plan.segments.push(Segment { url: join(&rep_base, &subst(media, &id, bw, start + i, i * d as u64)), ..Default::default() });
                    }
                }
                f.protocol = Protocol::Segments;
                f.plan = Some(Arc::new(plan));
            } else if let Some(l) = list {
                let mut plan = SegmentPlan::default();
                if let Some(i) = l.child("Initialization") {
                    plan.init = Some(Segment { url: join(&rep_base, i.attr("sourceURL").unwrap_or("")), range: i.attr("range").and_then(range_of), key: None });
                }
                for s in l.kids("SegmentURL") {
                    plan.segments.push(Segment { url: join(&rep_base, s.attr("media").unwrap_or("")), range: s.attr("mediaRange").and_then(range_of), key: None });
                }
                f.protocol = Protocol::Segments;
                f.plan = Some(Arc::new(plan));
            } else {
                // SegmentBase ou simple BaseURL : un seul fichier, téléchargé en HTTP.
                f.protocol = Protocol::Http;
                f.url = rep_base;
            }
            info.formats.push(f);
        }
    }
    if info.formats.is_empty() && protected > 0 {
        return Err(Error::Drm);
    }
    if info.formats.is_empty() {
        return Err(Error::Extract("le manifeste DASH ne contient aucun flux vidéo ou audio".into()));
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MPD: &str = r#"<?xml version="1.0"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="static" mediaPresentationDuration="PT20S">
 <Period>
  <AdaptationSet mimeType="video/mp4" contentType="video">
   <SegmentTemplate timescale="1000" duration="4000" startNumber="1" media="v_$RepresentationID$_$Number%05d$.m4s" initialization="v_$RepresentationID$_init.mp4"/>
   <Representation id="360" bandwidth="500000" width="640" height="360" codecs="avc1.4d401e"/>
   <Representation id="720" bandwidth="2000000" width="1280" height="720" codecs="avc1.4d401f"/>
  </AdaptationSet>
  <AdaptationSet mimeType="audio/mp4" contentType="audio" lang="en">
   <Representation id="a1" bandwidth="96000" codecs="mp4a.40.2">
    <SegmentTemplate timescale="1" media="a_$Number$.m4s" initialization="a_init.mp4">
      <SegmentTimeline><S t="0" d="10" r="1"/></SegmentTimeline>
    </SegmentTemplate>
   </Representation>
  </AdaptationSet>
 </Period>
</MPD>"#;

    #[test]
    fn template_and_timeline() {
        let i = parse_manifest("https://x.test/d/m.mpd", MPD, &Default::default(), None).unwrap();
        assert_eq!(i.formats.len(), 3);
        let v = i.formats.iter().find(|f| f.id == "720").unwrap();
        let p = v.plan.as_ref().unwrap();
        assert_eq!(p.segments.len(), 5);
        assert_eq!(p.segments[0].url, "https://x.test/d/v_720_00001.m4s");
        assert_eq!(p.init.as_ref().unwrap().url, "https://x.test/d/v_720_init.mp4");
        let a = i.formats.iter().find(|f| f.id == "a1").unwrap();
        assert_eq!(a.plan.as_ref().unwrap().segments.len(), 2);
        assert_eq!(a.language.as_deref(), Some("en"));
    }

    #[test]
    fn content_protection_is_judged_per_track() {
        // Vidéo 720 protégée (Representation), audio en clair : on garde le reste.
        let m = MPD.replace("<Representation id=\"720\" bandwidth=\"2000000\" width=\"1280\" height=\"720\" codecs=\"avc1.4d401f\"/>", "<Representation id=\"720\" bandwidth=\"2000000\" width=\"1280\" height=\"720\" codecs=\"avc1.4d401f\"><ContentProtection schemeIdUri=\"urn:uuid:edef8ba9\"/></Representation>");
        let i = parse_manifest("https://x.test/m.mpd", &m, &Default::default(), None).unwrap();
        assert!(i.formats.iter().all(|f| f.id != "720") && i.formats.iter().any(|f| f.id == "360") && i.formats.iter().any(|f| f.id == "a1"));
        // AdaptationSet vidéo entier protégé : l'audio reste.
        let m2 = MPD.replace("<AdaptationSet mimeType=\"video/mp4\" contentType=\"video\">", "<AdaptationSet mimeType=\"video/mp4\" contentType=\"video\"><ContentProtection schemeIdUri=\"urn:uuid:edef8ba9\"/>");
        let i2 = parse_manifest("https://x.test/m.mpd", &m2, &Default::default(), None).unwrap();
        assert_eq!(i2.formats.len(), 1);
        // Tout protégé : refus.
        let m3 = m2.replace("<AdaptationSet mimeType=\"audio/mp4\" contentType=\"audio\" lang=\"en\">", "<AdaptationSet mimeType=\"audio/mp4\" contentType=\"audio\" lang=\"en\"><ContentProtection schemeIdUri=\"urn:uuid:edef8ba9\"/>");
        assert!(matches!(parse_manifest("https://x.test/m.mpd", &m3, &Default::default(), None), Err(Error::Drm)));
    }

    #[test]
    fn durations() {
        assert_eq!(iso_duration("PT1M30.5S"), Some(90.5));
        assert_eq!(iso_duration("PT0H3M30.0S"), Some(210.0));
    }
}
