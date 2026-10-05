use serde_json::json;
use serde_json::Value;
use tauri::AppHandle;
use tauri::Manager;

use crate::contact_profile::{ContactProfile, ContactProfileStore};
use crate::error::{AppError, AppResult};

// Every `#[tauri::command]` below is a one-line `try_state` + delegate to an
// `_inner` fn that takes `Option<&ContactProfileStore>` instead of an
// `AppHandle` — so the degrade path (unmanaged store, the `try_state ==
// None` branch `panic = "abort"` made non-optional) is testable at all. This
// crate has no `tauri::test` mock-app harness (established precedent —
// `extension_bridge::assist_registry`, `commands::ai_provider::openai`,
// `salary_research` all document the same gap and use the same shape of
// split); building a REAL `ContactProfileStore`
// via `tempfile::TempDir` + `ContactProfileStore::open` and calling the
// `_inner` fn directly with `Some(&store)` / `None` exercises both branches
// of the actual production logic without one.

fn contact_profile_get_inner(store: Option<&ContactProfileStore>) -> Value {
    match store {
        Some(store) => json!(store.get()),
        None => json!(ContactProfile::default()),
    }
}

#[tauri::command]
pub async fn contact_profile_get(app: AppHandle) -> Value {
    // `try_state`, not `state` — `shell/state.rs` logs a failed `ContactProfileStore::open`
    // as "non-fatal" and leaves the store unmanaged in that case; `Manager::state`
    // panics on an unmanaged type, and `panic = "abort"` (Cargo.toml) turns that
    // into a hard process exit on what should degrade to an empty profile.
    contact_profile_get_inner(app.try_state::<ContactProfileStore>().as_deref())
}

// Returns `AppResult<Value>`, not a bare `Value` with an in-band `{"error":
// …}` shape — a Tauri command that returns `Result` REJECTS the invoke
// promise on `Err`, so `useSaveContactProfile`'s `onError` fires and the
// mutation is visibly failed. The bare-`Value` degrade shape this used to
// have was a real defect: its only caller (`useSaveContactProfile.mutationFn`)
// never inspected an `.error` field, so an unmanaged store or a storage
// failure looked exactly like success — a save silently and permanently
// lost. Trading the `panic = "abort"` crash `try_state` avoids for silent
// data loss was a worse trade; `Result` can't be ignored by a future caller
// the way an ad-hoc JSON shape could.
fn contact_profile_set_inner(
    store: Option<&ContactProfileStore>,
    profile: Value,
) -> AppResult<Value> {
    let Some(store) = store else {
        return Err(AppError::Storage(
            "contact profile store unavailable".to_string(),
        ));
    };
    let parsed: ContactProfile = serde_json::from_value(profile)
        .map_err(|e| AppError::Parse(format!("invalid contact profile: {e}")))?;
    store.set(&parsed)?;
    Ok(json!({ "success": true }))
}

#[tauri::command]
pub async fn contact_profile_set(app: AppHandle, profile: Value) -> AppResult<Value> {
    contact_profile_set_inner(app.try_state::<ContactProfileStore>().as_deref(), profile)
}

/// Clamp to a short ISO-639-1(-ish) tag length before it reaches
/// `LocalizedText::resolve`'s map-key comparisons. `lang` is not purely
/// renderer-chosen — the renderer derives it from `meta.targetLanguage`
/// (AI-detected, so ultimately shaped by the job ad text).
fn clamp_lang(lang: &str) -> String {
    lang.chars().take(16).collect()
}

fn contact_profile_header_line_inner(store: Option<&ContactProfileStore>, lang: &str) -> String {
    match store {
        Some(store) => store.get().header_markdown(&clamp_lang(lang)),
        None => String::new(),
    }
}

/// The stored profile's header contact line, localized for `lang` — the single
/// header builder (`ContactProfile::header_markdown`) shared by every render
/// backend, exposed so the renderer can seed it into generated text (H) without
/// re-implementing the ordering rules in TypeScript.
#[tauri::command]
pub async fn contact_profile_header_line(app: AppHandle, lang: String) -> String {
    // `try_state` — this now runs on every résumé generation (H's header
    // seeding), not just the settings page, so an unmanaged store here must
    // degrade to "nothing to seed," not abort the process.
    contact_profile_header_line_inner(app.try_state::<ContactProfileStore>().as_deref(), &lang)
}

#[cfg(test)]
mod tests;
