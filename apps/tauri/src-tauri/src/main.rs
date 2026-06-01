// Prevents a terminal window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Async-safety: never hold a lock guard across an `.await` (the app uses
// `parking_lot::Mutex` inside async command handlers). See docs/architecture-rules.md R14.
#![deny(clippy::await_holding_lock)]

mod ai_generations;
mod apply_helpers;
mod applying;
mod autopilot;
mod autopilot_helpers;
mod autopilot_scheduler;
mod browser;
mod commands;
mod contact_profile;
mod conversations;
mod cover_letter;
mod credentials;
mod data_store;
mod db;
mod documents;
mod error;
mod export;
mod extraction;
mod ipc_contracts;
mod job_preferences;
mod jobs;
mod layout;
mod locale;
mod measure;
mod model;
mod net;
mod observability;
mod pipeline;
mod platform;
mod postings;
mod profile_import;
mod recommend;
mod scraping;
mod theme;
mod updater;
mod validate;

use parking_lot::Mutex;

use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
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

    let edit_submenu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .build()?;

    MenuBuilder::new(app)
        .item(&app_submenu)
        .item(&edit_submenu)
        .build()
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
    let tray_menu = MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&quit)
        .build()?;

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
    credentials::init_keyring();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .max_file_size(5_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .level(log::LevelFilter::Warn)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle();

            // Data dir for all persistent state. Resolved + exported once here so
            // AppHandle-less workers (scrapers/appliers) reach the same path.
            // All path/env knowledge lives in `platform::config`.
            let data_dir = platform::config::resolve_and_export_data_dir(handle);

            // Persistent user-data stores are managed via `manage_resettable`, which
            // also registers each one into the `ResetRegistry` so `privacy_reset_app`
            // wipes it on factory reset — no hand-maintained clear list.
            use commands::privacy::manage_resettable;
            let mut reset_registry = commands::privacy::ResetRegistry::default();

            manage_resettable(
                app,
                &mut reset_registry,
                "autopilots",
                std::sync::Arc::new(Mutex::new(AutopilotStore::new(&data_dir))),
            );
            manage_resettable(
                app,
                &mut reset_registry,
                "credentials",
                Mutex::new(CredentialStore::new(&data_dir)),
            );
            match documents::DocumentStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "documents", store),
                Err(e) => log::warn!("[setup] document store failed to open (non-fatal): {e}"),
            }
            match ai_generations::AiGenerationStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "ai_generations", store),
                Err(e) => {
                    log::warn!("[setup] ai generations store failed to open (non-fatal): {e}")
                }
            }
            match job_preferences::JobPreferencesStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "job_preferences", store),
                Err(e) => {
                    log::warn!("[setup] job preferences store failed to open (non-fatal): {e}")
                }
            }
            match contact_profile::ContactProfileStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "contact_profile", store),
                Err(e) => {
                    log::warn!("[setup] contact profile store failed to open (non-fatal): {e}")
                }
            }
            manage_resettable(
                app,
                &mut reset_registry,
                "job_tracker",
                Mutex::new(JobTracker::open(&data_dir)),
            );
            manage_resettable(
                app,
                &mut reset_registry,
                "postings",
                Mutex::new(PostingsCache::default()),
            );
            manage_resettable(
                app,
                &mut reset_registry,
                "interactions",
                Mutex::new(InteractionStore::new(&data_dir)),
            );
            app.manage(Mutex::new(UpdaterState::default()));
            app.manage(std::sync::Arc::new(ScraperEngine::new()));
            if let Ok(db) = conversations::ConversationDb::open(handle) {
                manage_resettable(app, &mut reset_registry, "conversations", db);
            } else {
                log::warn!("[setup] conversation db failed to open (non-fatal)");
            }
            match pipeline::cache::KvCache::open(&data_dir) {
                Ok(cache) => manage_resettable(app, &mut reset_registry, "cache", cache),
                Err(e) => log::warn!("[setup] pipeline cache failed to open (non-fatal): {e}"),
            }

            app.manage(reset_registry);

            // Build and set the application menu.
            let menu = build_app_menu(handle)?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| on_menu_event(app, event.id().as_ref()));

            // Platform-specific window decorations
            #[cfg(target_os = "windows")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_decorations(false)?;
                }
            }
            #[cfg(target_os = "macos")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_decorations(true)?;
                }
            }

            // Build system tray.
            if let Err(e) = build_tray(handle) {
                log::warn!("[setup] tray build error (non-fatal): {e}");
            }

            // Schedule background update checks (10 s after launch, then every 4 h).
            updater::setup_auto_check(handle);

            // Start autopilot schedule runner (checks every minute).
            autopilot_scheduler::start(handle.clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // system
            commands::system::system_health,
            commands::system::system_get_version,
            commands::system::system_get_locale,
            commands::system::system_set_locale,
            commands::system::system_get_platform,
            commands::system::system_open_external,
            commands::system::system_set_performance_mode,
            commands::system::system_get_metrics,
            commands::system::system_check_browser,
            commands::system::system_open_devtools,
            commands::system::system_get_protocol_version,
            // jobs
            commands::jobs::jobs_list,
            commands::jobs::jobs_get,
            commands::jobs::jobs_cancel,
            commands::jobs::jobs_retry,
            // ai
            commands::ai::ai_generate,
            commands::ai::ai_list_models,
            commands::ai::ai_inspect_model,
            commands::ai::ai_research_company,
            commands::ai::ai_pull_model,
            commands::ai::ai_unload_model,
            commands::ai::ai_embed,
            commands::ai::ai_set_provider_key,
            commands::ai::ai_remove_provider_key,
            commands::ai::ai_has_provider_key,
            commands::ai::ai_test_provider_key,
            commands::ai::ai_list_provider_models,
            commands::ai::ai_embedding_status,
            commands::ai::ai_set_embedding_config,
            commands::ai::ai_reembed_all,
            commands::pipeline::generate_pipeline,
            // resume extraction
            commands::resume::extract_resume,
            // documents
            commands::documents::documents_list,
            commands::documents::documents_import,
            commands::documents::documents_recommend_template,
            commands::documents::documents_remove,
            commands::documents::documents_set_default,
            commands::documents::documents_embed_text,
            commands::documents::documents_set_indexed,
            commands::documents::documents_upsert_vector,
            commands::documents::documents_get_vector,
            commands::documents::documents_all_vectors,
            commands::documents::documents_cosine_similarity,
            commands::documents::documents_strip_extension,
            // job preferences
            commands::job_preferences::job_preferences_get,
            commands::job_preferences::job_preferences_set,
            // contact profile (header source of truth)
            commands::contact_profile::contact_profile_get,
            commands::contact_profile::contact_profile_set,
            // search
            commands::search::search_hybrid,
            // scrape
            commands::scrape::scrape_board,
            commands::scrape::scrape_url,
            commands::scrape::scrape_resolve_url,
            commands::scrape::scrape_persist_job,
            commands::scrape::scrape_list_postings,
            commands::scrape::scrape_clear_postings,
            commands::scrape::scrape_list_interactions,
            // data backup / restore
            commands::data::data_export,
            commands::data::data_import,
            // match
            commands::match_resume::match_resume,
            commands::match_resume::resume_extract_text,
            // credentials
            commands::credentials::credentials_available,
            commands::credentials::credentials_set,
            commands::credentials::credentials_get,
            commands::credentials::credentials_remove,
            commands::credentials::credentials_list,
            // boards
            commands::boards::boards_get_config,
            commands::boards::boards_list_configs,
            commands::boards::boards_test_auth_url,
            commands::boards::boards_get_login_config,
            commands::boards::boards_get_disable_passkey_script,
            commands::boards::boards_list_browser_helpers,
            commands::boards::boards_login_with_browser,
            commands::boards::boards_logout,
            commands::boards::boards_get_status,
            commands::boards::boards_list,
            // privacy
            commands::privacy::privacy_clear_data,
            commands::privacy::privacy_clear_interactions,
            commands::privacy::privacy_sign_out_all,
            commands::privacy::privacy_reset_app,
            // apply
            commands::apply::apply_start,
            commands::apply::apply_catalog,
            // support
            commands::support::support_export_logs,
            commands::support::support_get_system_info,
            // conversations
            commands::conversations::conversations_get_or_create,
            commands::conversations::conversations_load_messages,
            commands::conversations::conversations_save_message,
            // dialog
            commands::dialog::dialog_open_files,
            // geocoding
            commands::geocoding::geocode_suggest,
            // autopilot
            commands::autopilot::autopilot_list,
            commands::autopilot::autopilot_get,
            commands::autopilot::autopilot_create,
            commands::autopilot::autopilot_update,
            commands::autopilot::autopilot_remove,
            commands::autopilot::autopilot_run,
            commands::autopilot::autopilot_pause,
            commands::autopilot::autopilot_resume,
            // ai generations
            commands::ai_generations::ai_generations_list,
            commands::ai_generations::ai_generations_save,
            commands::ai_generations::ai_generations_remove,
            // cover letter pipeline
            commands::cover_letter::generate_cover_letter,
            // profile import
            commands::profile_import::profile_import_from_url,
            // export
            export::commands::documents_export_document,
            export::commands::documents_export_and_save,
            // updater
            updater::updater_check,
            updater::updater_download,
            updater::updater_install,
            updater::updater_changelog,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
