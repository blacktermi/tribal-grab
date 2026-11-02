//! Mini-DOM XML (suffisant pour DASH et TTML), construit sur quick-xml.
use crate::error::{Error, Result};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    pub attrs: HashMap<String, String>,
    pub children: Vec<Node>,
    pub text: String,
}

impl Node {
    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.get(k).map(String::as_str)
    }
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn kids<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }
    /// Texte de tout le sous-arbre, avec `<br/>` converti en saut de ligne.
    pub fn full_text(&self) -> String {
        let mut out = String::new();
        for c in &self.children {
            match c.name.as_str() {
                "#text" => out.push_str(&c.text),
                "br" => out.push('\n'),
                _ => out.push_str(&c.full_text()),
            }
        }
        out
    }
}

fn local(n: &str) -> String {
    n.rsplit(':').next().unwrap_or("").to_string()
}

fn start_node(e: &BytesStart) -> Node {
    let mut n = Node { name: local(e.name().as_ref()), ..Default::default() };
    for a in e.attributes().flatten() {
        let key = local(a.key.as_ref());
        let val = a.normalized_value(quick_xml::XmlVersion::Implicit1_0).map(|v| v.into_owned()).unwrap_or_default();
        n.attrs.insert(key, val);
    }
    n
}

fn push_text(n: &mut Node, s: String) {
    n.children.push(Node { name: "#text".into(), text: s, ..Default::default() });
}

pub fn parse(xml: &str) -> Result<Node> {
    let mut r = Reader::from_str(xml);
    let mut stack: Vec<Node> = vec![Node::default()];
    loop {
        match r.read_event().map_err(|e| Error::Extract(format!("XML invalide : {e}")))? {
            Event::Start(e) => stack.push(start_node(&e)),
            Event::Empty(e) => {
                let n = start_node(&e);
                stack.last_mut().unwrap().children.push(n);
            }
            Event::Text(t) => push_text(stack.last_mut().unwrap(), t.xml10_content().into_owned()),
            Event::CData(t) => push_text(stack.last_mut().unwrap(), t.xml10_content().into_owned()),
            // Les références (&amp;, &#233;…) arrivent comme événements séparés.
            Event::GeneralRef(r) => {
                let s = match r.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    _ => match r.xml10_content().as_ref() {
                        "amp" => "&".into(),
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "quot" => "\"".into(),
                        "apos" => "'".into(),
                        other => format!("&{other};"),
                    },
                };
                push_text(stack.last_mut().unwrap(), s);
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err(Error::Extract("XML mal formé".into()));
                }
                let mut n = stack.pop().unwrap();
                n.text = n.children.iter().filter(|c| c.name == "#text").map(|c| c.text.as_str()).collect();
                stack.last_mut().unwrap().children.push(n);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut root = stack.pop().unwrap();
    let first = root.children.iter().position(|c| c.name != "#text");
    match first {
        Some(i) if stack.is_empty() => Ok(root.children.remove(i)),
        _ => Err(Error::Extract("XML incomplet".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_namespaces() {
        let n = parse(r#"<?xml version="1.0"?><tt:tt xmlns:tt="x"><tt:p begin="1s">a<tt:br/>b</tt:p></tt:tt>"#).unwrap();
        assert_eq!(n.name, "tt");
        assert_eq!(n.children[0].attr("begin"), Some("1s"));
    }
}
