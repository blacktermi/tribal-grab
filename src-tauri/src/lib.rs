//! Tribal Grab : application de bureau (Tauri 2) au-dessus de `grab-core`.
mod commands;
mod events;
mod guided;
mod mac;
mod opener;
mod prefs;
mod sites;
mod state;
mod windows;

use grab_core::Engine;
use state::AppState;
use std::sync::Mutex;
use tauri::{Manager, RunEvent, WindowEvent};

fn data_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("TRIBAL_GRAB_DATA_DIR").unwrap_or_else(|_| {
        format!("{}/Library/Application Support/Tribal Grab", std::env::var("HOME").unwrap_or_else(|_| ".".into()))
    }))
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tribal_grab_lib=info".into()))
        .init();

    let app = tauri::Builder::default()
        // Une seule instance : un second lancement transmet ses arguments à la première.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            opener::handle_args(app, &argv);
            windows::show_main(app);
        }))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![mac::HIDDEN_ARG])))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::tasks, commands::task_add_url, commands::task_add_text, commands::task_add, commands::probe,
            commands::pause, commands::resume, commands::retry, commands::cancel, commands::remove,
            commands::pause_all, commands::resume_all, commands::cancel_all, commands::start_queue,
            commands::clear_finished, commands::clear_inactive, commands::reset_queue, commands::set_priority,
            commands::move_task, commands::set_schedule, commands::stop_recording, commands::set_task_speed_limit,
            commands::answer, commands::get_settings, commands::set_settings, commands::set_proxy_password,
            commands::get_app_prefs, commands::set_app_prefs, commands::history, commands::history_delete,
            commands::history_clear, commands::history_remove_missing, commands::history_redownload,
            commands::history_export_file, commands::history_import_file, commands::import_links_file,
            commands::reveal_in_finder, commands::open_path, commands::quicklook, commands::close_quicklook,
            commands::search, commands::sites, commands::integrations, commands::integration_read_file,
            commands::install_integration, commands::save_integration, commands::set_integration_enabled,
            commands::remove_integration, commands::export_integration_file, commands::test_integration,
            commands::tools, commands::set_tool_path, commands::check_ytdlp_update, commands::update_ytdlp,
            commands::bootstrap_status, commands::read_clipboard, commands::list_sounds, commands::play_sound,
            commands::app_info, commands::open_window, commands::extension_install, commands::quit_app,
            guided::guided_report, guided::browser_state, guided::guided_navigate, guided::guided_nav,
            guided::guided_mute, guided::guided_layout, guided::guided_page_visible, guided::guided_send,
            guided::guided_clear_media, guided::bookmark_toggle, guided::guided_history_clear,
            sites::login_status, sites::login_save, sites::login_disable, sites::site_data_list,
            sites::site_data_clear, sites::site_data_clear_all,
        ])
        .setup(|app| {
            let dir = data_dir();
            let engine = tauri::async_runtime::block_on(Engine::start(dir.clone()))?;
            let mut p = prefs::AppPrefs::load(&dir);
            p.sanitize();
            app.manage(AppState {
                engine: engine.clone(),
                data_dir: dir.clone(),
                prefs: Mutex::new(p.clone()),
                quicklook: Mutex::new(None),
                popover_hidden_at: Mutex::new(None),
            });
            app.manage(guided::Guided::new(&dir));
            let handle = app.handle().clone();
            app.set_menu(mac::build_menu(&handle)?)?;
            app.on_menu_event(|app, e| mac::on_menu(app, e.id().as_ref()));
            mac::build_tray(&handle)?;
            mac::apply_prefs(&handle, &p);
            events::spawn(handle.clone(), engine);
            let _ = tauri_plugin_notification::NotificationExt::notification(&handle).request_permission();
            let hidden = std::env::var("TRIBAL_GRAB_HIDDEN").is_ok() || std::env::args().any(|a| a == mac::HIDDEN_ARG);
            if !hidden {
                windows::show_main(&handle);
            } else {
                let _ = windows::create_main(&handle);
            }
            // Crochet de test, uniquement dans les builds de débogage : ouvre le navigateur guidé sur une adresse.
            #[cfg(debug_assertions)]
            if let Ok(u) = std::env::var("TRIBAL_GRAB_TEST_BROWSER") {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let _ = guided::open_browser(&h, Some(&u), "guided");
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Fermer la fenêtre principale la masque : les téléchargements continuent.
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("échec de la création de l'application");

    app.run(|app, event| match event {
        RunEvent::Opened { urls } => opener::handle_opened(app, &urls),
        RunEvent::Reopen { has_visible_windows, .. } => {
            if !has_visible_windows {
                windows::show_main(app);
            }
        }
        RunEvent::ExitRequested { api, code, .. } => {
            // Plus aucune fenêtre ouverte : l'application reste active (barre de menus).
            if code.is_none() {
                api.prevent_exit();
            }
        }
        _ => {}
    });
}
