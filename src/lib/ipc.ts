import { invoke } from "@tauri-apps/api/core";
import type * as T from "@/types";

/** Toutes les commandes de l'application. Les erreurs du moteur arrivent en chaînes françaises. */
export const ipc = {
  tasks: (sort: T.SortKey = "Order", desc = false) => invoke<T.Task[]>("tasks", { sort, desc }),
  addUrl: (url: string) => invoke<number>("task_add_url", { url }),
  addText: (text: string) => invoke<number[]>("task_add_text", { text }),
  add: (req: { url: string; options?: Partial<T.TaskOptions>; priority?: T.Priority; scheduled_at?: number | null; stop_at?: number | null }) =>
    invoke<number>("task_add", { req: { options: { post_mode: "Default" }, priority: "Normal", scheduled_at: null, stop_at: null, ...req } }),
  probe: (url: string) => invoke<T.Resolved>("probe", { url }),
  pause: (id: number) => invoke<void>("pause", { id }),
  resume: (id: number) => invoke<void>("resume", { id }),
  retry: (id: number) => invoke<void>("retry", { id }),
  cancel: (id: number) => invoke<void>("cancel", { id }),
  remove: (id: number) => invoke<void>("remove", { id }),
  pauseAll: () => invoke<void>("pause_all"),
  resumeAll: () => invoke<void>("resume_all"),
  cancelAll: () => invoke<void>("cancel_all"),
  startQueue: () => invoke<void>("start_queue"),
  clearFinished: () => invoke<void>("clear_finished"),
  clearInactive: () => invoke<void>("clear_inactive"),
  resetQueue: () => invoke<void>("reset_queue"),
  setPriority: (id: number, priority: T.Priority) => invoke<void>("set_priority", { id, priority }),
  moveTask: (id: number, index: number) => invoke<void>("move_task", { id, index }),
  setSchedule: (id: number, scheduledAt: number | null, stopAt: number | null) =>
    invoke<void>("set_schedule", { id, scheduledAt, stopAt }),
  stopRecording: (id: number) => invoke<void>("stop_recording", { id }),
  setTaskSpeedLimit: (id: number, kbps: number) => invoke<void>("set_task_speed_limit", { id, kbps }),
  answer: (id: number, answer: T.Answer) => invoke<void>("answer", { id, answer }),

  getSettings: () => invoke<T.Settings>("get_settings"),
  setSettings: (settings: T.Settings) => invoke<T.Settings>("set_settings", { settings }),
  setProxyPassword: (password: string) => invoke<void>("set_proxy_password", { password }),
  getAppPrefs: () => invoke<T.AppPrefs>("get_app_prefs"),
  setAppPrefs: (prefs: T.AppPrefs) => invoke<T.AppPrefs>("set_app_prefs", { prefs }),

  history: (query: string, sort: string, desc: boolean, limit = 2000) => invoke<T.HistoryItem[]>("history", { query, sort, desc, limit }),
  historyDelete: (ids: number[]) => invoke<void>("history_delete", { ids }),
  historyClear: () => invoke<void>("history_clear"),
  historyRemoveMissing: () => invoke<number>("history_remove_missing"),
  historyRedownload: (id: number) => invoke<number>("history_redownload", { id }),
  historyExportFile: (format: "json" | "csv" | "xml", path: string) => invoke<void>("history_export_file", { format, path }),
  historyImportFile: (format: "json" | "csv" | "xml", path: string) => invoke<number>("history_import_file", { format, path }),
  importLinksFile: (path: string) => invoke<number[]>("import_links_file", { path }),

  revealInFinder: (path: string) => invoke<void>("reveal_in_finder", { path }),
  openPath: (path: string) => invoke<void>("open_path", { path }),
  quicklook: (path: string) => invoke<void>("quicklook", { path }),
  closeQuicklook: () => invoke<void>("close_quicklook"),

  search: (query: string) => invoke<T.SearchResult[]>("search", { query }),
  sites: (filter: string) => invoke<T.SiteList>("sites", { filter }),

  integrations: () => invoke<T.IntegrationInfo[]>("integrations"),
  integrationReadFile: (path: string) => invoke<{ json: string; info: T.IntegrationInfo; path: string }>("integration_read_file", { path }),
  installIntegration: (json: string) => invoke<T.IntegrationInfo>("install_integration", { json }),
  saveIntegration: (info: T.IntegrationInfo) => invoke<T.IntegrationInfo>("save_integration", { info }),
  setIntegrationEnabled: (name: string, enabled: boolean) => invoke<void>("set_integration_enabled", { name, enabled }),
  removeIntegration: (name: string) => invoke<void>("remove_integration", { name }),
  exportIntegrationFile: (name: string, path: string) => invoke<void>("export_integration_file", { name, path }),
  testIntegration: (info: T.IntegrationInfo, url: string) => invoke<T.IntegrationTest>("test_integration", { info, url }),

  tools: () => invoke<T.ToolStatus[]>("tools"),
  setToolPath: (name: string, path: string | null) => invoke<void>("set_tool_path", { name, path }),
  checkYtdlpUpdate: () => invoke<T.UpdateInfo>("check_ytdlp_update"),
  updateYtdlp: () => invoke<string>("update_ytdlp"),
  bootstrapStatus: () => invoke<{ needs_ytdlp: boolean }>("bootstrap_status"),

  readClipboard: () => invoke<string>("read_clipboard"),
  listSounds: () => invoke<string[]>("list_sounds"),
  playSound: (name: string) => invoke<void>("play_sound", { name }),
  appInfo: () => invoke<T.AppInfo>("app_info"),
  openWindow: (name: "main" | "prefs" | "library" | "browser", section?: string, url?: string) =>
    invoke<void>("open_window", { name, section: section ?? null, url: url ?? null }),
  extensionInstall: () => invoke<string>("extension_install"),
  quitApp: () => invoke<void>("quit_app"),

  browserState: () => invoke<T.BrowserState>("browser_state"),
  guidedNavigate: (input: string) => invoke<string>("guided_navigate", { input }),
  guidedNav: (action: "back" | "forward" | "reload" | "stop") => invoke<void>("guided_nav", { action }),
  guidedMute: (muted: boolean) => invoke<void>("guided_mute", { muted }),
  guidedLayout: (x: number, y: number, w: number, h: number) => invoke<void>("guided_layout", { x, y, w, h }),
  guidedPageVisible: (visible: boolean) => invoke<void>("guided_page_visible", { visible }),
  guidedSend: (urls: string[], referer: string | null) => invoke<number[]>("guided_send", { urls, referer }),
  guidedClearMedia: () => invoke<void>("guided_clear_media"),
  bookmarkToggle: (url: string, title: string) => invoke<boolean>("bookmark_toggle", { url, title }),
  guidedHistoryClear: () => invoke<void>("guided_history_clear"),

  loginStatus: () => invoke<T.LoginStatus>("login_status"),
  loginSave: () => invoke<number>("login_save"),
  loginDisable: () => invoke<void>("login_disable"),
  siteDataList: () => invoke<T.SiteData[]>("site_data_list"),
  siteDataClear: (domain: string, kinds: ("cookies" | "local" | "session")[]) => invoke<void>("site_data_clear", { domain, kinds }),
  siteDataClearAll: () => invoke<void>("site_data_clear_all"),
};

export function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return "Erreur inconnue";
}
