//! The backend-owned active generation provider and the per-stage model overrides.
//! Split out of `commands/ai/mod.rs` for R8 (issue #1280); `mod.rs` re-exports the
//! commands, so each keeps its `commands::ai::<name>` path.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

// ── Active generation provider (backend-owned; task #16) ─────────────────────────

/// Read the active generation provider config: the active provider's resolved
/// `model`/`baseUrl` (what `useGenerateConfig` reads) plus the `providers` map
/// for the Settings AI tab. Unseeded → `activeProvider` absent (generation errors
/// "No AI provider selected", never a silent fallback). Values are returned as the
/// writer validated them; the generation egress (`Completer::from_active`)
/// defensively re-validates the base_url before use.
#[tauri::command]
pub fn ai_active_config(app: AppHandle) -> Value {
    serde_json::to_value(
        app.state::<crate::ai_config::AiConfigStore>()
            .active_config(),
    )
    .unwrap_or_else(|_| json!({ "providers": {} }))
}

/// Switch the active provider (the "switch" half of the switch-vs-edit split —
/// deliberately separate from `ai_set_provider_settings` so editing a provider's
/// settings can never silently flip which provider is active). Validates the id
/// server-side. Returns the fresh active config, or `{ error }`.
#[tauri::command]
pub fn ai_set_active_provider(app: AppHandle, provider: String) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    match store.set_active_provider(&provider) {
        Ok(()) => serde_json::to_value(store.active_config()).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Edit a provider's model/base_url/context_window (the "edit" half — never
/// flips the active provider). Server-side validation: known id, cross-family
/// model check, base_url provenance (scheme + cloud-metadata block;
/// loopback/LAN gateways stay allowed), and the context-window range. Returns
/// the fresh active config, or `{ error }`.
///
/// PATCH semantics per field — absent keeps the stored value, explicit `null`
/// clears it, a value sets it. So a caller may send ONLY what changed, and
/// omitting a field can never erase it. See
/// [`ProviderSettingsPatch`](crate::ai_config::ProviderSettingsPatch) for why
/// the request is a struct rather than loose arguments.
#[tauri::command]
pub fn ai_set_provider_settings(
    app: AppHandle,
    req: crate::ai_config::ProviderSettingsPatch,
) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    match store.set_provider_settings(req) {
        Ok(()) => serde_json::to_value(store.active_config()).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// One-time first-run seed from the renderer's migrated Zustand `aiProviderConfig`
/// (`{ activeProvider, providers: { [id]: { model, baseUrl } } }`). Row-presence
/// gated SERVER-side: a no-op once anything has been set, so it can never clobber a
/// later explicit change (the renderer also gates on `persist.hasHydrated()`). Bad
/// values are scrubbed, never rejected. Returns `{ seeded: bool }` or `{ error }`.
#[tauri::command]
pub fn ai_seed_active_config(app: AppHandle, config: crate::ai_config::AiConfigSnapshot) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    match store.seed_if_empty(&config) {
        Ok(seeded) => json!({ "seeded": seeded }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ── Per-stage model overrides ────────────────────────────────────────────────

/// Read every explicitly-set per-stage model override, keyed by stage name.
///
/// ABSENT means "this stage runs on the active provider" — the read model has
/// no entry for an unconfigured stage, and the UI must render that as the
/// default rather than as an override equal to the default. Rows naming a stage
/// the current build no longer runs are filtered out server-side, so every key
/// returned is a live stage.
#[tauri::command]
pub fn ai_stage_overrides(app: AppHandle) -> Value {
    serde_json::to_value(
        app.state::<crate::ai_config::AiConfigStore>()
            .stage_overrides(),
    )
    .unwrap_or_else(|_| json!({}))
}

/// Point ONE pipeline stage at a specific provider + model.
///
/// Strict server-side validation — unknown stage, unknown provider,
/// cross-family model, out-of-range context window are all `{ error }`, never
/// a silently scrubbed row: an override the user cannot see the effect of is
/// worse than a refused one. Returns the fresh override map so the caller
/// re-renders from the server's answer.
///
/// Takes NO base URL, deliberately. The endpoint for the named provider is the
/// one already stored in that provider's own settings row, which Settings
/// displays; accepting a per-stage one here would let a caller plant an egress
/// endpoint that no screen shows. See [`crate::ai_config::StageOverride`].
#[tauri::command]
pub fn ai_set_stage_override(
    app: AppHandle,
    stage: String,
    provider: String,
    model: Option<String>,
    context_window: Option<u32>,
) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    let over = crate::ai_config::StageOverride {
        provider,
        model: model.unwrap_or_default(),
        context_window,
    };
    match store.set_stage_override(&stage, over) {
        Ok(()) => serde_json::to_value(store.stage_overrides()).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Return ONE stage to the active provider. A no-op (not an error) for a stage
/// that has no override, so the UI can clear without reading first.
#[tauri::command]
pub fn ai_clear_stage_override(app: AppHandle, stage: String) -> Value {
    let store = app.state::<crate::ai_config::AiConfigStore>();
    match store.clear_stage_override(&stage) {
        Ok(()) => serde_json::to_value(store.stage_overrides()).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    }
}
