# API de `grab-core` (pour `src-tauri`)

Crate : `crates/grab-core`, sans dépendance à Tauri. Tous les types de `grab_core::model` sont `Serialize + Deserialize`
(`Error` est `Serialize` : il se sérialise en chaîne française, utilisable directement comme erreur de commande Tauri).
Tout est asynchrone sous tokio ; `Engine` est `Clone` (un `Arc` à l'intérieur), à ranger dans l'état Tauri.

## Démarrage et événements

```rust
let engine = Engine::start(data_dir).await?;      // ~/Library/Application Support/Tribal Grab
let mut rx = engine.subscribe();                  // tokio::sync::broadcast::Receiver<Event>
```
`data_dir` contient `grab.sqlite`, `integrations/*.tgi` et `bin/` (outils téléchargés, prioritaires sur Homebrew).
À `start`, la file est restaurée : une tâche interrompue (état actif) repasse en `Queued` et reprend ses parties.

`Event` : `TaskAdded(Task)`, `TaskUpdated(Task)` (changement d'état), `TaskProgress{id, progress}` (toutes les 500 ms),
`TaskRemoved{id}`, `QueueFinished`, `Log{id, message}` (déjà expurgé de toute URL signée), `Notification{title, body}`,
`AfterQueue(AfterQueue)` (réglage « à la fin de la file » : l'app quitte si `Quit` ; le Mac est déjà mis en veille par le
moteur si `Sleep`), `ToolsChanged`, `GuidedExtraction{id, url}` (lien non pris en charge + réglage « extraction guidée » :
ouvrir le navigateur intégré). En cas de retard du récepteur (`Lagged`), relire `tasks()`.

## Ajout et sondage

- `add(AddRequest{url, options: TaskOptions, priority, scheduled_at, stop_at}) -> i64` (id). Seules http et https sont acceptées.
- `add_url(&str)`, `add_text(&str) -> Vec<i64>` (extrait toutes les adresses d'un texte collé).
- `probe(url) -> Resolved` : `Single(MediaInfo)`, `Playlist{title, entries}`, `Links(Vec<LinkCandidate>)`, `Unsupported(msg)`
  (sans créer de tâche ; sert aux « détails d'un lien »).
- `TaskOptions` : `post_mode`, `dest_dir`, `name_template`, `max_resolution`, `format_id`, `audio_format`, `sub_mode`, `sub_langs`,
  `duplicate`, `speed_limit_kbps`, `custom_script`, `referer`. Les champs vides prennent la valeur des réglages.
- Ordre de résolution : intégration `.tgi`, fichier direct, manifeste `.m3u8`/`.mpd`, yt-dlp, détection HTML.

## File d'attente

`tasks(SortKey, desc) -> Vec<Task>`, `task(id)`, `pause/resume/retry/cancel/remove(id)`, `pause_all()`, `resume_all()`,
`cancel_all()`, `start_queue()` (démarrage manuel si `auto_start` est faux), `clear_finished()`, `clear_inactive()`,
`reset_queue()`, `set_priority(id, Priority)`, `move_task(id, index)`, `set_schedule(id, scheduled_at, stop_at)`,
`stop_recording(id)` (garde ce qui est enregistré), `set_task_speed_limit(id, kbps)`.

`Task` : `id, url, state, priority, position, title, site, thumbnail, playlist, options, progress, output_path, error, created_at,
scheduled_at, stop_at, retries, question`. `TaskState` : `Queued, Extracting, NeedsInput, Scheduled, Downloading, Recording,
PostProcessing, Paused, Finished, Failed, Cancelled`. `Progress{downloaded, total, percent, speed (o/s), eta_secs, stage}`.
Les nouvelles tentatives passent par `Scheduled` avec `scheduled_at` et `error` renseignés.

## Questions (`NeedsInput`)

L'état `NeedsInput` porte `task.question`. Répondre avec `answer(id, Answer)` :

| `Question` | `Answer` |
|---|---|
| `Quality{choices: Vec<FormatChoice>}` | `Quality{format_id}` (un `id` de `choices`) |
| `Subtitles{tracks}` | `Subtitles{langs}` |
| `Links{title, links}` (playlist, page à plusieurs médias) | `Links{urls, as_playlist}` ; la tâche d'origine disparaît (`TaskRemoved`) et une tâche est créée par lien |
| `Duplicate{previous: HistoryItem}` | `Duplicate{action: Redownload \| Skip}` |
| `Destination{suggested}` | `Destination{dir}` |

Le filtre de liens (tout, rien, inverser, N premiers…) se fait côté interface sur `links`. Avec `automated_mode`, aucune question n'est posée.

## Réglages

`settings() -> Settings`, `set_settings(Settings)` (valide, enregistre, reconstruit le client réseau).
`set_proxy_password(&str)` écrit dans le trousseau macOS (jamais dans SQLite). Le défaut est la meilleure qualité
(`max_resolution: None`), sans plafond ni liste bloquée. `Settings` couvre : général, fichiers, qualité, post-traitement,
sous-titres, réseau, direct, historique, moteurs de recherche, `tool_paths`, notifications.

## Historique, recherche, sites

- `history(query, sort ("date"|"title"|"site"|"size"), desc, limit) -> Vec<HistoryItem>` (`exists` = le fichier est toujours là),
  `history_delete(&[id])`, `history_clear()`, `history_remove_missing()`, `history_redownload(id) -> task id`.
- `history_export(history_io::Format::{Json,Csv,Xml}) -> String`, `history_import(format, &str) -> usize`.
- `reveal_in_finder(path)`, `open_path(path)`.
- `search(query) -> Vec<SearchResult>` (moteurs activés dans `settings.search_engines` : youtube, soundcloud, google, yahoo, niconico, bilibili).
- `sites(filter) -> SiteList{total, sites}` (depuis `yt-dlp --list-extractors`, mis en cache).

## Intégrations `.tgi`

`integrations() -> Vec<IntegrationInfo>`, `install_integration(json)`, `save_integration(info)`, `set_integration_enabled(name, bool)`,
`remove_integration(name)`, `export_integration(name) -> String`, `test_integration(info, url) -> IntegrationTest{logs, result, error}`
(console de l'éditeur). Format : JSON `{name, url_regex, code, timeout_secs, requires_account, example_url, comment, enabled}`.
Le code JavaScript (QuickJS, 32 Mo, délai) n'a ni disque ni processus : `url` (globale), `grab.fetch(url) -> string`, `grab.log(msg)`,
`grab.result({title, formats:[{url, ext, height, protocol:"hls"|"http"}], subtitles:[{lang, url, ext}], thumbnail, id})`.

## Outils

`tools() -> Vec<ToolStatus>` (yt-dlp, ffmpeg, ffprobe, deno, node : chemin, version, provenance), `set_tool_path(name, Option<path>)`,
`check_ytdlp_update() -> UpdateInfo`, `update_ytdlp() -> version` (publication GitHub officielle, SHA256 vérifié, installé dans `bin/yt-dlp-app/`).
`update_ytdlp` dure environ 30 s, dont 18 s d'analyse macOS payée une seule fois. Il doit être lancé au premier démarrage : le yt-dlp
trouvé sur la machine peut être trop ancien, et YouTube répond alors 403.

## Fichiers produits et reprise

Destination : `{dossier}/{modèle}.{ext}` ; partiel : `{destination}.tgpart/` avec `manifest.json` (aucune URL ni cookie) et les parties.
Le fichier final est déplacé atomiquement. Ouvrir un `.tgpart` : relancer `add` sur la même URL ne suffit pas, utiliser `resume(id)` sur la tâche
(la destination est mémorisée dans `task.target`). DRM : refus format par format (Widevine, FairPlay, PlayReady) ; le reste est téléchargé.

## Ligne de commande

`crates/grab-cli` fournit `tribal-grab get|text|queue|probe|search|sites|history|tools|update-ytdlp` (voir `tribal-grab help`).
