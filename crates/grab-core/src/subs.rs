//! Sous-titres : lecture de WebVTT, SRT, TTML, DFXP, SMPTE-TT ; écriture en SRT ou VTT ; choix des pistes.
use crate::error::{Error, Result};
use crate::model::SubtitleTrack;
use crate::xml;
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// Détecte le format d'après le contenu, puis lit les repères.
pub fn parse(text: &str) -> Result<Vec<Cue>> {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    let cues = if t.starts_with("WEBVTT") {
        parse_vtt(t)
    } else if t.starts_with('<') {
        parse_ttml(t)?
    } else {
        parse_srt(t)
    };
    if cues.is_empty() {
        return Err(Error::Other("sous-titres vides ou format non reconnu".into()));
    }
    Ok(cues)
}

fn clock_to_ms(s: &str) -> Option<u64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    let secs = |p: &str| p.parse::<f64>().ok();
    let total = match parts.as_slice() {
        [h, m, sec] => secs(h)? * 3600.0 + secs(m)? * 60.0 + secs(sec)?,
        [m, sec] => secs(m)? * 60.0 + secs(sec)?,
        _ => return None,
    };
    Some((total * 1000.0).round() as u64)
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r"<[^>]*>").unwrap();
    let t = re.replace_all(s, "");
    t.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ").replace("&quot;", "\"").replace("&#39;", "'")
}

fn parse_blocks(t: &str, sep_arrow: &str) -> Vec<Cue> {
    let norm = t.replace("\r\n", "\n").replace('\r', "\n");
    let mut cues = Vec::new();
    for block in norm.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(i) = lines.iter().position(|l| l.contains(sep_arrow)) else { continue };
        let (a, b) = lines[i].split_once(sep_arrow).unwrap();
        let (Some(start), Some(end)) = (clock_to_ms(a), clock_to_ms(b.split_whitespace().next().unwrap_or(""))) else { continue };
        let text = strip_tags(&lines[i + 1..].join("\n")).trim().to_string();
        if !text.is_empty() {
            cues.push(Cue { start_ms: start, end_ms: end, text });
        }
    }
    cues
}

fn parse_vtt(t: &str) -> Vec<Cue> {
    parse_blocks(t, "-->")
}

fn parse_srt(t: &str) -> Vec<Cue> {
    parse_blocks(t, "-->")
}

fn ttml_time(s: &str, tick_rate: f64) -> Option<u64> {
    let s = s.trim();
    if let Some(v) = s.strip_suffix("ms") {
        return v.parse::<f64>().ok().map(|x| x as u64);
    }
    if let Some(v) = s.strip_suffix('s') {
        return v.parse::<f64>().ok().map(|x| (x * 1000.0) as u64);
    }
    if let Some(v) = s.strip_suffix('t') {
        return v.parse::<f64>().ok().map(|x| (x / tick_rate * 1000.0) as u64);
    }
    // hh:mm:ss.fff ou hh:mm:ss:ff (images, ignorées : 25 i/s supposées)
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 4 {
        let f: f64 = parts[3].parse().ok()?;
        let base = clock_to_ms(&parts[..3].join(":"))?;
        return Some(base + (f / 25.0 * 1000.0) as u64);
    }
    clock_to_ms(s)
}

fn collect_p(n: &xml::Node, tick: f64, out: &mut Vec<Cue>) {
    if n.name == "p" {
        if let (Some(b), Some(e)) = (n.attr("begin").and_then(|v| ttml_time(v, tick)), n.attr("end").and_then(|v| ttml_time(v, tick))) {
            let text = n.full_text().lines().map(str::trim).collect::<Vec<_>>().join("\n").trim().to_string();
            if !text.is_empty() {
                out.push(Cue { start_ms: b, end_ms: e, text });
            }
        }
        return;
    }
    n.children.iter().for_each(|c| collect_p(c, tick, out));
}

fn parse_ttml(t: &str) -> Result<Vec<Cue>> {
    let root = xml::parse(t)?;
    let tick = root.attr("tickRate").and_then(|v| v.parse().ok()).unwrap_or(10_000_000.0);
    let mut cues = Vec::new();
    collect_p(&root, tick, &mut cues);
    cues.sort_by_key(|c| c.start_ms);
    Ok(cues)
}

fn ts(ms: u64, comma: bool) -> String {
    let (h, m, s, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    format!("{h:02}:{m:02}:{s:02}{}{f:03}", if comma { ',' } else { '.' })
}

pub fn to_srt(cues: &[Cue]) -> String {
    cues.iter().enumerate().map(|(i, c)| format!("{}\n{} --> {}\n{}\n\n", i + 1, ts(c.start_ms, true), ts(c.end_ms, true), c.text)).collect()
}

pub fn to_vtt(cues: &[Cue]) -> String {
    let body: String = cues.iter().map(|c| format!("{} --> {}\n{}\n\n", ts(c.start_ms, false), ts(c.end_ms, false), c.text)).collect();
    format!("WEBVTT\n\n{body}")
}

/// Choisit les pistes selon les langues préférées (dans l'ordre), les libellés et les sous-titres automatiques.
pub fn select(tracks: &[SubtitleTrack], langs: &[String], label_re: Option<&str>, include_auto: bool, multiple: bool) -> Vec<SubtitleTrack> {
    let re = label_re.filter(|r| !r.is_empty()).and_then(|r| Regex::new(&format!("(?i){r}")).ok());
    let matches = |t: &SubtitleTrack, l: &str| {
        let tl = t.lang.to_lowercase();
        let l = l.to_lowercase();
        tl == l || tl.starts_with(&format!("{l}-")) || tl.starts_with(&format!("{l}_"))
    };
    let mut chosen: Vec<SubtitleTrack> = Vec::new();
    for l in langs {
        // Les sous-titres manuels passent avant les automatiques.
        let found = tracks
            .iter()
            .filter(|t| matches(t, l))
            .filter(|t| include_auto || !t.auto)
            .filter(|t| re.as_ref().is_none_or(|r| r.is_match(t.name.as_deref().unwrap_or("")) || r.is_match(&t.lang)))
            .min_by_key(|t| (t.auto, t.lang.len()));
        if let Some(t) = found
            && !chosen.iter().any(|c| c.lang == t.lang && c.auto == t.auto) {
                chosen.push(t.clone());
            }
        if !multiple && !chosen.is_empty() {
            break;
        }
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vtt_to_srt() {
        let v = "WEBVTT\n\n00:00:01.000 --> 00:00:02.500 align:start\nBonjour <b>le</b> monde\n\n00:03.000 --> 00:04.000\nSalut &amp; merci\n";
        let c = parse(v).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].text, "Bonjour le monde");
        assert_eq!(c[1].start_ms, 3000);
        let srt = to_srt(&c);
        assert!(srt.contains("00:00:01,000 --> 00:00:02,500"));
        assert!(to_vtt(&c).starts_with("WEBVTT"));
    }

    #[test]
    fn srt_roundtrip() {
        let s = "1\r\n00:00:01,000 --> 00:00:02,000\r\nUne ligne\r\nDeux\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nFin\r\n";
        let c = parse(s).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].text, "Une ligne\nDeux");
        assert_eq!(parse(&to_srt(&c)).unwrap(), c);
    }

    #[test]
    fn ttml_with_namespaces_and_ticks() {
        let t = r#"<?xml version="1.0"?><tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttp="http://www.w3.org/ns/ttml#parameter" ttp:tickRate="1000"><body><div>
          <p begin="1000t" end="2500t">Un<br/>deux</p><p begin="00:00:05.000" end="00:00:06.000"><span>Trois</span></p><p begin="7s" end="8.5s">Quatre</p></div></body></tt>"#;
        let c = parse(t).unwrap();
        assert_eq!(c.len(), 3);
        assert_eq!((c[0].start_ms, c[0].end_ms), (1000, 2500));
        assert_eq!(c[0].text, "Un\ndeux");
        assert_eq!(c[2].end_ms, 8500);
    }

    #[test]
    fn dfxp_and_smpte_share_the_path() {
        let t = r#"<tt xmlns="http://www.w3.org/2006/10/ttaf1"><body><div><p begin="0:00:01.5" end="0:00:02.0">x</p></div></body></tt>"#;
        assert_eq!(parse(t).unwrap()[0].start_ms, 1500);
    }

    fn tr(lang: &str, auto: bool) -> SubtitleTrack {
        SubtitleTrack { lang: lang.into(), name: None, url: format!("https://x/{lang}"), ext: "vtt".into(), auto }
    }

    #[test]
    fn selection_orders_languages() {
        let tracks = vec![tr("en", false), tr("fr-FR", true), tr("fr", true), tr("de", false)];
        let langs = vec!["fr".to_string(), "en".to_string()];
        let one = select(&tracks, &langs, None, true, false);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].lang, "fr");
        let none_auto = select(&tracks, &langs, None, false, false);
        assert_eq!(none_auto[0].lang, "en");
        let both = select(&tracks, &langs, None, true, true);
        assert_eq!(both.len(), 2);
    }
}
