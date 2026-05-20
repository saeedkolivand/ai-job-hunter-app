// Prevents a terminal window from appearing on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod sidecar;

use std::sync::Mutex;
use sidecar::ScraperSidecarState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(ScraperSidecarState::default()))
        .setup(|app| {
            // Attempt to start the scraper sidecar. Failure is non-fatal so
            // the app opens even when the binary is absent during development.
            let handle = app.handle();
            if let Err(e) = sidecar::try_start(handle) {
                eprintln!("[setup] sidecar start error (non-fatal): {e}");
            }
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
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
