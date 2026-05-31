use crate::credentials::CredentialStore;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn credentials_available(app: AppHandle) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    json!({ "available": guard.is_available() })
}

#[tauri::command]
pub fn credentials_set(
    app: AppHandle,
    board_id: String,
    username: String,
    password: String,
) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    match guard.set(&board_id, &username, &password) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn credentials_get(app: AppHandle, board_id: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    match guard.get_decrypted(&board_id) {
        Some((username, password)) => json!({ "username": username, "password": password }),
        None => json!(null),
    }
}

#[tauri::command]
pub fn credentials_remove(app: AppHandle, board_id: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    match guard.remove(&board_id) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn credentials_list(app: AppHandle) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    json!(guard.list())
}
