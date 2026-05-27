use parking_lot::Mutex;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ai_generations::AiGenerationStore;
use crate::conversations::ConversationDb;
use crate::credentials::CredentialStore;
use crate::documents::DocumentStore;
use crate::postings::{InteractionStore, PostingsCache};

#[tauri::command]
pub fn privacy_clear_data(app: AppHandle) -> Value {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }
    app.state::<Mutex<PostingsCache>>().lock().clear_all();
    app.state::<Mutex<InteractionStore>>().lock().clear_all();
    json!({ "success": true })
}

#[tauri::command]
pub fn privacy_clear_interactions(app: AppHandle) -> Value {
    app.state::<Mutex<InteractionStore>>()
        .lock()
        .clear_all();
    json!({ "success": true })
}

/// Sign out of all connected job boards (sessions only, data is preserved).
#[tauri::command]
pub fn privacy_sign_out_all(app: AppHandle) -> Value {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }
    json!({ "success": true })
}

/// Full factory reset: sign out all boards and wipe every persistent store.
/// The frontend is responsible for resetting persisted preferences (localStorage).
#[tauri::command]
pub fn privacy_reset_app(app: AppHandle) -> Value {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Sign out all board sessions
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }

    // Clear in-memory + JSON-backed stores
    app.state::<Mutex<PostingsCache>>().lock().clear_all();
    app.state::<Mutex<InteractionStore>>().lock().clear_all();

    // Clear SQLite databases
    app.state::<DocumentStore>().clear_all();
    app.state::<AiGenerationStore>().clear_all();
    app.state::<ConversationDb>().clear_all();

    // Clear all AI provider API keys from keychain
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    for provider in &["openai", "anthropic", "gemini", "openai-compatible"] {
        let _ = guard.remove(&format!("ai:{provider}"));
    }

    json!({ "success": true })
}
