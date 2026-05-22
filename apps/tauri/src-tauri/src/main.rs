// Prevents a terminal window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::unnecessary_lazy_evaluations)]
#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::manual_next_back)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::regex_creation_in_loops)]
#![allow(clippy::get_first)]
#![allow(clippy::double_ended_iterator_last)]
#![allow(clippy::wrong_self_convention)]

mod autopilot;
mod applying;
mod browser;
mod commands;
mod conversations;
mod credentials;
mod documents;
mod export;
mod jobs;
mod postings;
mod scraping;
mod updater;

use std::sync::Mutex;

use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use autopilot::AutopilotStore;
use credentials::CredentialStore;
use jobs::JobTracker;
use postings::{InteractionStore, PostingsCache};
use scraping::ScraperEngine;
use updater::UpdaterState;

// ── App menu ──────────────────────────────────────────────────────────────────

fn build_app_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let quit = MenuItemBuilder::with_id("quit", "Quit")
        .accelerator("CmdOrControl+Q")
        .build(app)?;
    let hide = MenuItemBuilder::with_id("hide", "Hide Window")
        .accelerator("CmdOrControl+H")
        .build(app)?;
    let about = MenuItemBuilder::with_id("about", "About AI Job Hunter").build(app)?;

    let app_submenu = SubmenuBuilder::new(app, "AI Job Hunter")
        .item(&about)
        .separator()
        .item(&hide)
        .separator()
        .item(&quit)
        .build()?;

    MenuBuilder::new(app).item(&app_submenu).build()
}

fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        "quit" => app.exit(0),
        "hide" => {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.hide();
            }
        }
        _ => {}
    }
}

// ── System tray ───────────────────────────────────────────────────────────────

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("tray_show", "Show AI Job Hunter").build(app)?;
    let quit = MenuItemBuilder::with_id("tray_quit", "Quit").build(app)?;
    let tray_menu = MenuBuilder::new(app).item(&show).separator().item(&quit).build()?;

    TrayIconBuilder::new()
        .menu(&tray_menu)
        .tooltip("AI Job Hunter")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "tray_quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Left-click on the tray icon shows/focuses the main window.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle();

            // Data dir for all persistent state.
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".ajh")
            });
            // Export so scrapers (which don't have an AppHandle) can resolve
            // the same directory via `std::env::var("AJH_DATA_DIR")`.
            if std::env::var_os("AJH_DATA_DIR").is_none() {
                // SAFETY: only writer, called once before any worker spawns.
                unsafe { std::env::set_var("AJH_DATA_DIR", &data_dir); }
            }

            app.manage(Mutex::new(AutopilotStore::new(&data_dir)));
            app.manage(Mutex::new(CredentialStore::new(&data_dir)));
            match documents::DocumentStore::open(&data_dir) {
                Ok(store) => { app.manage(store); }
                Err(e) => eprintln!("[setup] document store failed to open (non-fatal): {e}"),
            }
            app.manage(Mutex::new(JobTracker::default()));
            app.manage(Mutex::new(PostingsCache::default()));
            app.manage(Mutex::new(InteractionStore::new(&data_dir)));
            app.manage(Mutex::new(UpdaterState::default()));
            app.manage(std::sync::Arc::new(ScraperEngine::new()));
            if let Ok(db) = conversations::ConversationDb::open(handle) {
                app.manage(db);
            } else {
                eprintln!("[setup] conversation db failed to open (non-fatal)");
            }

            // Build and set the application menu.
            let menu = build_app_menu(handle)?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| on_menu_event(app, event.id().as_ref()));

            // Build system tray.
            if let Err(e) = build_tray(handle) {
                eprintln!("[setup] tray build error (non-fatal): {e}");
            }

            // Schedule background update checks (10 s after launch, then every 4 h).
            updater::setup_auto_check(handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // system
            commands::system_health,
            commands::system_get_version,
            commands::system_get_locale,
            commands::system_set_locale,
            commands::system_get_platform,
            commands::system_open_external,
            commands::system_set_performance_mode,
            commands::system_get_metrics,
            // jobs
            commands::jobs_list,
            commands::jobs_get,
            commands::jobs_cancel,
            commands::jobs_retry,
            // ai
            commands::ai_generate,
            commands::ai_list_models,
            commands::ai_pull_model,
            commands::ai_unload_model,
            commands::ai_embed,
            commands::ai_set_provider_key,
            commands::ai_remove_provider_key,
            commands::ai_has_provider_key,
            commands::ai_list_provider_models,
            // documents
            commands::documents_list,
            commands::documents_import,
            commands::documents_remove,
            // search
            commands::search_hybrid,
            // scrape
            commands::scrape_board,
            commands::scrape_url,
            commands::scrape_persist_job,
            commands::scrape_list_postings,
            commands::scrape_clear_postings,
            commands::scrape_list_interactions,
            commands::scrape_export_data,
            commands::scrape_import_data,
            // match
            commands::match_resume,
            // credentials
            commands::credentials_available,
            commands::credentials_list,
            commands::credentials_set,
            commands::credentials_remove,
            // linkedin / boards
            commands::linkedin_connect,
            commands::linkedin_disconnect,
            commands::linkedin_get_status,
            commands::boards_connect,
            commands::boards_disconnect,
            commands::boards_get_status,
            // privacy
            commands::privacy_sign_out_all,
            commands::privacy_clear_interactions,
            // apply
            commands::apply_start,
            commands::apply_catalog,
            // resume
            commands::resume_extract_text,
            // support
            commands::support_export_diagnostics,
            commands::support_reload_ai_runtime,
            commands::support_unload_all_models,
            commands::support_reset_model_configuration,
            commands::support_rebuild_vector_indexes,
            commands::support_clear_embeddings_cache,
            commands::support_reset_vector_database,
            commands::support_clear_ocr_cache,
            commands::support_reindex_all_documents,
            commands::support_reset_all_sessions,
            commands::support_clear_scraping_queue,
            commands::support_copy_environment_details,
            commands::support_copy_app_version,
            commands::support_copy_system_info,
            // conversations
            commands::conversations_get_or_create,
            commands::conversations_load_messages,
            commands::conversations_save_message,
            // native dialogs
            commands::dialog_open_files,
            commands::geocode_suggest,
            // autopilot
            commands::autopilot_list,
            commands::autopilot_get,
            commands::autopilot_create,
            commands::autopilot_update,
            commands::autopilot_remove,
            commands::autopilot_run,
            commands::autopilot_pause,
            commands::autopilot_resume,
            // export
            export::commands::documents_export_document,
            export::commands::documents_export_and_save,
            // updater
            updater::updater_check,
            updater::updater_download,
            updater::updater_install,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
