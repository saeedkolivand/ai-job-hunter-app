use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::events::{emit_event, BOARDS_LOGIN_STATUS};

#[tauri::command]
pub async fn boards_login_with_browser(app: AppHandle, board_id: String) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    let app_clone = app.clone();
    let board_id_clone = board_id.clone();

    // Status callback that emits Tauri events
    let on_status = move |status: &str| {
        emit_event(
            &app_clone,
            BOARDS_LOGIN_STATUS,
            serde_json::json!({
                "boardId": board_id_clone,
                "status": status
            }),
        );
    };

    match crate::scraping::board_login::open_login(&data_dir, &board_id, on_status).await {
        Ok(success) => json!({ "connected": success }),
        Err(e) => json!({ "connected": false, "error": e.to_string() }),
    }
}

#[tauri::command]
pub fn boards_import_cookies(app: AppHandle, board_id: String) -> Value {
    use crate::scraping::board_login::ImportOutcome;

    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    match crate::scraping::board_login::import_cookies(&data_dir, &board_id) {
        Ok(outcome) => {
            let (label, imported) = match outcome {
                ImportOutcome::Imported(n) => ("Imported", n),
                ImportOutcome::NoSession => ("NoSession", 0),
                ImportOutcome::Undecryptable => ("Undecryptable", 0),
                ImportOutcome::BrowserNotFound => ("BrowserNotFound", 0),
            };
            json!({ "outcome": label, "imported": imported })
        }
        Err(e) => json!({ "outcome": "Error", "imported": 0, "error": e.to_string() }),
    }
}

#[tauri::command]
pub fn boards_logout(app: AppHandle, board_id: String) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    crate::scraping::board_login::disconnect(&data_dir, &board_id);
    json!({ "success": true })
}

#[tauri::command]
pub fn boards_get_status(app: AppHandle, board_id: String) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    json!({ "connected": crate::scraping::board_login::get_status(&data_dir, &board_id) })
}

/// Board catalog driving the manual jobs picker — the registry is the single
/// source of truth for each board's auth tier and `listed` flag. Serialized
/// entries match `BoardCatalogEntry` in the shared IPC contract.
#[tauri::command]
pub fn boards_catalog(app: AppHandle) -> Value {
    let engine = app.state::<std::sync::Arc<crate::scraping::ScraperEngine>>();
    json!(engine.catalog())
}

/// Boards that support in-app login/connect (auth-capable). Add a board id here
/// when its login flow is wired; the scrapeable-board catalog is separate (registry-derived).
const LOGIN_BOARDS: &[&str] = &["linkedin"];

/// In-app login / connect status for auth-capable boards.
/// Returns each board's `connected` state via `board_login::get_status` — NOT the
/// scrapeable-board catalog, which is registry-derived (`boards::all()` / `boards_catalog`).
#[tauri::command]
pub fn boards_list(app: AppHandle) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut boards = Vec::new();
    for board_id in LOGIN_BOARDS {
        boards.push(json!({
            "id": board_id,
            "displayName": board_id.to_uppercase(),
            "connected": crate::scraping::board_login::get_status(&data_dir, board_id),
        }));
    }
    json!(boards)
}
