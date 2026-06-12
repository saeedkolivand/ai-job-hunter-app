use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

#[tauri::command]
pub fn boards_get_config(board_id: String) -> Value {
    match crate::scraping::board_login::get_config(&board_id) {
        Some(config) => json!({
            "id": config.id,
            "displayName": config.display_name,
            "loginUrl": config.login_url,
            "hasAuthUrlPredicate": config.is_authed_url.is_some(),
            "hasAuthCookiePredicate": config.is_authed_cookies.is_some(),
        }),
        None => json!(null),
    }
}

#[tauri::command]
pub fn boards_list_configs() -> Value {
    let board_ids = ["linkedin", "indeed", "xing", "glassdoor"];
    let configs: Vec<Value> = board_ids
        .iter()
        .filter_map(|id| crate::scraping::board_login::get_config(id))
        .map(|c| {
            json!({
                "id": c.id,
                "displayName": c.display_name,
                "loginUrl": c.login_url,
            })
        })
        .collect();
    json!(configs)
}

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
        let _ = app_clone.emit(
            "boards:login-status",
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

#[tauri::command]
pub fn boards_test_auth_url(url: String) -> Value {
    // Uses default_is_authed_url to test if a URL pattern indicates logged-in state
    json!({ "isAuthed": crate::scraping::board_login::default_is_authed_url(&url) })
}

#[tauri::command]
pub fn boards_get_login_config() -> Value {
    // Expose login timeout configuration
    json!({
        "loginTimeoutSecs": crate::scraping::board_login::LOGIN_TIMEOUT.as_secs(),
        "pollIntervalMs": crate::scraping::board_login::POLL_INTERVAL.as_millis(),
    })
}

#[tauri::command]
pub fn boards_get_disable_passkey_script() -> Value {
    // Expose the script used to disable passkey during login
    json!({ "script": crate::scraping::board_login::DISABLE_PASSKEY_SCRIPT })
}

#[tauri::command]
pub fn boards_list_browser_helpers() -> Value {
    // Browser automation helpers are now integrated into boards_login_with_browser
    json!({
        "status": "integrated",
        "command": "boards_login_with_browser",
        "note": "Browser automation functions (open_login, wait_for_auth, read_cookies, export_cookies) are used internally"
    })
}

#[tauri::command]
pub fn boards_list(app: AppHandle) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut boards = Vec::new();
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        boards.push(json!({
            "id": board_id,
            "displayName": board_id.to_uppercase(),
            "connected": crate::scraping::board_login::get_status(&data_dir, board_id),
        }));
    }
    json!(boards)
}
