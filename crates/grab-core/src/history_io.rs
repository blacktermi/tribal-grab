//! Export (JSON, CSV, XML) et import (JSON, CSV, XML) de l'historique.
use crate::error::{Error, Result};
use crate::model::HistoryItem;
use crate::xml;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    Json,
    Csv,
    Xml,
}

const COLS: [&str; 7] = ["url", "media_id", "title", "site", "path", "size", "finished_at"];

fn csv_field(s: &str) -> String {
    // Neutralise l'injection de formules dans les tableurs.
    let s = if s.starts_with(['=', '+', '@']) || (s.starts_with('-') && s.len() > 1 && !s[1..].chars().all(|c| c.is_ascii_digit())) { format!("'{s}") } else { s.to_string() };
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn export(items: &[HistoryItem], f: Format) -> Result<String> {
    Ok(match f {
        Format::Json => serde_json::to_string_pretty(items)?,
        Format::Csv => {
            let mut s = COLS.join(",") + "\n";
            for h in items {
                let row = [
                    h.url.clone(),
                    h.media_id.clone().unwrap_or_default(),
                    h.title.clone(),
                    h.site.clone(),
                    h.path.clone().unwrap_or_default(),
                    h.size.map(|x| x.to_string()).unwrap_or_default(),
                    h.finished_at.to_string(),
                ];
                s.push_str(&row.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(","));
                s.push('\n');
            }
            s
        }
        Format::Xml => {
            let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<history>\n");
            for h in items {
                s.push_str("  <item>\n");
                let vals = [
                    h.url.clone(),
                    h.media_id.clone().unwrap_or_default(),
                    h.title.clone(),
                    h.site.clone(),
                    h.path.clone().unwrap_or_default(),
                    h.size.map(|x| x.to_string()).unwrap_or_default(),
                    h.finished_at.to_string(),
                ];
                for (c, v) in COLS.iter().zip(vals) {
                    s.push_str(&format!("    <{c}>{}</{c}>\n", xml_escape(&v)));
                }
                s.push_str("  </item>\n");
            }
            s.push_str("</history>\n");
            s
        }
    })
}

fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let (mut row, mut field, mut quoted) = (Vec::new(), String::new(), false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => row.push(std::mem::take(&mut field)),
            ('\n', false) | ('\r', false) => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                if row.iter().any(|f| !f.is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            (c, _) => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

fn item_from(get: impl Fn(&str) -> String) -> Option<HistoryItem> {
    let url = get("url");
    crate::util::validate_url(&url).ok()?;
    let unq = |s: String| s.strip_prefix('\'').map(String::from).unwrap_or(s);
    let opt = |s: String| (!s.is_empty()).then_some(unq(s));
    Some(HistoryItem {
        id: 0,
        url,
        media_id: opt(get("media_id")),
        title: unq(get("title")),
        site: unq(get("site")),
        path: opt(get("path")),
        size: get("size").parse().ok(),
        thumbnail: None,
        finished_at: get("finished_at").parse().unwrap_or_else(|_| crate::util::now()),
        exists: false,
    })
}

pub fn import(data: &str, f: Format) -> Result<Vec<HistoryItem>> {
    if data.len() > 64 * 1024 * 1024 {
        return Err(Error::Other("fichier d'import trop volumineux".into()));
    }
    let items: Vec<HistoryItem> = match f {
        Format::Json => serde_json::from_str::<Vec<HistoryItem>>(data)?.into_iter().filter(|h| crate::util::validate_url(&h.url).is_ok()).collect(),
        Format::Csv => {
            let rows = parse_csv(data);
            let header = rows.first().ok_or_else(|| Error::Other("fichier CSV vide".into()))?.clone();
            rows.iter()
                .skip(1)
                .filter_map(|r| item_from(|k| header.iter().position(|h| h == k).and_then(|i| r.get(i).cloned()).unwrap_or_default()))
                .collect()
        }
        Format::Xml => {
            let root = xml::parse(data)?;
            root.kids("item").filter_map(|n| item_from(|k| n.child(k).map(|c| c.text.trim().to_string()).unwrap_or_default())).collect()
        }
    };
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<HistoryItem> {
        vec![
            HistoryItem { url: "https://a.com/v?x=1".into(), title: "Titre, avec \"guillemets\"\net saut".into(), site: "YouTube".into(), path: Some("/tmp/a.mp4".into()), size: Some(42), finished_at: 1000, ..Default::default() },
            HistoryItem { url: "https://b.com/v".into(), title: "=CMD()".into(), site: "X & Y <z>".into(), finished_at: 2000, ..Default::default() },
        ]
    }

    #[test]
    fn roundtrip_all_formats() {
        for f in [Format::Json, Format::Csv, Format::Xml] {
            let s = export(&items(), f).unwrap();
            let back = import(&s, f).unwrap();
            assert_eq!(back.len(), 2, "{f:?}");
            assert_eq!(back[0].url, "https://a.com/v?x=1");
            assert_eq!(back[0].size, Some(42));
            assert_eq!(back[1].title, "=CMD()", "{f:?}");
            if f != Format::Json {
                assert_eq!(back[0].title, items()[0].title, "{f:?}");
            }
        }
    }

    #[test]
    fn csv_formula_guard() {
        let s = export(&items(), Format::Csv).unwrap();
        assert!(s.contains("'=CMD()"));
    }

    #[test]
    fn rejects_non_http_urls_on_import() {
        let csv = "url,title\nfile:///etc/passwd,x\nhttps://ok.com/a,y\n";
        assert_eq!(import(csv, Format::Csv).unwrap().len(), 1);
    }
}
