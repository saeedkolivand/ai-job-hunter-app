//! Provider credentials and catalogue — the stored API keys, the key probe, the
//! provider model list and the capability probe. Split out of `commands/ai/mod.rs`
//! for R8 (issue #1280); `mod.rs` re-exports the commands, so each keeps its
//! `commands::ai::<name>` path.

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::commands::ai_provider::resolve_by_name;
use crate::credentials::CredentialStore;
use crate::error::AppResult;

pub(crate) fn get_provider_key(app: &AppHandle, provider: &str) -> Option<String> {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    guard
        .get_decrypted(&format!("ai:{provider}"))
        .map(|(_, password)| password)
}

#[tauri::command]
pub fn ai_set_provider_key(app: AppHandle, provider: String, api_key: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    match guard.set(&format!("ai:{provider}"), "apikey", &api_key) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn ai_remove_provider_key(app: AppHandle, provider: String) -> Value {
    let store = app.state::<Mutex<CredentialStore>>();
    let guard = store.lock();
    match guard.remove(&format!("ai:{provider}")) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub fn ai_has_provider_key(app: AppHandle, provider: String) -> Value {
    json!({ "has": get_provider_key(&app, &provider).is_some() })
}

#[tauri::command]
pub async fn ai_test_provider_key(
    app: AppHandle,
    provider: String,
    base_url: Option<String>,
) -> Value {
    // The provider resolves its own credentials/transport (keychain key + client,
    // or a CLI binary check) — this command just dispatches.
    let provider_client = match resolve_by_name(&provider, base_url) {
        Ok(p) => p,
        Err(e) => return json!({ "success": false, "error": e }),
    };
    match provider_client.test_key(&app).await {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[tauri::command]
pub async fn ai_list_provider_models(
    app: AppHandle,
    provider: String,
    base_url: Option<String>,
) -> AppResult<Value> {
    let provider_client = resolve_by_name(&provider, base_url)?;
    Ok(json!(provider_client.list_models(&app).await?))
}

/// Capability probe for a provider/model. Network-free, but NOT side-effect
/// free: `supportsWebSearch` reads the OS keychain to see whether a search
/// backend is actually configured — whether it can
/// attempt a web-grounded `research*` search, whether it accepts a
/// reasoning-effort value, and (when it does) exactly which levels this
/// model accepts (drives the Settings → AI effort picker). Reads the
/// resolved [`ModelCapabilities`] matrix + [`AiProvider::effort_levels`] (the
/// SAME values consumed server-side by `ai_research_*` and by each adapter's
/// own effort-field gate), so the renderer never mirrors the per-provider
/// vocabulary or booleans: a NEW provider/model is exposed with zero
/// TypeScript change — this is a per-MODEL lookup (Gemini's accepted level
/// subset genuinely varies by model tier, not just by provider). An
/// unknown/unresolvable provider degrades to `supportsWebSearch: false`,
/// `supportsReasoning: false`, `effortLevels: []`, matching the caller's safe
/// default-off fallback.
#[tauri::command]
pub fn ai_model_capabilities(
    app: AppHandle,
    provider: String,
    model: Option<String>,
    base_url: Option<String>,
) -> Value {
    let model = model.unwrap_or_default();
    match resolve_by_name(&provider, base_url) {
        Ok(client) => {
            let caps = client.capabilities(&model);
            json!({
                // Whether research can actually RUN, not what the provider
                // advertises — see `search::research_available`. This is why the
                // command takes `app`, and why it reads stored credentials.
                "supportsWebSearch": crate::commands::ai_provider::search::research_available(
                    &app,
                    client.as_ref(),
                    &model,
                ),
                "supportsReasoning": caps.supports_reasoning,
                "effortLevels": client.effort_levels(&model),
            })
        }
        Err(_) => json!({
            "supportsWebSearch": false,
            "supportsReasoning": false,
            "effortLevels": Vec::<&str>::new(),
        }),
    }
}
