//! Web-search backend settings for company research: the SearXNG-compatible server URL
//! (SearXNG, RustXNG). Stored beside the provider config (`ai_config::search_settings`), not
//! in the keychain: a URL is not a secret. The Exa key keeps using the provider-key commands.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

fn current(store: &crate::ai_config::AiConfigStore) -> Value {
    json!({ "searxngUrl": store.searxng_url() })
}

/// The stored search-backend settings: `{ searxngUrl: string | null }`.
#[tauri::command]
pub fn ai_search_settings(app: AppHandle) -> Value {
    current(&app.state::<crate::ai_config::AiConfigStore>())
}

/// Set (or clear, with `null`/blank) the SearXNG-compatible server URL. Validated server-side:
/// http/https with a host, no query or fragment, cloud-metadata host refused. Returns the fresh
/// settings, or `{ error }`.
#[tauri::command]
pub fn ai_set_search_settings(app: AppHandle, searxng_url: Option<String>) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    match store.set_searxng_url(searxng_url.as_deref()) {
        Ok(()) => current(&store),
        Err(e) => json!({ "error": e.to_string() }),
    }
}
