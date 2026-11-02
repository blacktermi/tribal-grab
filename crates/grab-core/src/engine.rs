//! Façade `Engine` : file d'attente, planificateur, réglages, historique, outils.
use crate::db::Db;
use crate::error::{Error, Result};
use crate::extract::Extractor;
use crate::history_io;
use crate::integrations::{self, Integrations};
use crate::model::*;
use crate::net::Net;
use crate::util::{Throttle, now};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{Notify, broadcast};
use tokio_util::sync::CancellationToken;

pub(crate) const STOP_PAUSE: u8 = 1;
pub(crate) const STOP_CANCEL: u8 = 2;
pub(crate) const STOP_LIVE: u8 = 3;

#[derive(Clone)]
pub(crate) struct RunHandle {
    pub cancel: CancellationToken,
    pub reason: Arc<AtomicU8>,
    pub throttle: Arc<Throttle>,
}

pub(crate) struct Entry {
    pub task: Task,
    pub run: Option<RunHandle>,
    pub remove_when_done: bool,
}

#[derive(Default)]
pub(crate) struct State {
    pub tasks: BTreeMap<i64, Entry>,
    pub queue_started: bool,
    pub was_busy: bool,
    /// Fichiers terminés depuis le dernier vidage de la file (script de fin de file).
    pub finished_files: Vec<(String, String, String)>,
}

pub(crate) struct Inner {
    pub data_dir: PathBuf,
    pub db: Db,
    pub tx: broadcast::Sender<Event>,
    pub settings: RwLock<Settings>,
    pub net: RwLock<Net>,
    pub integrations: Arc<Integrations>,
    pub state: Mutex<State>,
    pub wake: Notify,
    pub sites_cache: Mutex<Option<Vec<String>>>,
}

/// Façade partageable avec `Arc<Engine>` ; toutes les méthodes prennent `&self`.
#[derive(Clone)]
pub struct Engine {
    pub(crate) inner: Arc<Inner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationTest {
    pub logs: Vec<String>,
    pub result: Option<MediaInfo>,
    pub error: Option<String>,
}

impl Inner {
    pub fn emit(&self, e: Event) {
        let _ = self.tx.send(e);
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn task(&self, id: i64) -> Option<Task> {
        self.state.lock().unwrap().tasks.get(&id).map(|e| e.task.clone())
    }

    pub fn persist(&self, t: &Task) {
        if let Err(e) = self.db.task_save(t) {
            tracing::error!("sauvegarde de la tâche impossible : {e}");
        }
    }

    /// Modifie une tâche, la sauvegarde et prévient l'interface.
    pub fn update(&self, id: i64, f: impl FnOnce(&mut Task)) -> Option<Task> {
        let t = {
            let mut st = self.state.lock().unwrap();
            let e = st.tasks.get_mut(&id)?;
            f(&mut e.task);
            e.task.clone()
        };
        self.persist(&t);
        self.emit(Event::TaskUpdated(Box::new(t.clone())));
        Some(t)
    }

    pub fn set_state(&self, id: i64, s: TaskState) -> Option<Task> {
        self.update(id, |t| t.state = s)
    }

    pub fn log(&self, id: Option<i64>, msg: impl Into<String>) {
        self.emit(Event::Log { id, message: crate::util::redact(&msg.into()) });
    }

    pub fn insert_task(&self, mut t: Task) -> Result<Task> {
        {
            let st = self.state.lock().unwrap();
            t.position = st.tasks.values().map(|e| e.task.position).max().unwrap_or(0) + 1;
        }
        self.db.task_insert(&mut t)?;
        self.state.lock().unwrap().tasks.insert(t.id, Entry { task: t.clone(), run: None, remove_when_done: false });
        self.emit(Event::TaskAdded(Box::new(t.clone())));
        self.wake.notify_one();
        Ok(t)
    }

    pub fn new_task(&self, url: &str, options: TaskOptions, priority: Priority, scheduled_at: Option<i64>, stop_at: Option<i64>) -> Task {
        let future = scheduled_at.is_some_and(|s| s > now());
        Task {
            url: url.to_string(),
            state: if future { TaskState::Scheduled } else { TaskState::Queued },
            priority,
            options,
            created_at: now(),
            scheduled_at,
            stop_at,
            ..Default::default()
        }
    }

    pub fn remove_entry(&self, id: i64) {
        self.state.lock().unwrap().tasks.remove(&id);
        let _ = self.db.task_delete(id);
        self.emit(Event::TaskRemoved { id });
    }

    pub fn discard_partial(task: &Task) {
        if let Some(t) = &task.target {
            let _ = std::fs::remove_dir_all(crate::naming::part_dir(Path::new(t)));
        }
    }

    pub fn extractor(&self) -> Extractor {
        Extractor { net: self.net.read().unwrap().clone(), settings: self.settings(), data_dir: self.data_dir.clone(), integrations: self.integrations.clone() }
    }
}

fn prio_rank(p: Priority) -> u8 {
    match p {
        Priority::High => 0,
        Priority::Normal => 1,
        Priority::Low => 2,
    }
}

impl Engine {
    /// Démarre le moteur avec son dossier de données (base SQLite, intégrations, `bin/`).
    /// Doit être appelé depuis un runtime tokio.
    pub async fn start(data_dir: impl Into<PathBuf>) -> Result<Engine> {
        let data_dir: PathBuf = data_dir.into();
        std::fs::create_dir_all(&data_dir)?;
        let db = Db::open(&data_dir.join("grab.sqlite"))?;
        let settings: Settings = db.kv_get("settings")?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let net = Net::new(&settings)?;
        let (tx, _) = broadcast::channel(2048);
        let integrations = Arc::new(Integrations::load(data_dir.join("integrations")));
        let mut tasks = BTreeMap::new();
        for mut t in db.tasks_load()? {
            // Une tâche interrompue par un arrêt de l'application reprend où elle en était.
            if t.state.is_active() {
                t.state = TaskState::Queued;
            }
            tasks.insert(t.id, Entry { task: t, run: None, remove_when_done: false });
        }
        let inner = Arc::new(Inner {
            data_dir,
            db,
            tx,
            settings: RwLock::new(settings.clone()),
            net: RwLock::new(net),
            integrations,
            state: Mutex::new(State { tasks, ..Default::default() }),
            wake: Notify::new(),
            sites_cache: Mutex::new(None),
        });
        let _ = inner.db.history_prune(settings.history_retention);
        let i2 = inner.clone();
        tokio::spawn(async move { scheduler(i2).await });
        Ok(Engine { inner })
    }

    pub fn data_dir(&self) -> &Path {
        &self.inner.data_dir
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.inner.tx.subscribe()
    }

    // --- Ajout ---

    pub fn add(&self, req: AddRequest) -> Result<i64> {
        let u = crate::util::validate_url(&req.url)?;
        let t = self.inner.new_task(u.as_str(), req.options, req.priority, req.scheduled_at, req.stop_at);
        Ok(self.inner.insert_task(t)?.id)
    }

    pub fn add_url(&self, url: &str) -> Result<i64> {
        self.add(AddRequest { url: url.to_string(), ..Default::default() })
    }

    /// Extrait toutes les adresses http(s) d'un texte collé et les ajoute.
    pub fn add_text(&self, text: &str) -> Result<Vec<i64>> {
        let re = regex::Regex::new(r#"https?://[^\s<>"'`]+"#).unwrap();
        let mut seen = std::collections::HashSet::new();
        let mut ids = Vec::new();
        for m in re.find_iter(text) {
            let u = m.as_str().trim_end_matches(['.', ',', ';', ')', ']', '!', '?']);
            if seen.insert(u.to_string())
                && let Ok(id) = self.add_url(u) {
                    ids.push(id);
                }
        }
        if ids.is_empty() {
            return Err(Error::InvalidUrl("aucune adresse http ou https trouvée dans le texte".into()));
        }
        Ok(ids)
    }

    /// Analyse une URL sans créer de tâche (détails d'un lien, choix de playlist).
    pub async fn probe(&self, url: &str) -> Result<Resolved> {
        self.inner.extractor().resolve(url, None).await
    }

    // --- Liste et actions ---

    pub fn task(&self, id: i64) -> Option<Task> {
        self.inner.task(id)
    }

    pub fn tasks(&self, sort: SortKey, desc: bool) -> Vec<Task> {
        let mut v: Vec<Task> = self.inner.state.lock().unwrap().tasks.values().map(|e| e.task.clone()).collect();
        match sort {
            SortKey::Order => v.sort_by_key(|t| t.position),
            SortKey::Site => v.sort_by_key(|t| (t.site.clone().unwrap_or_default().to_lowercase(), t.position)),
            SortKey::Playlist => v.sort_by_key(|t| (t.playlist.clone().unwrap_or_default().to_lowercase(), t.position)),
            SortKey::Progress => v.sort_by(|a, b| a.progress.percent.partial_cmp(&b.progress.percent).unwrap_or(std::cmp::Ordering::Equal)),
            SortKey::Name => v.sort_by_key(|t| t.title.clone().unwrap_or_else(|| t.url.clone()).to_lowercase()),
        }
        if desc {
            v.reverse();
        }
        v
    }

    fn handle_of(&self, id: i64) -> Result<(Task, Option<RunHandle>)> {
        let st = self.inner.state.lock().unwrap();
        st.tasks.get(&id).map(|e| (e.task.clone(), e.run.clone())).ok_or(Error::NoTask(id))
    }

    fn stop_run(h: &RunHandle, reason: u8) {
        h.reason.store(reason, Ordering::SeqCst);
        h.cancel.cancel();
    }

    pub fn pause(&self, id: i64) -> Result<()> {
        let (t, run) = self.handle_of(id)?;
        match run {
            Some(h) => Self::stop_run(&h, STOP_PAUSE),
            None if matches!(t.state, TaskState::Queued | TaskState::Scheduled) => {
                self.inner.set_state(id, TaskState::Paused);
            }
            None => {}
        }
        Ok(())
    }

    pub fn resume(&self, id: i64) -> Result<()> {
        let (t, _) = self.handle_of(id)?;
        if matches!(t.state, TaskState::Paused | TaskState::Failed | TaskState::Cancelled) {
            self.inner.update(id, |t| {
                t.state = TaskState::Queued;
                t.error = None;
                t.retries = 0;
            });
            self.inner.wake.notify_one();
        }
        Ok(())
    }

    /// Relance un téléchargement échoué, annulé ou terminé (repart de zéro si besoin).
    pub fn retry(&self, id: i64) -> Result<()> {
        let (t, run) = self.handle_of(id)?;
        if run.is_some() {
            return Ok(());
        }
        if t.state == TaskState::Finished || t.state == TaskState::Cancelled {
            Inner::discard_partial(&t);
        }
        let was_finished = t.state == TaskState::Finished;
        self.inner.update(id, |t| {
            t.state = TaskState::Queued;
            t.error = None;
            t.retries = 0;
            t.progress = Progress::default();
            if was_finished {
                t.output_path = None;
            }
            t.target = None;
            t.options.duplicate = Some(DuplicatePolicy::Redownload);
        });
        self.inner.wake.notify_one();
        Ok(())
    }

    pub fn cancel(&self, id: i64) -> Result<()> {
        let (t, run) = self.handle_of(id)?;
        match run {
            Some(h) => Self::stop_run(&h, STOP_CANCEL),
            None if !t.state.is_done() => {
                Inner::discard_partial(&t);
                self.inner.update(id, |t| t.state = TaskState::Cancelled);
            }
            None => {}
        }
        Ok(())
    }

    pub fn remove(&self, id: i64) -> Result<()> {
        let (t, run) = self.handle_of(id)?;
        match run {
            Some(h) => {
                if let Some(e) = self.inner.state.lock().unwrap().tasks.get_mut(&id) {
                    e.remove_when_done = true;
                }
                Self::stop_run(&h, STOP_CANCEL);
            }
            None => {
                if !matches!(t.state, TaskState::Finished) {
                    Inner::discard_partial(&t);
                }
                self.inner.remove_entry(id);
            }
        }
        Ok(())
    }

    fn ids_where(&self, f: impl Fn(&Task) -> bool) -> Vec<i64> {
        self.inner.state.lock().unwrap().tasks.values().filter(|e| f(&e.task)).map(|e| e.task.id).collect()
    }

    pub fn pause_all(&self) {
        self.inner.state.lock().unwrap().queue_started = false;
        for id in self.ids_where(|t| !t.state.is_done() && t.state != TaskState::Paused && t.state != TaskState::NeedsInput) {
            let _ = self.pause(id);
        }
    }

    pub fn resume_all(&self) {
        for id in self.ids_where(|t| t.state == TaskState::Paused) {
            let _ = self.resume(id);
        }
        self.start_queue();
    }

    pub fn cancel_all(&self) {
        for id in self.ids_where(|t| !t.state.is_done()) {
            let _ = self.cancel(id);
        }
    }

    /// Démarrage manuel : lance la file même si le démarrage automatique est désactivé.
    pub fn start_queue(&self) {
        self.inner.state.lock().unwrap().queue_started = true;
        self.inner.wake.notify_one();
    }

    pub fn clear_finished(&self) {
        for id in self.ids_where(|t| t.state == TaskState::Finished) {
            let _ = self.remove(id);
        }
    }

    /// Retire les tâches inactives : terminées, échouées ou annulées.
    pub fn clear_inactive(&self) {
        for id in self.ids_where(|t| t.state.is_done()) {
            let _ = self.remove(id);
        }
    }

    pub fn reset_queue(&self) {
        for id in self.ids_where(|_| true) {
            let _ = self.remove(id);
        }
    }

    pub fn set_priority(&self, id: i64, p: Priority) -> Result<()> {
        self.handle_of(id)?;
        self.inner.update(id, |t| t.priority = p);
        self.inner.wake.notify_one();
        Ok(())
    }

    /// Place la tâche à une position donnée (0 = en tête) dans l'ordre manuel de la file.
    pub fn move_task(&self, id: i64, index: usize) -> Result<()> {
        self.handle_of(id)?;
        let mut order: Vec<Task> = self.tasks(SortKey::Order, false);
        let from = order.iter().position(|t| t.id == id).ok_or(Error::NoTask(id))?;
        let t = order.remove(from);
        order.insert(index.min(order.len()), t);
        for (i, t) in order.iter().enumerate() {
            let pos = i as i64 + 1;
            if t.position != pos {
                self.inner.update(t.id, |x| x.position = pos);
            }
        }
        Ok(())
    }

    pub fn set_schedule(&self, id: i64, scheduled_at: Option<i64>, stop_at: Option<i64>) -> Result<()> {
        let (t, _) = self.handle_of(id)?;
        self.inner.update(id, |t| {
            t.scheduled_at = scheduled_at;
            t.stop_at = stop_at;
            if matches!(t.state, TaskState::Queued | TaskState::Scheduled) {
                t.state = if scheduled_at.is_some_and(|s| s > now()) { TaskState::Scheduled } else { TaskState::Queued };
            }
        });
        let _ = t;
        self.inner.wake.notify_one();
        Ok(())
    }

    /// Arrête un enregistrement de direct en gardant ce qui a été enregistré.
    pub fn stop_recording(&self, id: i64) -> Result<()> {
        let (_, run) = self.handle_of(id)?;
        if let Some(h) = run {
            Self::stop_run(&h, STOP_LIVE);
        }
        Ok(())
    }

    pub fn set_task_speed_limit(&self, id: i64, kbps: u32) -> Result<()> {
        let (_, run) = self.handle_of(id)?;
        self.inner.update(id, |t| t.options.speed_limit_kbps = Some(kbps));
        if let Some(h) = run {
            h.throttle.set(kbps);
        }
        Ok(())
    }

    /// Réponse à une question (`NeedsInput`).
    pub fn answer(&self, id: i64, a: Answer) -> Result<()> {
        let (t, _) = self.handle_of(id)?;
        if t.state != TaskState::NeedsInput {
            return Err(Error::Other("cette tâche n'attend aucune réponse".into()));
        }
        match (a, t.question.clone()) {
            (Answer::Quality { format_id }, Some(Question::Quality { .. })) => {
                self.inner.update(id, |t| {
                    t.options.format_id = Some(format_id);
                    t.question = None;
                    t.state = TaskState::Queued;
                });
            }
            (Answer::Subtitles { langs }, Some(Question::Subtitles { .. })) => {
                self.inner.update(id, |t| {
                    t.options.sub_langs = Some(langs);
                    t.question = None;
                    t.state = TaskState::Queued;
                });
            }
            (Answer::Destination { dir }, Some(Question::Destination { .. })) => {
                self.inner.update(id, |t| {
                    t.options.dest_dir = Some(dir);
                    t.question = None;
                    t.state = TaskState::Queued;
                });
            }
            (Answer::Duplicate { action }, Some(Question::Duplicate { .. })) => match action {
                DuplicateAction::Redownload => {
                    self.inner.update(id, |t| {
                        t.options.duplicate = Some(DuplicatePolicy::Redownload);
                        t.question = None;
                        t.state = TaskState::Queued;
                    });
                }
                DuplicateAction::Skip => {
                    self.inner.update(id, |t| {
                        t.question = None;
                        t.state = TaskState::Cancelled;
                        t.error = Some("Ignoré : déjà téléchargé".into());
                    });
                }
            },
            (Answer::Links { urls, as_playlist }, Some(Question::Links { title, links })) => {
                let count = links.len() as u32;
                for u in urls {
                    let Some(idx) = links.iter().position(|l| l.url == u) else { continue };
                    let mut o = t.options.clone();
                    o.referer = if links[idx].recognized { t.options.referer.clone() } else { Some(t.url.clone()) };
                    o.title_hint = links[idx].title.clone();
                    o.format_id = None;
                    if as_playlist {
                        o.playlist = Some(PlaylistRef { title: title.clone(), index: idx as u32 + 1, count });
                    }
                    let child = self.inner.new_task(&u, o, t.priority, None, None);
                    self.inner.insert_task(child)?;
                }
                self.inner.remove_entry(id);
            }
            _ => return Err(Error::Other("la réponse ne correspond pas à la question posée".into())),
        }
        self.inner.wake.notify_one();
        Ok(())
    }

    // --- Réglages ---

    pub fn settings(&self) -> Settings {
        self.inner.settings()
    }

    pub fn set_settings(&self, s: Settings) -> Result<()> {
        crate::naming::validate_template(&s.name_template)?;
        let mut s = s;
        s.max_concurrent = s.max_concurrent.clamp(1, 16);
        s.connections = s.connections.clamp(1, 32);
        if s.connections > 8 {
            self.inner.log(None, "Avertissement : plus de 8 connexions par téléchargement peut faire bloquer votre adresse par certains serveurs.");
        }
        if let Some(r) = &s.sub_label_regex {
            regex::Regex::new(r).map_err(|e| Error::Other(format!("expression régulière des libellés invalide : {e}")))?;
        }
        let net = Net::new(&s)?;
        self.inner.db.kv_set("settings", &serde_json::to_string(&s)?)?;
        let retention = s.history_retention;
        *self.inner.settings.write().unwrap() = s;
        *self.inner.net.write().unwrap() = net;
        let _ = self.inner.db.history_prune(retention);
        self.inner.emit(Event::ToolsChanged);
        self.inner.wake.notify_one();
        Ok(())
    }

    /// Le mot de passe du proxy va dans le trousseau macOS, jamais dans la base.
    pub fn set_proxy_password(&self, password: &str) -> Result<()> {
        if password.is_empty() {
            crate::secrets::delete_proxy_password();
            Ok(())
        } else {
            crate::secrets::set_proxy_password(password)
        }
    }

    // --- Historique ---

    pub fn history(&self, query: &str, sort: &str, desc: bool, limit: i64) -> Result<Vec<HistoryItem>> {
        self.inner.db.history_list(query, sort, desc, limit)
    }

    pub fn history_delete(&self, ids: &[i64]) -> Result<()> {
        self.inner.db.history_delete(ids)
    }

    pub fn history_clear(&self) -> Result<()> {
        self.inner.db.history_clear()
    }

    pub fn history_remove_missing(&self) -> Result<usize> {
        self.inner.db.history_remove_missing()
    }

    pub fn history_redownload(&self, id: i64) -> Result<i64> {
        let h = self.inner.db.history_get(id)?.ok_or_else(|| Error::Other("entrée d'historique introuvable".into()))?;
        let o = TaskOptions { duplicate: Some(DuplicatePolicy::Redownload), ..Default::default() };
        self.add(AddRequest { url: h.url, options: o, ..Default::default() })
    }

    pub fn history_export(&self, f: history_io::Format) -> Result<String> {
        history_io::export(&self.inner.db.history_list("", "date", true, 10_000_000)?, f)
    }

    pub fn history_import(&self, f: history_io::Format, data: &str) -> Result<usize> {
        let existing = self.inner.db.history_list("", "date", true, 10_000_000)?;
        let mut n = 0;
        for h in history_io::import(data, f)? {
            if existing.iter().any(|e| e.url == h.url && e.finished_at == h.finished_at) {
                continue;
            }
            self.inner.db.history_add(&h)?;
            n += 1;
        }
        Ok(n)
    }

    /// Affiche un fichier dans le Finder.
    pub fn reveal_in_finder(&self, path: &str) -> Result<()> {
        std::process::Command::new("/usr/bin/open").args(["-R", "--", path]).spawn()?;
        Ok(())
    }

    pub fn open_path(&self, path: &str) -> Result<()> {
        std::process::Command::new("/usr/bin/open").args(["--", path]).spawn()?;
        Ok(())
    }

    // --- Recherche et sites ---

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let s = self.settings();
        crate::search::search(&s, &self.inner.data_dir, query, &s.search_engines, 10).await
    }

    pub async fn sites(&self, filter: &str) -> Result<SiteList> {
        let cached = self.inner.sites_cache.lock().unwrap().clone();
        let all = match cached {
            Some(a) => a,
            None => {
                let a = crate::search::list_sites(&self.settings(), &self.inner.data_dir).await?;
                *self.inner.sites_cache.lock().unwrap() = Some(a.clone());
                a
            }
        };
        Ok(crate::search::filter_sites(&all, filter))
    }

    // --- Intégrations ---

    pub fn integrations(&self) -> Vec<IntegrationInfo> {
        self.inner.integrations.list()
    }

    pub fn install_integration(&self, tgi_json: &str) -> Result<IntegrationInfo> {
        self.inner.integrations.install(tgi_json)
    }

    pub fn save_integration(&self, i: IntegrationInfo) -> Result<IntegrationInfo> {
        self.inner.integrations.save(i)
    }

    pub fn set_integration_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        self.inner.integrations.set_enabled(name, enabled)
    }

    pub fn remove_integration(&self, name: &str) -> Result<()> {
        self.inner.integrations.remove(name)
    }

    pub fn export_integration(&self, name: &str) -> Result<String> {
        self.inner.integrations.export(name)
    }

    /// Console de l'éditeur : exécute le code fourni sur une URL sans rien enregistrer.
    pub async fn test_integration(&self, info: IntegrationInfo, url: &str) -> IntegrationTest {
        let net = self.inner.net.read().unwrap().clone();
        let out = integrations::run(&info, url, &net).await;
        match out.result {
            Ok(m) => IntegrationTest { logs: out.logs, result: Some(m), error: None },
            Err(e) => IntegrationTest { logs: out.logs, result: None, error: Some(e.to_string()) },
        }
    }

    // --- Outils ---

    pub async fn tools(&self) -> Vec<ToolStatus> {
        crate::tools::status(&self.settings(), &self.inner.data_dir).await
    }

    pub fn set_tool_path(&self, name: &str, path: Option<String>) -> Result<()> {
        let mut s = self.settings();
        match path.filter(|p| !p.is_empty()) {
            Some(p) => s.tool_paths.insert(name.to_string(), p),
            None => s.tool_paths.remove(name),
        };
        self.set_settings(s)
    }

    pub async fn check_ytdlp_update(&self) -> Result<crate::update::UpdateInfo> {
        let s = self.settings();
        let current = match crate::tools::find("yt-dlp", &s, &self.inner.data_dir) {
            Some((p, _)) => crate::tools::version(&p, "yt-dlp").await,
            None => None,
        };
        let net = self.inner.net.read().unwrap().clone();
        crate::update::check(&net, current).await
    }

    /// Télécharge la dernière version officielle de yt-dlp, vérifie son SHA256 et l'installe dans `bin/`.
    pub async fn update_ytdlp(&self) -> Result<String> {
        let net = self.inner.net.read().unwrap().clone();
        let v = crate::update::install_ytdlp(&net, &self.inner.data_dir.join("bin")).await?;
        self.inner.emit(Event::ToolsChanged);
        Ok(v)
    }
}

/// Boucle du planificateur : réveille les tâches différées, lance les téléchargements, détecte la fin de file.
async fn scheduler(inner: Arc<Inner>) {
    loop {
        tokio::select! {
            _ = inner.wake.notified() => {}
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
        }
        schedule_pass(&inner).await;
    }
}

async fn schedule_pass(inner: &Arc<Inner>) {
    let s = inner.settings();
    let t_now = now();
    let mut to_start: Vec<(i64, RunHandle)> = Vec::new();
    let (just_finished, finished_files) = {
        let mut st = inner.state.lock().unwrap();
        let woken: Vec<i64> = st.tasks.values().filter(|e| e.task.state == TaskState::Scheduled && e.task.scheduled_at.is_none_or(|x| x <= t_now)).map(|e| e.task.id).collect();
        for id in woken {
            if let Some(e) = st.tasks.get_mut(&id) {
                e.task.state = TaskState::Queued;
                let t = e.task.clone();
                inner.persist(&t);
                inner.emit(Event::TaskUpdated(Box::new(t)));
            }
        }
        let active = st.tasks.values().filter(|e| e.run.is_some()).count();
        let mut free = (s.max_concurrent as usize).saturating_sub(active);
        if s.auto_start || st.queue_started {
            let mut queued: Vec<&Entry> = st.tasks.values().filter(|e| e.task.state == TaskState::Queued && e.run.is_none()).collect();
            queued.sort_by_key(|e| (prio_rank(e.task.priority), e.task.position));
            let ids: Vec<(i64, u32)> = queued.iter().map(|e| (e.task.id, e.task.options.speed_limit_kbps.unwrap_or(s.speed_limit_kbps))).collect();
            for (id, kbps) in ids {
                if free == 0 {
                    break;
                }
                let h = RunHandle { cancel: CancellationToken::new(), reason: Arc::new(AtomicU8::new(0)), throttle: Arc::new(Throttle::new(kbps)) };
                if let Some(e) = st.tasks.get_mut(&id) {
                    e.run = Some(h.clone());
                    to_start.push((id, h));
                    free -= 1;
                }
            }
        }
        let busy = st.tasks.values().any(|e| {
            e.run.is_some() || matches!(e.task.state, TaskState::Queued | TaskState::Scheduled | TaskState::NeedsInput | TaskState::Extracting)
        });
        let mut ff = Vec::new();
        if st.was_busy && !busy {
            ff = std::mem::take(&mut st.finished_files);
            st.queue_started = false;
        }
        let was = st.was_busy;
        st.was_busy = busy;
        (was && !busy, ff)
    };
    for (id, h) in to_start {
        let i = inner.clone();
        tokio::spawn(async move { crate::runner::run(i, id, h).await });
    }
    if just_finished {
        inner.emit(Event::QueueFinished);
        if s.script_when == ScriptWhen::EndOfQueue
            && let (Some(script), Some((file, title, url))) = (s.script.clone().filter(|x| !x.trim().is_empty()), finished_files.last().cloned()) {
                let env = [("TG_FILE", file), ("TG_TITLE", title), ("TG_URL", url), ("TG_SITE", String::new()), ("TG_PLAYLIST", String::new())];
                if let Err(e) = crate::post::run_script(&script, &env).await {
                    inner.log(None, e.to_string());
                }
            }
        match s.after_queue {
            AfterQueue::Nothing => {}
            AfterQueue::Quit => inner.emit(Event::AfterQueue(AfterQueue::Quit)),
            AfterQueue::Sleep => {
                let _ = std::process::Command::new("/usr/bin/pmset").arg("sleepnow").spawn();
                inner.emit(Event::AfterQueue(AfterQueue::Sleep));
            }
        }
    }
}
