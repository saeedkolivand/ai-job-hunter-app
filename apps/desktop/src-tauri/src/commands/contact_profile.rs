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
// `agent::controller`, `extension_bridge::assist_registry`,
// `commands::ai_provider::openai`, `salary_research` all document the same
// gap and use the same shape of split); building a REAL `ContactProfileStore`
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
    // `try_state`, not `state` — `lib.rs` logs a failed `ContactProfileStore::open`
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
fn contact_profile_set_inner(store: Option<&ContactProfileStore>, profile: Value) -> AppResult<Value> {
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
mod test {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn clamp_lang_passes_a_short_code_through_unchanged() {
        assert_eq!(clamp_lang("de"), "de");
        assert_eq!(clamp_lang("en-US"), "en-US");
    }

    #[test]
    fn clamp_lang_truncates_an_oversized_value() {
        let huge = "x".repeat(1000);
        assert_eq!(clamp_lang(&huge).len(), 16);
    }

    /// A real, isolated store (temp SQLite file) — not a mock — so the
    /// managed-branch tests below exercise the actual `ContactProfileStore`
    /// read/write path, not a stand-in. `TempDir` must outlive the store (it
    /// deletes on drop), hence returning both.
    fn store() -> (TempDir, ContactProfileStore) {
        let dir = TempDir::new().expect("tempdir");
        let store = ContactProfileStore::open(&dir.path().to_path_buf()).expect("open store");
        (dir, store)
    }

    // ── contact_profile_get_inner ───────────────────────────────────────────

    #[test]
    fn get_inner_degrades_to_default_profile_when_store_unmanaged() {
        assert_eq!(contact_profile_get_inner(None), json!(ContactProfile::default()));
    }

    #[test]
    fn get_inner_returns_the_stored_profile_when_managed() {
        let (_dir, store) = store();
        store
            .set(&ContactProfile {
                full_name: Some("Jane Doe".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(contact_profile_get_inner(Some(&store))["fullName"], "Jane Doe");
    }

    // ── contact_profile_set_inner ───────────────────────────────────────────

    /// The HIGH-1 repro: an unmanaged store must REJECT (not silently
    /// "succeed" with an in-band error field nobody reads).
    #[test]
    fn set_inner_rejects_when_store_unmanaged() {
        let err = contact_profile_set_inner(None, json!({ "fullName": "Jane Doe" })).unwrap_err();
        assert!(matches!(err, AppError::Storage(_)), "{err:?}");
    }

    #[test]
    fn set_inner_persists_and_reports_success_when_managed() {
        let (_dir, store) = store();
        let result = contact_profile_set_inner(Some(&store), json!({ "fullName": "Jane Doe" }));
        assert_eq!(result.unwrap(), json!({ "success": true }));
        assert_eq!(store.get().full_name.as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn set_inner_rejects_an_invalid_payload_even_when_managed() {
        let (_dir, store) = store();
        let err = contact_profile_set_inner(Some(&store), json!("not a contact profile object"))
            .unwrap_err();
        assert!(matches!(err, AppError::Parse(_)), "{err:?}");
    }

    // ── contact_profile_header_line_inner ───────────────────────────────────

    #[test]
    fn header_line_inner_degrades_to_empty_string_when_store_unmanaged() {
        assert_eq!(contact_profile_header_line_inner(None, "en"), "");
    }

    #[test]
    fn header_line_inner_returns_the_localized_header_when_managed() {
        let (_dir, store) = store();
        store
            .set(&ContactProfile {
                email: Some("jane@example.com".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            contact_profile_header_line_inner(Some(&store), "en"),
            "jane@example.com"
        );
    }
}
