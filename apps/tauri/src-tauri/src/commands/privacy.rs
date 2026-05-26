use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use crate::postings::{InteractionStore, PostingsCache};

#[tauri::command]
pub fn privacy_clear_data(app: AppHandle) -> Value {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }
    app.state::<Mutex<PostingsCache>>().lock().unwrap().clear_all();
    app.state::<Mutex<InteractionStore>>().lock().unwrap().clear_all();
    json!({ "success": true })
}

#[tauri::command]
pub fn privacy_clear_interactions(app: AppHandle) -> Value {
    app.state::<Mutex<InteractionStore>>()
        .lock()
        .unwrap()
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

/// Full factory reset: sign out all boards, clear all cached postings and interactions.
/// The frontend is responsible for resetting persisted preferences (localStorage).
#[tauri::command]
pub fn privacy_reset_app(app: AppHandle) -> Value {
    privacy_clear_data(app)
}
