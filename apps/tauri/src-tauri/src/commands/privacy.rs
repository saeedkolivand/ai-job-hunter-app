use parking_lot::Mutex;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ai_generations::AiGenerationStore;
use crate::autopilot::AutopilotStore;
use crate::contact_profile::ContactProfileStore;
use crate::conversations::ConversationDb;
use crate::credentials::CredentialStore;
use crate::documents::DocumentStore;
use crate::job_preferences::JobPreferencesStore;
use crate::jobs::JobTracker;
use crate::pipeline::cache::KvCache;
use crate::postings::{InteractionStore, PostingsCache};

#[tauri::command]
pub fn privacy_clear_data(app: AppHandle) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }
    app.state::<Mutex<PostingsCache>>().lock().clear_all();
    app.state::<Mutex<InteractionStore>>().lock().clear_all();
    json!({ "success": true })
}

#[tauri::command]
pub fn privacy_clear_interactions(app: AppHandle) -> Value {
    app.state::<Mutex<InteractionStore>>().lock().clear_all();
    json!({ "success": true })
}

/// Sign out of all connected job boards (sessions only, data is preserved).
#[tauri::command]
pub fn privacy_sign_out_all(app: AppHandle) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }
    json!({ "success": true })
}

/// Full factory reset: sign out all boards and wipe every persistent store.
/// The frontend is responsible for resetting persisted preferences (localStorage).
#[tauri::command]
pub fn privacy_reset_app(app: AppHandle) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Sign out all board sessions
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }

    // Wipe EVERY persistent user-data store. Keep this list in lockstep with
    // `commands/data.rs::build_bundle` (the backup set) plus the stores that are
    // intentionally excluded from backups (secrets, caches, the job log) — adding
    // a new persistent store means handling it in BOTH places.
    app.state::<Mutex<PostingsCache>>().lock().clear_all();
    app.state::<Mutex<InteractionStore>>().lock().clear_all();
    app.state::<Mutex<JobTracker>>().lock().clear();
    app.state::<DocumentStore>().clear_all();
    app.state::<AiGenerationStore>().clear_all();
    app.state::<ConversationDb>().clear_all();
    if let Some(s) = app.try_state::<std::sync::Arc<Mutex<AutopilotStore>>>() {
        s.lock().clear_all();
    }
    if let Some(s) = app.try_state::<JobPreferencesStore>() {
        let _ = s.clear();
    }
    if let Some(s) = app.try_state::<ContactProfileStore>() {
        let _ = s.clear();
    }
    if let Some(s) = app.try_state::<KvCache>() {
        s.clear();
    }

    // Remove every stored secret — all AI/provider keys (incl. Brave) and board
    // passwords — driven off the credential metadata index (no hardcoded list).
    let _ = app.state::<Mutex<CredentialStore>>().lock().clear_all();

    json!({ "success": true })
}
