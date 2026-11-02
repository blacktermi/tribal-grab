//! Relais des événements du moteur vers les fenêtres, et réactions de l'application
//! (sons, notifications, fin de file, extraction guidée, icône de barre de menus).
use crate::mac::TRAY_ID;
use crate::prefs;
use crate::state::AppState;
use grab_core::{AfterQueue, Engine, Event, TaskState};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

fn human_speed(bps: u64) -> String {
    let v = bps as f64;
    if v >= 1_048_576.0 { format!("{:.1} Mo/s", v / 1_048_576.0).replace('.', ",") } else { format!("{:.0} Ko/s", v / 1024.0) }
}

pub fn spawn(app: AppHandle, engine: Engine) {
    let mut rx = engine.subscribe();
    tauri::async_runtime::spawn(async move {
        let mut states: HashMap<i64, TaskState> = HashMap::new();
        let mut speeds: HashMap<i64, u64> = HashMap::new();
        let mut last_tray = Instant::now() - Duration::from_secs(5);
        loop {
            let ev = match rx.recv().await {
                Ok(e) => e,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let _ = app.emit("grab://lagged", ());
                    continue;
                }
                Err(_) => break,
            };
            let _ = app.emit("grab://event", &ev);
            let st = app.state::<AppState>();
            let prefs = st.prefs.lock().unwrap().clone();
            let play = |name: &str| {
                if prefs.sounds {
                    prefs::play_sound(name)
                }
            };
            match &ev {
                Event::TaskAdded(t) => {
                    states.insert(t.id, t.state);
                    play(&prefs.sound_added);
                }
                Event::TaskUpdated(t) => {
                    let prev = states.insert(t.id, t.state);
                    if prev != Some(t.state) {
                        match t.state {
                            TaskState::Finished => play(&prefs.sound_finished),
                            TaskState::Failed => play(&prefs.sound_failed),
                            TaskState::NeedsInput if prefs.focus_on_question => crate::windows::show_main(&app),
                            _ => {}
                        }
                        if !matches!(t.state, TaskState::Downloading | TaskState::Recording) {
                            speeds.remove(&t.id);
                        }
                    }
                }
                Event::TaskRemoved { id } => {
                    states.remove(id);
                    speeds.remove(id);
                }
                Event::TaskProgress { id, progress } => {
                    speeds.insert(*id, progress.speed);
                }
                Event::QueueFinished => {
                    speeds.clear();
                    play(&prefs.sound_queue_finished);
                }
                Event::Notification { title, body } => {
                    if engine.settings().notifications {
                        let _ = app.notification().builder().title(title).body(body).show();
                    }
                }
                Event::AfterQueue(AfterQueue::Quit) => {
                    app.exit(0);
                }
                Event::GuidedExtraction { url, .. } => {
                    let _ = crate::guided::open_browser(&app, Some(url), "guided");
                }
                _ => {}
            }
            if last_tray.elapsed() >= Duration::from_millis(900) || matches!(ev, Event::QueueFinished) {
                last_tray = Instant::now();
                let active = states.values().filter(|s| s.is_active()).count();
                let total: u64 = speeds.values().sum();
                if let Some(t) = app.tray_by_id(TRAY_ID) {
                    let title = (active > 0 && prefs.menubar_speed).then(|| format!(" {active} · {}", human_speed(total)));
                    let _ = t.set_title(title.as_deref());
                    let _ = t.set_tooltip(Some(if active > 0 { format!("Tribal Grab : {active} en cours") } else { "Tribal Grab".to_string() }));
                }
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_badge_count(if active > 0 { Some(active as i64) } else { None });
                }
            }
        }
    });
}
