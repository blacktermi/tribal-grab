//! Intégration macOS : icône de barre de menus, menus natifs, Dock, thème.
use crate::prefs::AppPrefs;
use crate::state::AppState;
use tauri_plugin_autostart::ManagerExt;
use tauri::menu::{AboutMetadataBuilder, Menu, MenuItem, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Theme};

pub const TRAY_ID: &str = "tribal-grab-tray";

/// Argument du lancement à l'ouverture de session : l'application démarre sans fenêtre.
pub const HIDDEN_ARG: &str = "--hidden";

/// L'agent de lancement contient le chemin de l'application : on le réécrit à chaque démarrage, au cas où elle a été déplacée.
fn apply_login_item(app: &AppHandle, enabled: bool) {
    // `tauri dev` remplacerait le chemin par celui du binaire de débogage.
    if cfg!(debug_assertions) {
        return;
    }
    let auto = app.autolaunch();
    let result = if enabled {
        auto.enable()
    } else if auto.is_enabled().unwrap_or(false) {
        auto.disable()
    } else {
        Ok(())
    };
    if let Err(e) = result {
        tracing::warn!("lancement à l'ouverture de session : {e}");
    }
}

pub fn apply_prefs(app: &AppHandle, p: &AppPrefs) {
    let _ = app.set_dock_visibility(!p.hide_dock);
    apply_login_item(app, p.launch_at_login);
    app.set_theme(match p.theme.as_str() {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None,
    });
    if let Some(t) = app.tray_by_id(TRAY_ID) {
        let _ = t.set_visible(p.menubar_icon);
    }
}

fn item(app: &AppHandle, id: &str, text: &str, accel: Option<&str>) -> tauri::Result<MenuItem<tauri::Wry>> {
    let b = MenuItemBuilder::with_id(id, text);
    match accel {
        Some(a) => b.accelerator(a).build(app),
        None => b.build(app),
    }
}

pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let about = AboutMetadataBuilder::new()
        .name(Some("Tribal Grab"))
        .version(Some(app.package_info().version.to_string()))
        .comments(Some("Téléchargeur de vidéos et d'audio. Aucune télémétrie."))
        .build();
    let app_menu = SubmenuBuilder::new(app, "Tribal Grab")
        .item(&PredefinedMenuItem::about(app, Some("À propos de Tribal Grab"), Some(about))?)
        .separator()
        .item(&item(app, "prefs", "Préférences…", Some("CmdOrCtrl+,"))?)
        .item(&item(app, "update_ytdlp", "Mettre à jour yt-dlp…", None)?)
        .separator()
        .item(&PredefinedMenuItem::services(app, Some("Services"))?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, Some("Masquer Tribal Grab"))?)
        .item(&PredefinedMenuItem::hide_others(app, Some("Masquer les autres"))?)
        .item(&PredefinedMenuItem::show_all(app, Some("Tout afficher"))?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some("Quitter Tribal Grab"))?)
        .build()?;
    let file = SubmenuBuilder::new(app, "Fichier")
        .item(&item(app, "add_link", "Ajouter un lien…", Some("CmdOrCtrl+N"))?)
        .item(&item(app, "paste_download", "Coller et télécharger", Some("CmdOrCtrl+Shift+V"))?)
        .item(&item(app, "import_file", "Importer des liens depuis un fichier…", Some("CmdOrCtrl+O"))?)
        .separator()
        .item(&item(app, "guided", "Extraction guidée", Some("CmdOrCtrl+B"))?)
        .item(&item(app, "login", "Connexion aux sites…", Some("CmdOrCtrl+Shift+L"))?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, Some("Fermer la fenêtre"))?)
        .build()?;
    let edit = SubmenuBuilder::new(app, "Édition")
        .item(&PredefinedMenuItem::undo(app, Some("Annuler"))?)
        .item(&PredefinedMenuItem::redo(app, Some("Rétablir"))?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, Some("Couper"))?)
        .item(&PredefinedMenuItem::copy(app, Some("Copier"))?)
        .item(&PredefinedMenuItem::paste(app, Some("Coller"))?)
        .item(&PredefinedMenuItem::select_all(app, Some("Tout sélectionner"))?)
        .build()?;
    let queue = SubmenuBuilder::new(app, "Téléchargements")
        .item(&item(app, "start_queue", "Démarrer la file", Some("CmdOrCtrl+Shift+S"))?)
        .item(&item(app, "pause_all", "Tout mettre en pause", Some("CmdOrCtrl+Shift+P"))?)
        .item(&item(app, "resume_all", "Tout reprendre", Some("CmdOrCtrl+Shift+R"))?)
        .separator()
        .item(&item(app, "clear_finished", "Retirer les terminés", Some("CmdOrCtrl+K"))?)
        .item(&item(app, "clear_inactive", "Retirer les inactifs", None)?)
        .separator()
        .item(&item(app, "cancel_all", "Tout annuler…", None)?)
        .item(&item(app, "reset_queue", "Réinitialiser la file…", None)?)
        .build()?;
    let window = SubmenuBuilder::new(app, "Fenêtre")
        .item(&PredefinedMenuItem::minimize(app, Some("Réduire"))?)
        .item(&item(app, "show_main", "File d'attente", Some("CmdOrCtrl+1"))?)
        .item(&item(app, "history", "Historique", Some("CmdOrCtrl+Y"))?)
        .item(&item(app, "search", "Recherche de vidéos", Some("CmdOrCtrl+F"))?)
        .item(&item(app, "sites", "Sites pris en charge", Some("CmdOrCtrl+Shift+Y"))?)
        .build()?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &queue, &window])
}

/// Actions de menu : certaines passent par l'interface (dialogues, confirmations).
pub fn on_menu(app: &AppHandle, id: &str) {
    let st = app.state::<AppState>();
    match id {
        "prefs" => crate::windows::show_prefs(app, None),
        "update_ytdlp" => crate::windows::show_prefs(app, Some("avance")),
        "show_main" => crate::windows::show_main(app),
        "history" => crate::windows::show_library(app, Some("history")),
        "search" => crate::windows::show_library(app, Some("search")),
        "sites" => crate::windows::show_library(app, Some("sites")),
        "guided" => {
            let _ = crate::guided::open_browser(app, None, "guided");
        }
        "login" => {
            let _ = crate::guided::open_browser(app, None, "login");
        }
        "start_queue" => st.engine.start_queue(),
        "pause_all" => st.engine.pause_all(),
        "resume_all" => st.engine.resume_all(),
        "clear_finished" => st.engine.clear_finished(),
        "clear_inactive" => st.engine.clear_inactive(),
        "add_link" | "paste_download" | "import_file" | "cancel_all" | "reset_queue" => {
            crate::windows::show_main(app);
            let _ = app.emit_to("main", "menu://action", id);
        }
        _ => {}
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &item(app, "show_main", "Ouvrir Tribal Grab", None)?,
            &item(app, "paste_download", "Coller et télécharger", None)?,
            &item(app, "guided", "Extraction guidée", None)?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, "prefs", "Préférences…", None)?,
            &PredefinedMenuItem::quit(app, Some("Quitter Tribal Grab"))?,
        ],
    )?;
    let icon = tauri::include_image!("icons/tray.png");
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Tribal Grab")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| on_menu(app, e.id().as_ref()))
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = ev {
                let app = tray.app_handle();
                let scale = app.get_webview_window("main").and_then(|w| w.scale_factor().ok()).unwrap_or(2.0);
                let pos = rect.position.to_physical::<f64>(scale);
                let size = rect.size.to_physical::<f64>(scale);
                crate::windows::toggle_popover(app, pos, (size.width, size.height));
            }
        })
        .build(app)?;
    Ok(())
}
