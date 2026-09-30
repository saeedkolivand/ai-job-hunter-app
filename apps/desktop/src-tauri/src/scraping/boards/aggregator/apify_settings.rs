//! Non-secret aggregator settings (plugin-store JSON) backing the Apify
//! LinkedIn opt-in toggle — split out of `providers.rs` (R8 module-size guard).

// ── Non-secret aggregator settings (plugin-store JSON) ──────────────────────────
//
// The "Include LinkedIn (Apify)" opt-in toggle and the optional actor-id override
// are NOT secrets, so they do not belong in the OS keychain. The renderer persists
// them with `@tauri-apps/plugin-store` to `<app_data_dir>/scraping-settings.json`;
// plugin-store resolves a relative store path against the app data dir — the SAME
// directory `platform::config::data_dir()` resolves for AppHandle-less workers, so
// the provider can read them here without an `AppHandle` (mirrors how API keys are
// read AppHandle-free via `credentials::read_credential`).
//
// The file name + key strings are the cross-language contract in
// `packages/shared/src/scraping-settings.ts`; the literals below are pinned to it
// by `aggregator_settings_keys_match_shared_contract` in `test.rs`.
pub(super) const SCRAPING_SETTINGS_FILE: &str = "scraping-settings.json";
pub(super) const SETTING_APIFY_ENABLED: &str = "apifyLinkedinEnabled";
pub(super) const SETTING_APIFY_ACTOR_ID: &str = "apifyLinkedinActorId";

#[derive(Debug, Default, Clone)]
pub(super) struct AggregatorSettings {
    /// Master opt-in for the paid Apify LinkedIn provider. Default `false`.
    pub(super) apify_linkedin_enabled: bool,
    /// Optional actor-id override; `None` → the built-in default actor.
    pub(super) apify_linkedin_actor_id: Option<String>,
}

/// Read the non-secret aggregator settings from the plugin-store JSON file.
///
/// Absent file, parse failure, or missing keys all degrade to defaults (toggle
/// OFF) — never an error. A missing/garbled settings file must never crash a
/// user-triggered search; it simply means the opt-in provider stays disabled.
pub(super) fn read_aggregator_settings() -> AggregatorSettings {
    let path = crate::platform::config::data_dir().join(SCRAPING_SETTINGS_FILE);
    let json: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::Value::Null);

    let apify_linkedin_enabled = json
        .get(SETTING_APIFY_ENABLED)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let apify_linkedin_actor_id = json
        .get(SETTING_APIFY_ACTOR_ID)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    AggregatorSettings {
        apify_linkedin_enabled,
        apify_linkedin_actor_id,
    }
}
