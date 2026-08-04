use serde_json::json;
use serde_json::Value;
use tauri::AppHandle;
use tauri::Manager;

use crate::contact_profile::{ContactProfile, ContactProfileStore};

#[tauri::command]
pub async fn contact_profile_get(app: AppHandle) -> Value {
    let store = app.state::<ContactProfileStore>();
    json!(store.get())
}

#[tauri::command]
pub async fn contact_profile_set(app: AppHandle, profile: Value) -> Value {
    let store = app.state::<ContactProfileStore>();
    let parsed: ContactProfile = match serde_json::from_value(profile) {
        Ok(p) => p,
        Err(e) => return json!({ "error": format!("invalid contact profile: {e}") }),
    };
    match store.set(&parsed) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Clamp to a short ISO-639-1(-ish) tag length before it reaches
/// `LocalizedText::resolve`'s map-key comparisons. `lang` is not purely
/// renderer-chosen — the renderer derives it from `meta.targetLanguage`
/// (AI-detected, so ultimately shaped by the job ad text).
fn clamp_lang(lang: &str) -> String {
    lang.chars().take(16).collect()
}

/// The stored profile's header contact line, localized for `lang` — the single
/// header builder (`ContactProfile::header_markdown`) shared by every render
/// backend, exposed so the renderer can seed it into generated text (H) without
/// re-implementing the ordering rules in TypeScript.
#[tauri::command]
pub async fn contact_profile_header_line(app: AppHandle, lang: String) -> String {
    let store = app.state::<ContactProfileStore>();
    store.get().header_markdown(&clamp_lang(&lang))
}

#[cfg(test)]
mod test {
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
}
