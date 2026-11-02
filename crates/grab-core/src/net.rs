//! Client HTTP commun : proxy, agent utilisateur, délais, cookies Netscape.
use crate::error::{Error, Result};
use crate::model::Settings;
use std::collections::BTreeMap;
use std::time::Duration;

pub const DEFAULT_UA: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

/// Certains serveurs coupent la connexion à tout ce qui ressemble à un navigateur : on se présente alors tel quel.
pub const NEUTRAL_UA: &str = concat!("TribalGrab/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
struct Cookie {
    domain: String,
    path: String,
    secure: bool,
    name: String,
    value: String,
}

#[derive(Clone)]
pub struct Net {
    pub client: reqwest::Client,
    pub user_agent: String,
    cookies: std::sync::Arc<Vec<Cookie>>,
}

impl Net {
    pub fn new(s: &Settings) -> Result<Net> {
        let ua = s.user_agent.clone().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| DEFAULT_UA.to_string());
        let mut b = reqwest::Client::builder()
            .user_agent(&ua)
            .connect_timeout(Duration::from_secs(s.timeout_secs.max(5)))
            .read_timeout(Duration::from_secs(s.timeout_secs.max(5)))
            .redirect(reqwest::redirect::Policy::limited(10));
        if let Some(p) = &s.proxy
            && !p.url.trim().is_empty() {
                let mut proxy = reqwest::Proxy::all(&p.url).map_err(|e| Error::Network(format!("proxy invalide : {e}")))?;
                if let Some(user) = &p.username {
                    let pw = crate::secrets::get_proxy_password().unwrap_or_default();
                    proxy = proxy.basic_auth(user, &pw);
                }
                b = b.proxy(proxy);
            }
        let cookies = match &s.cookies_file {
            Some(f) if !f.is_empty() => parse_netscape(&std::fs::read_to_string(f).unwrap_or_default()),
            _ => vec![],
        };
        Ok(Net { client: b.build()?, user_agent: ua, cookies: std::sync::Arc::new(cookies) })
    }

    pub fn cookie_header(&self, url: &str) -> Option<String> {
        let u = url::Url::parse(url).ok()?;
        let host = u.host_str()?;
        let parts: Vec<String> = self
            .cookies
            .iter()
            .filter(|c| {
                let d = c.domain.trim_start_matches('.');
                (host == d || host.ends_with(&format!(".{d}"))) && u.path().starts_with(&c.path) && (!c.secure || u.scheme() == "https")
            })
            .map(|c| format!("{}={}", c.name, c.value))
            .collect();
        (!parts.is_empty()).then(|| parts.join("; "))
    }

    pub fn request(&self, url: &str, headers: &BTreeMap<String, String>) -> reqwest::RequestBuilder {
        let mut rb = self.client.get(url);
        for (k, v) in headers {
            // L'en-tête Cookie venant de l'extraction est conservé, jamais journalisé.
            rb = rb.header(k, v);
        }
        if !headers.keys().any(|k| k.eq_ignore_ascii_case("cookie"))
            && let Some(c) = self.cookie_header(url) {
                rb = rb.header("Cookie", c);
            }
        rb
    }

    /// Lit un corps texte en mémoire, avec une taille maximale.
    pub async fn fetch_text(&self, url: &str, headers: &BTreeMap<String, String>, max: usize) -> Result<String> {
        let mut resp = self.request(url, headers).send().await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(Error::Network(format!("le serveur a répondu {status}")));
        }
        let mut buf: Vec<u8> = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            buf.extend_from_slice(&chunk);
            if buf.len() > max {
                return Err(Error::Network(format!("la réponse dépasse {} Mo", max / 1_048_576)));
            }
        }
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    pub async fn fetch_bytes(&self, url: &str, headers: &BTreeMap<String, String>, max: usize) -> Result<Vec<u8>> {
        let mut resp = self.request(url, headers).send().await?;
        if !resp.status().is_success() {
            return Err(Error::Network(format!("le serveur a répondu {}", resp.status())));
        }
        let mut buf = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            buf.extend_from_slice(&chunk);
            if buf.len() > max {
                return Err(Error::Network("réponse trop volumineuse".into()));
            }
        }
        Ok(buf)
    }
}

fn parse_netscape(txt: &str) -> Vec<Cookie> {
    txt.lines()
        .filter_map(|l| {
            let l = l.strip_prefix("#HttpOnly_").unwrap_or(l);
            if l.starts_with('#') || l.trim().is_empty() {
                return None;
            }
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() >= 7).then(|| Cookie {
                domain: f[0].to_string(),
                path: f[2].to_string(),
                secure: f[3].eq_ignore_ascii_case("TRUE"),
                name: f[5].to_string(),
                value: f[6].trim().to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn netscape_cookies() {
        let c = parse_netscape("# Netscape\n.example.com\tTRUE\t/\tFALSE\t0\tsid\tabc\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].name, "sid");
    }
}
