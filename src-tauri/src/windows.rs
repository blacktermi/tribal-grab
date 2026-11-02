//! Création et affichage des fenêtres locales.
use crate::state::AppState;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, TitleBarStyle, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

fn bring(w: &WebviewWindow) {
    let _ = w.show();
    let _ = w.unminimize();
    let _ = w.set_focus();
}

pub fn create_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Tribal Grab")
        .inner_size(1060.0, 700.0)
        .min_inner_size(780.0, 480.0)
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .visible(false)
        .build()
}

pub fn show_main(app: &AppHandle) {
    match app.get_webview_window("main") {
        Some(w) => bring(&w),
        None => {
            if let Ok(w) = create_main(app) {
                bring(&w);
            }
        }
    }
}

fn show_titled(app: &AppHandle, label: &str, title: &str, size: (f64, f64), min: (f64, f64), section: Option<&str>) {
    if let Some(w) = app.get_webview_window(label) {
        bring(&w);
        if let Some(s) = section {
            let _ = app.emit_to(label, "window://section", s);
        }
        return;
    }
    let url = match section {
        Some(s) => format!("index.html?section={s}"),
        None => "index.html".to_string(),
    };
    if let Ok(w) = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(size.0, size.1)
        .min_inner_size(min.0, min.1)
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()
    {
        bring(&w);
    }
}

pub fn show_prefs(app: &AppHandle, section: Option<&str>) {
    show_titled(app, "prefs", "Préférences", (900.0, 640.0), (760.0, 480.0), section);
}

pub fn show_library(app: &AppHandle, tab: Option<&str>) {
    show_titled(app, "library", "Bibliothèque", (1020.0, 700.0), (760.0, 480.0), tab);
}

const POPOVER_W: f64 = 340.0;
const POPOVER_H: f64 = 440.0;

pub fn ensure_popover(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window("tray") {
        return Some(w);
    }
    let w = WebviewWindowBuilder::new(app, "tray", WebviewUrl::App("index.html".into()))
        .title("Tribal Grab")
        .inner_size(POPOVER_W, POPOVER_H)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .shadow(true)
        .build()
        .ok()?;
    let app2 = app.clone();
    let w2 = w.clone();
    w.on_window_event(move |e| {
        if let tauri::WindowEvent::Focused(false) = e {
            let _ = w2.hide();
            *app2.state::<AppState>().popover_hidden_at.lock().unwrap() = Some(Instant::now());
        }
    });
    Some(w)
}

/// Affiche ou masque le popover sous l'icône de la barre de menus.
pub fn toggle_popover(app: &AppHandle, icon_pos: PhysicalPosition<f64>, icon_size: (f64, f64)) {
    let Some(w) = ensure_popover(app) else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    // Un clic sur l'icône retire d'abord le focus du popover, qui se masque : ne pas le rouvrir aussitôt.
    if let Some(t) = *app.state::<AppState>().popover_hidden_at.lock().unwrap()
        && t.elapsed() < Duration::from_millis(250)
    {
        return;
    }
    let scale = w.scale_factor().unwrap_or(2.0);
    let _ = w.set_size(LogicalSize::new(POPOVER_W, POPOVER_H));
    let width = POPOVER_W * scale;
    let x = (icon_pos.x + icon_size.0 / 2.0 - width / 2.0).max(8.0 * scale);
    let y = icon_pos.y + icon_size.1 + 6.0 * scale;
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit_to("tray", "popover://shown", ());
}
