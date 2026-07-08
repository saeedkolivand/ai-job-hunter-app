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
    // Return a BARE boolean to match the TS contract (`available(): Promise<boolean>`)
    // and its mock — the consumer gates on `=== false`, which never fires against an
    // object. Wrapping in `{ available: … }` here silently broke that seam.
    json!(guard.is_available())
}
