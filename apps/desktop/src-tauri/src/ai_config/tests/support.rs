//! Fixtures shared by the `ai_config` store tests.

use super::*;

pub(in crate::ai_config) fn new_store() -> (TempDir, AiConfigStore) {
    let dir = TempDir::new().unwrap();
    let store = AiConfigStore::open(&dir.path().to_path_buf()).expect("open store");
    (dir, store)
}

/// A patch that names EVERY field explicitly — the shape the old
/// replace-semantics writer had, so the tests below keep their meaning. Tests
/// that care about ABSENCE build the patch themselves.
pub(super) fn full_patch(
    provider: &str,
    model: Option<&str>,
    base_url: Option<&str>,
    context_window: Option<u32>,
) -> ProviderSettingsPatch {
    ProviderSettingsPatch {
        provider: provider.to_string(),
        model: Some(model.map(str::to_string)),
        base_url: Some(base_url.map(str::to_string)),
        context_window: Some(context_window),
    }
}

pub(super) fn provider_cfg(model: Option<&str>, base_url: Option<&str>) -> ProviderConfig {
    ProviderConfig {
        model: model.map(str::to_string),
        base_url: base_url.map(str::to_string),
        context_window: None,
    }
}

/// The stored base URL of `provider`'s row, read through the same model the
/// renderer gets.
pub(super) fn base_url_of(store: &AiConfigStore, provider: &str) -> Option<String> {
    store
        .active_config()
        .providers
        .get(provider)
        .and_then(|c| c.base_url.clone())
}

/// A seed/import snapshot naming `active` plus the given provider rows (no
/// stage overrides).
pub(super) fn snapshot_of(
    active: &str,
    providers: Vec<(&str, ProviderConfig)>,
) -> AiConfigSnapshot {
    AiConfigSnapshot {
        active_provider: Some(active.to_string()),
        providers: providers
            .into_iter()
            .map(|(id, cfg)| (id.to_string(), cfg))
            .collect(),
        stage_overrides: Default::default(),
    }
}
