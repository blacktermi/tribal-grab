//! Intégrations personnalisées `.tgi` : JSON + code JavaScript exécuté dans QuickJS.
//! Le code n'a ni disque ni processus : seulement `grab.fetch`, `grab.log` et `grab.result`.
use crate::error::{Error, Result};
use crate::model::*;
use crate::net::Net;
use regex::Regex;
use rquickjs::{Context, Function, Object, Runtime};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

const MAX_FILE: usize = 256 * 1024;
const MEMORY_LIMIT: usize = 32 * 1024 * 1024;
const STACK_LIMIT: usize = 512 * 1024;
const MAX_FETCH: usize = 4 * 1024 * 1024;

pub struct Integrations {
    dir: PathBuf,
    items: RwLock<Vec<IntegrationInfo>>,
}

pub struct RunOutput {
    pub logs: Vec<String>,
    pub result: Result<MediaInfo>,
}

fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    s.trim_matches('-').to_string()
}

fn validate(i: &IntegrationInfo) -> Result<()> {
    if i.name.trim().is_empty() || slug(&i.name).is_empty() {
        return Err(Error::Other("l'intégration doit avoir un nom".into()));
    }
    Regex::new(&i.url_regex).map_err(|e| Error::Other(format!("expression régulière invalide : {e}")))?;
    if i.code.trim().is_empty() {
        return Err(Error::Other("l'intégration ne contient aucun code".into()));
    }
    Ok(())
}

impl Integrations {
    pub fn load(dir: PathBuf) -> Integrations {
        let _ = std::fs::create_dir_all(&dir);
        let mut items = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if e.path().extension().is_some_and(|x| x == "tgi")
                    && let Ok(txt) = std::fs::read_to_string(e.path())
                        && let Ok(i) = serde_json::from_str::<IntegrationInfo>(&txt) {
                            items.push(i);
                        }
            }
        }
        items.sort_by(|a, b| a.name.cmp(&b.name));
        Integrations { dir, items: RwLock::new(items) }
    }

    pub fn list(&self) -> Vec<IntegrationInfo> {
        self.items.read().unwrap().clone()
    }

    pub fn get(&self, name: &str) -> Option<IntegrationInfo> {
        self.items.read().unwrap().iter().find(|i| i.name == name).cloned()
    }

    /// Première intégration activée dont la regex correspond.
    pub fn find(&self, url: &str) -> Option<IntegrationInfo> {
        self.items.read().unwrap().iter().filter(|i| i.enabled).find(|i| Regex::new(&i.url_regex).is_ok_and(|r| r.is_match(url))).cloned()
    }

    /// Installe ou remplace une intégration depuis le contenu JSON d'un `.tgi`.
    pub fn install(&self, json: &str) -> Result<IntegrationInfo> {
        if json.len() > MAX_FILE {
            return Err(Error::Other("fichier .tgi trop volumineux".into()));
        }
        let mut i: IntegrationInfo = serde_json::from_str(json).map_err(|e| Error::Other(format!("fichier .tgi invalide : {e}")))?;
        if i.timeout_secs == 0 {
            i.timeout_secs = 15;
        }
        self.save(i)
    }

    pub fn save(&self, mut i: IntegrationInfo) -> Result<IntegrationInfo> {
        validate(&i)?;
        i.timeout_secs = i.timeout_secs.clamp(1, 120);
        let path = self.dir.join(format!("{}.tgi", slug(&i.name)));
        crate::util::write_atomic(&path, serde_json::to_string_pretty(&i)?.as_bytes())?;
        let mut items = self.items.write().unwrap();
        items.retain(|x| x.name != i.name);
        items.push(i.clone());
        items.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(i)
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        let mut i = self.get(name).ok_or_else(|| Error::Other(format!("intégration « {name} » introuvable")))?;
        i.enabled = enabled;
        self.save(i).map(|_| ())
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        let _ = std::fs::remove_file(self.dir.join(format!("{}.tgi", slug(name))));
        self.items.write().unwrap().retain(|x| x.name != name);
        Ok(())
    }

    pub fn export(&self, name: &str) -> Result<String> {
        let i = self.get(name).ok_or_else(|| Error::Other(format!("intégration « {name} » introuvable")))?;
        Ok(serde_json::to_string_pretty(&i)?)
    }
}

/// Exécute une intégration. Le résultat contient aussi le journal (console de l'éditeur).
pub async fn run(integ: &IntegrationInfo, url: &str, net: &Net) -> RunOutput {
    let logs = Arc::new(Mutex::new(Vec::<String>::new()));
    let (code, timeout, url_s) = (integ.code.clone(), Duration::from_secs(integ.timeout_secs.clamp(1, 120)), url.to_string());
    let (net2, logs2) = (net.clone(), logs.clone());
    let handle = tokio::runtime::Handle::current();
    let r = tokio::task::spawn_blocking(move || exec(&code, &url_s, timeout, net2, logs2, handle)).await;
    let result = match r {
        Ok(x) => x,
        Err(e) => Err(Error::Extract(format!("l'intégration a échoué : {e}"))),
    };
    let logs = logs.lock().unwrap().clone();
    RunOutput { logs, result }
}

fn exec(code: &str, url: &str, timeout: Duration, net: Net, logs: Arc<Mutex<Vec<String>>>, rt_handle: tokio::runtime::Handle) -> Result<MediaInfo> {
    let rt = Runtime::new().map_err(|e| Error::Extract(e.to_string()))?;
    rt.set_memory_limit(MEMORY_LIMIT);
    rt.set_max_stack_size(STACK_LIMIT);
    let deadline = Instant::now() + timeout;
    rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let ctx = Context::full(&rt).map_err(|e| Error::Extract(e.to_string()))?;
    let result: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let res = ctx.with(|ctx| -> rquickjs::Result<()> {
        let grab = Object::new(ctx.clone())?;
        let l = logs.clone();
        grab.set("log", Function::new(ctx.clone(), move |m: String| {
            let mut g = l.lock().unwrap();
            if g.len() < 500 {
                g.push(crate::util::redact(&m.chars().take(2000).collect::<String>()));
            }
        })?)?;
        let r = result.clone();
        // `grab.result(obj)` est défini en JS par-dessus cette fonction : elle reçoit déjà du JSON.
        grab.set("__result", Function::new(ctx.clone(), move |json: String| {
            if json.len() <= 4 * 1024 * 1024 {
                *r.lock().unwrap() = Some(json);
            }
        })?)?;
        let dl = deadline;
        grab.set("fetch", Function::new(ctx.clone(), move |u: String| -> rquickjs::Result<String> {
            let parsed = crate::util::validate_url(&u).map_err(|_| rquickjs::Error::new_from_js_message("string", "url", "adresse http ou https attendue"))?;
            if Instant::now() > dl {
                return Err(rquickjs::Error::new_from_js_message("string", "url", "délai dépassé"));
            }
            rt_handle
                .block_on(net.fetch_text(parsed.as_str(), &Default::default(), MAX_FETCH))
                .map_err(|e| rquickjs::Error::new_from_js_message("string", "url", e.to_string()))
        })?)?;
        let g = ctx.globals();
        g.set("grab", grab)?;
        g.set("url", url.to_string())?;
        ctx.eval::<(), _>("grab.result = function (o) { grab.__result(JSON.stringify(o)); };")?;
        ctx.eval::<(), _>(code.as_bytes().to_vec())?;
        Ok(())
    });
    if let Err(e) = res {
        let msg = if Instant::now() > deadline { "délai dépassé".to_string() } else { e.to_string() };
        return Err(Error::Extract(format!("erreur dans l'intégration : {msg}")));
    }
    let json = result.lock().unwrap().clone().ok_or_else(|| Error::Extract("l'intégration n'a appelé grab.result()".into()))?;
    let v: Value = serde_json::from_str(&json)?;
    media_from_json(&v, url)
}

fn media_from_json(v: &Value, page_url: &str) -> Result<MediaInfo> {
    let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).map(String::from);
    let mut formats = Vec::new();
    let list: Vec<&Value> = match v.get("formats").and_then(Value::as_array) {
        Some(a) => a.iter().collect(),
        None if v.get("url").is_some() => vec![v],
        None => vec![],
    };
    for (n, f) in list.iter().enumerate() {
        let Some(u) = s(f, "url") else { continue };
        crate::util::validate_url(&u)?;
        let is_hls = s(f, "protocol").as_deref() == Some("hls") || u.split('?').next().unwrap_or("").ends_with(".m3u8");
        formats.push(Format {
            id: s(f, "id").unwrap_or_else(|| n.to_string()),
            url: u,
            protocol: if is_hls { Protocol::Hls } else { Protocol::Http },
            ext: s(f, "ext").unwrap_or_else(|| if is_hls { "ts".into() } else { "mp4".into() }),
            vcodec: Some(s(f, "vcodec").unwrap_or_else(|| "unknown".into())),
            acodec: Some(s(f, "acodec").unwrap_or_else(|| "unknown".into())),
            width: f.get("width").and_then(Value::as_u64).map(|x| x as u32),
            height: f.get("height").and_then(Value::as_u64).map(|x| x as u32),
            label: s(f, "label"),
            language: s(f, "language"),
            ..Default::default()
        });
    }
    if formats.is_empty() {
        return Err(Error::Extract("l'intégration n'a renvoyé aucun format".into()));
    }
    let subtitles = v
        .get("subtitles")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|t| Some(SubtitleTrack { lang: s(t, "lang")?, name: s(t, "name"), url: s(t, "url")?, ext: s(t, "ext").unwrap_or_else(|| "vtt".into()), auto: false })).collect())
        .unwrap_or_default();
    Ok(MediaInfo {
        id: s(v, "id").unwrap_or_default(),
        title: s(v, "title").unwrap_or_else(|| "video".into()),
        url: page_url.to_string(),
        site: s(v, "site").unwrap_or_else(|| "Intégration".into()),
        host: crate::util::host_of(page_url),
        uploader: s(v, "uploader"),
        thumbnail: s(v, "thumbnail"),
        duration: v.get("duration").and_then(Value::as_f64),
        formats,
        subtitles,
        referer: Some(page_url.to_string()),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integ(code: &str, timeout: u64) -> IntegrationInfo {
        IntegrationInfo { name: "Test".into(), url_regex: r"^https://exemple\.test/".into(), enabled: true, timeout_secs: timeout, code: code.into(), ..Default::default() }
    }

    fn net() -> Net {
        Net::new(&Settings::default()).unwrap()
    }

    #[tokio::test]
    async fn runs_and_returns_formats() {
        let i = integ(r#"grab.log("bonjour " + url); grab.result({title: "Mon clip", formats: [{url: "https://cdn.test/a.mp4", height: 720}]});"#, 5);
        let out = run(&i, "https://exemple.test/v/1", &net()).await;
        let m = out.result.unwrap();
        assert_eq!(m.title, "Mon clip");
        assert_eq!(m.formats[0].height, Some(720));
        assert!(out.logs[0].contains("bonjour"));
    }

    #[tokio::test]
    async fn no_disk_or_process_access() {
        let i = integ(
            r#"var out = [typeof require, typeof std, typeof os, typeof process, typeof Deno, typeof readFile, typeof XMLHttpRequest, typeof fetch].join(",");
               grab.result({title: out, formats: [{url: "https://cdn.test/a.mp4"}]});"#,
            5,
        );
        let m = run(&i, "https://exemple.test/", &net()).await.result.unwrap();
        assert_eq!(m.title, "undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined");
    }

    #[tokio::test]
    async fn infinite_loop_is_interrupted() {
        let i = integ("while(true){}", 1);
        let t = Instant::now();
        let out = run(&i, "https://exemple.test/", &net()).await;
        assert!(out.result.is_err());
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn memory_bomb_is_stopped() {
        let i = integ("var a=[]; while(true){ a.push(new Array(1000000).fill(1)); }", 10);
        let out = run(&i, "https://exemple.test/", &net()).await;
        assert!(out.result.is_err());
    }

    #[test]
    fn install_find_export() {
        let d = tempfile::tempdir().unwrap();
        let reg = Integrations::load(d.path().to_path_buf());
        let i = reg.install(r#"{"name":"Exemple","url_regex":"^https://exemple\\.test/","code":"grab.result({})","enabled":true}"#).unwrap();
        assert_eq!(i.timeout_secs, 15);
        assert!(reg.find("https://exemple.test/x").is_some());
        assert!(reg.find("https://autre.test/x").is_none());
        reg.set_enabled("Exemple", false).unwrap();
        assert!(reg.find("https://exemple.test/x").is_none());
        let again = Integrations::load(d.path().to_path_buf());
        assert_eq!(again.list().len(), 1);
        assert!(reg.export("Exemple").unwrap().contains("exemple"));
        assert!(reg.install(r#"{"name":"X","url_regex":"(","code":"1"}"#).is_err());
    }
}
