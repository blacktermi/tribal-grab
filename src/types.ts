// Types miroir de `grab_core::model` (sérialisation serde par défaut : enums externes, unités en chaîne).
export type TaskState =
  | "Queued" | "Extracting" | "NeedsInput" | "Scheduled" | "Downloading" | "Recording"
  | "PostProcessing" | "Paused" | "Finished" | "Failed" | "Cancelled";
export type Priority = "High" | "Normal" | "Low";
export type PostMode = "Default" | "None" | "Mp4" | "AudioOnly" | "SendToApp" | "Custom";
export type AudioFormat = "M4a" | "Mp3" | "Original";
export type Container = "PreferMp4" | "ForceMp4" | "Original";
export type SubMode = "None" | "IfAvailable" | "Required" | "Only";
export type SubOut = "Srt" | "Vtt" | "Embed";
export type DuplicatePolicy = "Ask" | "Redownload" | "Skip";
export type AfterQueue = "Nothing" | "Quit" | "Sleep";
export type UnsupportedLink = "Ignore" | "Guided";
export type ScriptWhen = "PerFile" | "EndOfQueue";
export type HistoryRetention = "Week" | "Month" | "ThreeMonths" | "SixMonths" | "Year" | "Never";
export type SortKey = "Order" | "Site" | "Playlist" | "Progress" | "Name";

export interface PlaylistRef { title: string; index: number; count: number }

export interface Progress {
  downloaded: number;
  total: number | null;
  percent: number;
  speed: number;
  eta_secs: number | null;
  stage: string;
}

export interface TaskOptions {
  post_mode: PostMode;
  dest_dir?: string | null;
  name_template?: string | null;
  max_resolution?: number | null;
  format_id?: string | null;
  audio_format?: AudioFormat | null;
  sub_mode?: SubMode | null;
  sub_langs?: string[] | null;
  duplicate?: DuplicatePolicy | null;
  speed_limit_kbps?: number | null;
  custom_script?: string | null;
  playlist?: PlaylistRef | null;
  title_hint?: string | null;
  referer?: string | null;
}

export interface SubtitleTrack { lang: string; name: string | null; url: string; ext: string; auto: boolean }
export interface FormatChoice { id: string; label: string; height: number | null; ext: string; size: number | null; audio_only: boolean }
export interface LinkCandidate {
  url: string; title: string | null; duration: number | null; thumbnail: string | null;
  size: number | null; kind: string | null; recognized: boolean;
}
export interface HistoryItem {
  id: number; url: string; media_id: string | null; title: string; site: string; path: string | null;
  size: number | null; thumbnail: string | null; finished_at: number; exists: boolean;
}

export type Question =
  | { Quality: { choices: FormatChoice[] } }
  | { Subtitles: { tracks: SubtitleTrack[] } }
  | { Links: { title: string; links: LinkCandidate[] } }
  | { Duplicate: { previous: HistoryItem } }
  | { Destination: { suggested: string } };

export type Answer =
  | { Quality: { format_id: string } }
  | { Subtitles: { langs: string[] } }
  | { Links: { urls: string[]; as_playlist: boolean } }
  | { Duplicate: { action: "Redownload" | "Skip" } }
  | { Destination: { dir: string } };

export interface Task {
  id: number; url: string; state: TaskState; priority: Priority; position: number; is_file?: boolean;
  title: string | null; site: string | null; thumbnail: string | null; playlist: string | null;
  options: TaskOptions; progress: Progress; output_path: string | null; target: string | null;
  error: string | null; created_at: number; scheduled_at: number | null; stop_at: number | null;
  retries: number; question: Question | null;
}

export interface Format {
  id: string; url: string; protocol: string; ext: string; vcodec: string | null; acodec: string | null;
  width: number | null; height: number | null; tbr: number | null; filesize: number | null;
  language: string | null; label: string | null; has_drm: boolean;
}
export interface MediaInfo {
  id: string; title: string; url: string; site: string; host: string; uploader: string | null;
  upload_date: string | null; timestamp: number | null; thumbnail: string | null; description: string | null;
  duration: number | null; is_live: boolean; formats: Format[]; subtitles: SubtitleTrack[];
  playlist: PlaylistRef | null; referer: string | null;
}
export type Resolved =
  | { Single: MediaInfo }
  | { Playlist: { title: string; entries: LinkCandidate[] } }
  | { Links: LinkCandidate[] }
  | { Unsupported: string };

export interface Settings {
  auto_start: boolean; max_concurrent: number; connections: number; speed_limit_kbps: number;
  automated_mode: boolean; after_queue: AfterQueue; retry_max: number; retry_base_secs: number;
  unsupported_link: UnsupportedLink; duplicate_policy: DuplicatePolicy;
  download_dir: string; name_template: string; ask_destination: boolean; min_free_mb: number;
  max_resolution: number | null; quality_prompt: boolean; container: Container;
  audio_format: AudioFormat | null; audio_languages: string[];
  post_mode: PostMode; reencode: boolean; keep_original: boolean; keep_source_on_convert: boolean; embed_metadata: boolean;
  embed_thumbnail: boolean; save_thumbnail: boolean; save_json: boolean; script: string | null;
  script_when: ScriptWhen; send_to_app: string | null; add_to_music: boolean;
  sub_mode: SubMode; sub_langs: string[]; sub_multiple: boolean; sub_auto: boolean;
  sub_label_regex: string | null; sub_out: SubOut;
  proxy: { url: string; username: string | null } | null; user_agent: string | null;
  timeout_secs: number; fragmented: boolean; xff: string | null;
  cookies_browser: string | null; cookies_file: string | null;
  live_mpegts: boolean; live_min_free_mb: number; history_retention: HistoryRetention;
  search_engines: string[]; tool_paths: Record<string, string>; notifications: boolean;
}

export interface AppPrefs {
  theme: "dark" | "light" | "system";
  density: "comfortable" | "compact";
  show_thumbnails: boolean; hide_dock: boolean; launch_at_login: boolean; menubar_icon: boolean; menubar_speed: boolean;
  sounds: boolean; sound_added: string; sound_finished: string; sound_failed: string;
  sound_queue_finished: string; focus_on_question: boolean; search_in_main: boolean;
}

export interface ToolStatus { name: string; path: string | null; version: string | null; source: string | null }
export interface UpdateInfo { current: string | null; latest: string; available: boolean }
export interface IntegrationInfo {
  name: string; url_regex: string; enabled: boolean; timeout_secs: number; requires_account: boolean;
  example_url: string | null; comment: string | null; code: string;
}
export interface IntegrationTest { logs: string[]; result: MediaInfo | null; error: string | null }
export interface SearchResult {
  title: string; url: string; engine: string; duration: number | null; thumbnail: string | null; uploader: string | null;
}
export interface SiteList { total: number; sites: string[] }
export interface AppInfo { version: string; data_dir: string; extension_dir: string; cookies_file: string }

export type GrabEvent =
  | { TaskAdded: Task }
  | { TaskUpdated: Task }
  | { TaskProgress: { id: number; progress: Progress } }
  | { TaskRemoved: { id: number } }
  | "QueueFinished"
  | { Log: { id: number | null; message: string } }
  | { Notification: { title: string; body: string } }
  | { AfterQueue: AfterQueue }
  | "ToolsChanged"
  | { GuidedExtraction: { id: number; url: string } };

export interface Detected {
  id: number; kind: "video" | "audio" | "hls" | "dash" | "image"; url: string; mime: string | null;
  page: string | null; width: number | null; height: number | null; size: number | null;
}
export interface Bookmark { url: string; title: string }
export interface BrowserHistoryEntry { url: string; title: string; at: number }
export interface BrowserState {
  media: Detected[]; page_url: string; page_title: string; muted: boolean; mode: string;
  bookmarks: Bookmark[]; history: BrowserHistoryEntry[]; has_page: boolean;
}
export interface LoginStatus { path: string; active: boolean; cookies: number; updated_at: number | null }
export interface SiteData { domain: string; cookies: number }
