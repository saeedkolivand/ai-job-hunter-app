use crate::credentials::CredentialStore;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

// Board-login credential CRUD (set/get/remove/list) was removed with the
// credential-based account UI — boards now authenticate via browser sessions
// (`boards.connect` / `linkedin.connect`), and no scraper reads stored
// passwords. `CredentialStore` itself stays: AI provider keys (`ai:*`) and the
// factory reset still use it. Only the keyring-availability probe is exposed.
#[tauri::command]
pub fn credentials_available(app: AppHandle) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    json!({ "available": guard.is_available() })
}
