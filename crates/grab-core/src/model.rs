//! Types partagés avec l'interface. Tous sont `Serialize + Deserialize`.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskState {
    #[default]
    Queued,
    Extracting,
    NeedsInput,
    Scheduled,
    Downloading,
    Recording,
    PostProcessing,
    Paused,
    Finished,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Extracting | Self::Downloading | Self::Recording | Self::PostProcessing)
    }
    pub fn is_done(self) -> bool {
        matches!(self, Self::Finished | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Priority {
    High,
    #[default]
    Normal,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Protocol {
    #[default]
    Http,
    /// Playlist HLS (l'URL du format est la playlist média).
    Hls,
    /// Liste de segments déjà résolue (DASH, fragments de yt-dlp).
    Segments,
    /// Format que seul yt-dlp sait télécharger.
    Ytdlp,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Format {
    pub id: String,
    pub url: String,
    pub protocol: Protocol,
    pub ext: String,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<f64>,
    pub tbr: Option<f64>,
    pub filesize: Option<u64>,
    pub language: Option<String>,
    pub label: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub has_drm: bool,
    #[serde(skip)]
    pub plan: Option<std::sync::Arc<SegmentPlan>>,
}

impl Format {
    pub fn has_video(&self) -> bool {
        // Sans aucune information de codec (fréquent chez yt-dlp), le flux est supposé complet.
        match self.vcodec.as_deref() {
            Some(v) => v != "none",
            None => self.height.is_some() || self.acodec.is_none(),
        }
    }
    pub fn has_audio(&self) -> bool {
        match self.acodec.as_deref() {
            Some(a) => a != "none",
            None => self.vcodec.is_none() && self.height.is_none(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Segment {
    pub url: String,
    pub range: Option<(u64, u64)>,
    pub key: Option<SegKey>,
}

#[derive(Debug, Clone)]
pub struct SegKey {
    pub url: String,
    pub iv: [u8; 16],
}

#[derive(Debug, Clone, Default)]
pub struct SegmentPlan {
    pub init: Option<Segment>,
    pub segments: Vec<Segment>,
    pub is_live: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubtitleTrack {
    pub lang: String,
    pub name: Option<String>,
    pub url: String,
    pub ext: String,
    pub auto: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaylistRef {
    pub title: String,
    pub index: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MediaInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    pub site: String,
    pub host: String,
    pub uploader: Option<String>,
    pub upload_date: Option<String>,
    pub timestamp: Option<i64>,
    pub thumbnail: Option<String>,
    pub description: Option<String>,
    pub duration: Option<f64>,
    pub is_live: bool,
    pub formats: Vec<Format>,
    pub subtitles: Vec<SubtitleTrack>,
    pub playlist: Option<PlaylistRef>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// Page d'origine, utilisée comme Referer.
    pub referer: Option<String>,
    /// Fichier quelconque (archive, PDF, installeur…) : enregistré tel quel, sans aucun traitement.
    #[serde(default)]
    pub is_file: bool,
}

/// Un lien proposé à l'utilisateur (playlist, page contenant plusieurs médias).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinkCandidate {
    pub url: String,
    pub title: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub size: Option<u64>,
    pub kind: Option<String>,
    /// Reconnu par une extraction native ou yt-dlp.
    pub recognized: bool,
}

/// Résultat de l'analyse d'une URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Resolved {
    Single(Box<MediaInfo>),
    Playlist { title: String, entries: Vec<LinkCandidate> },
    Links(Vec<LinkCandidate>),
    Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PostMode {
    #[default]
    Default,
    None,
    Mp4,
    AudioOnly,
    SendToApp,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AudioFormat {
    #[default]
    M4a,
    Mp3,
    Original,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Container {
    PreferMp4,
    #[default]
    ForceMp4,
    Original,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SubMode {
    #[default]
    None,
    IfAvailable,
    Required,
    Only,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SubOut {
    #[default]
    Srt,
    Vtt,
    Embed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DuplicatePolicy {
    #[default]
    Ask,
    Redownload,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AfterQueue {
    #[default]
    Nothing,
    Quit,
    Sleep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UnsupportedLink {
    #[default]
    Ignore,
    Guided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScriptWhen {
    #[default]
    PerFile,
    EndOfQueue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HistoryRetention {
    Week,
    Month,
    ThreeMonths,
    SixMonths,
    Year,
    #[default]
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxySettings {
    /// Exemple : `http://hote:3128`, `socks5://hote:1080`. Sans identifiants.
    pub url: String,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // Général
    pub auto_start: bool,
    pub max_concurrent: u32,
    pub connections: u32,
    pub speed_limit_kbps: u32,
    pub automated_mode: bool,
    pub after_queue: AfterQueue,
    pub retry_max: u32,
    pub retry_base_secs: u32,
    pub unsupported_link: UnsupportedLink,
    pub duplicate_policy: DuplicatePolicy,
    // Fichiers
    pub download_dir: String,
    pub name_template: String,
    pub ask_destination: bool,
    pub min_free_mb: u64,
    // Qualité
    pub max_resolution: Option<u32>,
    pub quality_prompt: bool,
    pub container: Container,
    pub audio_format: Option<AudioFormat>,
    pub audio_languages: Vec<String>,
    // Post-traitement
    pub post_mode: PostMode,
    pub reencode: bool,
    pub keep_original: bool,
    /// Quand QuickTime impose une conversion, garde aussi le flux d'origine, sans perte, à côté.
    pub keep_source_on_convert: bool,
    pub embed_metadata: bool,
    pub embed_thumbnail: bool,
    pub save_thumbnail: bool,
    pub save_json: bool,
    pub script: Option<String>,
    pub script_when: ScriptWhen,
    pub send_to_app: Option<String>,
    pub add_to_music: bool,
    // Sous-titres
    pub sub_mode: SubMode,
    pub sub_langs: Vec<String>,
    pub sub_multiple: bool,
    pub sub_auto: bool,
    pub sub_label_regex: Option<String>,
    pub sub_out: SubOut,
    // Réseau
    pub proxy: Option<ProxySettings>,
    pub user_agent: Option<String>,
    pub timeout_secs: u64,
    pub fragmented: bool,
    pub xff: Option<String>,
    pub cookies_browser: Option<String>,
    pub cookies_file: Option<String>,
    // Direct
    pub live_mpegts: bool,
    pub live_min_free_mb: u64,
    // Historique
    pub history_retention: HistoryRetention,
    // Recherche
    pub search_engines: Vec<String>,
    // Outils
    pub tool_paths: BTreeMap<String, String>,
    pub notifications: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        Self {
            auto_start: true,
            max_concurrent: 3,
            connections: 8,
            speed_limit_kbps: 0,
            automated_mode: false,
            after_queue: AfterQueue::Nothing,
            retry_max: 3,
            retry_base_secs: 5,
            unsupported_link: UnsupportedLink::Ignore,
            duplicate_policy: DuplicatePolicy::Ask,
            download_dir: format!("{home}/Downloads"),
            name_template: "{title}".into(),
            ask_destination: false,
            min_free_mb: 200,
            max_resolution: None,
            quality_prompt: false,
            container: Container::ForceMp4,
            audio_format: None,
            audio_languages: vec![],
            post_mode: PostMode::Mp4,
            reencode: false,
            keep_original: false,
            keep_source_on_convert: true,
            embed_metadata: true,
            embed_thumbnail: false,
            save_thumbnail: false,
            save_json: false,
            script: None,
            script_when: ScriptWhen::PerFile,
            send_to_app: None,
            add_to_music: false,
            sub_mode: SubMode::None,
            sub_langs: vec!["fr".into(), "en".into()],
            sub_multiple: false,
            sub_auto: false,
            sub_label_regex: None,
            sub_out: SubOut::Srt,
            proxy: None,
            user_agent: None,
            timeout_secs: 30,
            fragmented: true,
            xff: None,
            cookies_browser: None,
            cookies_file: None,
            live_mpegts: false,
            live_min_free_mb: 500,
            history_retention: HistoryRetention::Never,
            search_engines: vec!["youtube".into(), "soundcloud".into()],
            tool_paths: BTreeMap::new(),
            notifications: true,
        }
    }
}

/// Réglages propres à un téléchargement. Les champs vides prennent la valeur des réglages globaux.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskOptions {
    pub post_mode: PostMode,
    pub dest_dir: Option<String>,
    pub name_template: Option<String>,
    pub max_resolution: Option<u32>,
    pub format_id: Option<String>,
    pub audio_format: Option<AudioFormat>,
    pub sub_mode: Option<SubMode>,
    pub sub_langs: Option<Vec<String>>,
    pub duplicate: Option<DuplicatePolicy>,
    pub speed_limit_kbps: Option<u32>,
    pub custom_script: Option<String>,
    /// Indication transmise par la playlist d'origine.
    pub playlist: Option<PlaylistRef>,
    pub title_hint: Option<String>,
    /// Page d'où vient le lien (envoyée comme Referer).
    pub referer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub percent: f32,
    /// Octets par seconde.
    pub speed: u64,
    pub eta_secs: Option<u64>,
    /// Libellé de l'étape en cours (« Vidéo », « Audio », « Fusion »…).
    pub stage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Task {
    pub id: i64,
    pub url: String,
    pub state: TaskState,
    pub priority: Priority,
    pub position: i64,
    pub title: Option<String>,
    pub site: Option<String>,
    pub thumbnail: Option<String>,
    pub playlist: Option<String>,
    pub options: TaskOptions,
    pub progress: Progress,
    pub output_path: Option<String>,
    /// Destination choisie (sans le `.tgpart`), conservée pour reprendre au même endroit.
    pub target: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub scheduled_at: Option<i64>,
    pub stop_at: Option<i64>,
    pub retries: u32,
    pub question: Option<Question>,
    #[serde(default)]
    pub is_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatChoice {
    pub id: String,
    pub label: String,
    pub height: Option<u32>,
    pub ext: String,
    pub size: Option<u64>,
    pub audio_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Question {
    Quality { choices: Vec<FormatChoice> },
    Subtitles { tracks: Vec<SubtitleTrack> },
    Links { title: String, links: Vec<LinkCandidate> },
    Duplicate { previous: HistoryItem },
    Destination { suggested: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DuplicateAction {
    Redownload,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Answer {
    Quality { format_id: String },
    Subtitles { langs: Vec<String> },
    Links { urls: Vec<String>, as_playlist: bool },
    Duplicate { action: DuplicateAction },
    Destination { dir: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryItem {
    pub id: i64,
    pub url: String,
    pub media_id: Option<String>,
    pub title: String,
    pub site: String,
    pub path: Option<String>,
    pub size: Option<u64>,
    pub thumbnail: Option<String>,
    pub finished_at: i64,
    pub exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortKey {
    #[default]
    Order,
    Site,
    Playlist,
    Progress,
    Name,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AddRequest {
    pub url: String,
    pub options: TaskOptions,
    pub priority: Priority,
    pub scheduled_at: Option<i64>,
    pub stop_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolStatus {
    pub name: String,
    pub path: Option<String>,
    pub version: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct IntegrationInfo {
    pub name: String,
    pub url_regex: String,
    pub enabled: bool,
    pub timeout_secs: u64,
    pub requires_account: bool,
    pub example_url: Option<String>,
    pub comment: Option<String>,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub engine: String,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub uploader: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteList {
    pub total: usize,
    pub sites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    TaskAdded(Box<Task>),
    TaskUpdated(Box<Task>),
    TaskProgress { id: i64, progress: Progress },
    TaskRemoved { id: i64 },
    QueueFinished,
    Log { id: Option<i64>, message: String },
    Notification { title: String, body: String },
    /// L'application doit quitter ou mettre le Mac en veille (réglage « à la fin de la file »).
    AfterQueue(AfterQueue),
    ToolsChanged,
    /// Lien non pris en charge et réglage « extraction guidée » : l'application ouvre le navigateur intégré.
    GuidedExtraction { id: i64, url: String },
}
