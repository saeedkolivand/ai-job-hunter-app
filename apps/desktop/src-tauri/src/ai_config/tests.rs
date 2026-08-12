//! Round-trip, validation, seed-gating, reset, and backup tests for
//! [`AiConfigStore`] — the invariant lock for the backend-owned provider store.

use tempfile::TempDir;

use super::{AiConfigSnapshot, AiConfigStore, ProviderConfig, ProviderSettingsPatch};
use crate::data_store::DataStore;

fn new_store() -> (TempDir, AiConfigStore) {
    let dir = TempDir::new().unwrap();
    let store = AiConfigStore::open(&dir.path().to_path_buf()).expect("open store");
    (dir, store)
}

/// A patch that names EVERY field explicitly — the shape the old
/// replace-semantics writer had, so the tests below keep their meaning. Tests
/// that care about ABSENCE build the patch themselves.
fn full_patch(
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

fn provider_cfg(model: Option<&str>, base_url: Option<&str>) -> ProviderConfig {
    ProviderConfig {
        model: model.map(str::to_string),
        base_url: base_url.map(str::to_string),
        context_window: None,
    }
}

// ── Defaults ────────────────────────────────────────────────────────────────

#[test]
fn unseeded_store_has_no_active_provider() {
    let (_dir, store) = new_store();
    assert!(store.active_provider().is_none());
    assert!(!store.is_seeded());
    let cfg = store.active_config();
    assert!(cfg.active_provider.is_none());
    assert!(cfg.model.is_none());
    assert!(cfg.base_url.is_none());
    assert!(cfg.providers.is_empty());
}

// ── Switch-vs-edit round-trip ─────────────────────────────────────────────────

#[test]
fn set_provider_settings_and_active_roundtrips() {
    let (_dir, store) = new_store();
    store
        .set_provider_settings(full_patch(
            "openai-compatible",
            Some("some-model"),
            Some("http://localhost:1234/v1"),
            None,
        ))
        .expect("edit settings");
    store
        .set_active_provider("openai-compatible")
        .expect("switch active");

    let cfg = store.active_config();
    assert_eq!(cfg.active_provider.as_deref(), Some("openai-compatible"));
    assert_eq!(cfg.model.as_deref(), Some("some-model"));
    assert_eq!(cfg.base_url.as_deref(), Some("http://localhost:1234/v1"));
    assert_eq!(
        cfg.providers.get("openai-compatible"),
        Some(&provider_cfg(
            Some("some-model"),
            Some("http://localhost:1234/v1")
        )),
    );
    assert!(store.is_seeded());
}

#[test]
fn editing_settings_does_not_flip_the_active_provider() {
    // The switch-vs-edit split must survive: editing one provider's settings must
    // not change which provider is active (a combined setter would be a regression).
    let (_dir, store) = new_store();
    store.set_active_provider("ollama").unwrap();
    store
        .set_provider_settings(full_patch("openai", Some("gpt-4o"), None, None))
        .unwrap();
    assert_eq!(
        store.active_provider().as_deref(),
        Some("ollama"),
        "editing openai's settings must not make it active",
    );
}

// ── Writer validation (the SSRF provenance gate) ──────────────────────────────

#[test]
fn writer_rejects_unknown_provider() {
    let (_dir, store) = new_store();
    assert!(store.set_active_provider("totally-made-up").is_err());
    assert!(store
        .set_provider_settings(full_patch("totally-made-up", None, None, None))
        .is_err());
}

#[test]
fn writer_rejects_cross_family_model() {
    let (_dir, store) = new_store();
    // A Claude model on the OpenAI provider is an unambiguous cross-family mistake.
    assert!(store
        .set_provider_settings(full_patch("openai", Some("claude-3-5-sonnet"), None, None))
        .is_err());
}

#[test]
fn writer_rejects_non_http_base_url_scheme() {
    let (_dir, store) = new_store();
    let err = store
        .set_provider_settings(full_patch(
            "openai-compatible",
            None,
            Some("ftp://evil.test/v1"),
            None,
        ))
        .unwrap_err();
    assert!(
        format!("{err}").to_lowercase().contains("scheme"),
        "got {err}"
    );
}

#[test]
fn writer_rejects_cloud_metadata_base_url() {
    let (_dir, store) = new_store();
    // 169.254.169.254 — the cloud-metadata credential-theft pivot. Blocked even
    // though loopback/LAN gateways are allowed (see below).
    assert!(store
        .set_provider_settings(full_patch(
            "openai-compatible",
            None,
            Some("http://169.254.169.254/latest/meta-data/"),
            None,
        ))
        .is_err());
}

#[test]
fn writer_drops_base_url_to_null_for_a_native_provider() {
    // `resolve()` only honors base_url for `openai-compatible`; a value stored
    // against a native provider (e.g. openai) is inert for egress but still
    // reaches `record_usage`'s free/paid cost gate, so it must be dropped to
    // NULL rather than persisted.
    let (_dir, store) = new_store();
    store
        .set_provider_settings(full_patch(
            "openai",
            Some("gpt-4o"),
            Some("https://sneaky.example/v1"),
            None,
        ))
        .expect("edit settings");
    assert_eq!(
        store
            .active_config()
            .providers
            .get("openai")
            .and_then(|c| c.base_url.clone()),
        None,
        "a native provider's base_url must be dropped to NULL",
    );

    // An openai-compatible base_url is the one kind that must survive.
    store
        .set_provider_settings(full_patch(
            "openai-compatible",
            None,
            Some("http://localhost:1234/v1"),
            None,
        ))
        .expect("edit settings");
    assert_eq!(
        store
            .active_config()
            .providers
            .get("openai-compatible")
            .and_then(|c| c.base_url.clone())
            .as_deref(),
        Some("http://localhost:1234/v1"),
        "an openai-compatible base_url must be retained",
    );
}

#[test]
fn writer_accepts_localhost_lan_and_public_base_urls() {
    let (_dir, store) = new_store();
    // The whole point of provenance-not-IP-filtering: local gateways stay legal.
    for url in [
        "http://127.0.0.1:11434",       // Ollama
        "http://localhost:1234/v1",     // LM Studio
        "http://192.168.1.50:8000/v1",  // on-prem LAN vLLM
        "https://openrouter.ai/api/v1", // public gateway
    ] {
        assert!(
            store
                .set_provider_settings(full_patch("openai-compatible", None, Some(url), None))
                .is_ok(),
            "{url} must be accepted",
        );
    }
}

// ── Patch semantics: absent keeps, null clears, value sets ──────────────────
//
// Replace-everything was the first design and it failed on first contact —
// three renderer call sites each saved one field and erased the other two. The
// three tests below are the mechanism that replaced the doc comment.

/// Deserialization is where absent and null stop being the same thing: a plain
/// `Option<Option<T>>` collapses both to `None`, which is exactly the bug.
///
/// Mutation check (executed): drop `deserialize_with = "double_option"` from
/// the `model` field and the explicit-null case reads as absent.
#[test]
fn a_patch_distinguishes_an_absent_field_from_an_explicit_null() {
    let absent: ProviderSettingsPatch =
        serde_json::from_value(serde_json::json!({ "provider": "ollama" })).unwrap();
    assert_eq!(absent.model, None, "absent must not look like a clear");
    assert_eq!(absent.base_url, None);
    assert_eq!(absent.context_window, None);

    let cleared: ProviderSettingsPatch = serde_json::from_value(serde_json::json!({
        "provider": "ollama", "model": null, "baseUrl": null, "contextWindow": null,
    }))
    .unwrap();
    assert_eq!(cleared.model, Some(None), "explicit null must mean clear");
    assert_eq!(cleared.base_url, Some(None));
    assert_eq!(cleared.context_window, Some(None));

    let set: ProviderSettingsPatch = serde_json::from_value(serde_json::json!({
        "provider": "ollama", "model": "m", "contextWindow": 8_192,
    }))
    .unwrap();
    assert_eq!(set.model, Some(Some("m".to_string())));
    assert_eq!(set.context_window, Some(Some(8_192)));
    assert_eq!(set.base_url, None, "an untouched field stays untouched");
}

/// The write half: saving ONE field must leave the others exactly as they were.
///
/// Mutation check (executed): make `set_provider_settings` ignore `stored` and
/// pass the patch fields straight through — every "unchanged" assertion fails.
#[test]
fn saving_one_field_keeps_the_others() {
    let (_dir, store) = new_store();
    store
        .set_provider_settings(full_patch(
            "openai-compatible",
            Some("first-model"),
            Some("http://localhost:1234/v1"),
            Some(8_192),
        ))
        .unwrap();

    // Only the model — the shape a "pick a model" click sends.
    store
        .set_provider_settings(ProviderSettingsPatch {
            provider: "openai-compatible".to_string(),
            model: Some(Some("second-model".to_string())),
            ..Default::default()
        })
        .unwrap();

    let cfg = store.active_config();
    let row = cfg.providers.get("openai-compatible").unwrap();
    assert_eq!(row.model.as_deref(), Some("second-model"));
    assert_eq!(
        row.base_url.as_deref(),
        Some("http://localhost:1234/v1"),
        "an absent baseUrl must not erase the stored one",
    );
    assert_eq!(
        row.context_window,
        Some(8_192),
        "an absent contextWindow must not erase the stored one",
    );
}

/// …and an explicit null still clears, so "unset this" remains expressible.
///
/// Mutation check: make the merge `patch.field.flatten().or(stored.field)` —
/// null then reads as absent and nothing clears.
#[test]
fn an_explicit_null_clears_a_stored_field() {
    let (_dir, store) = new_store();
    store
        .set_provider_settings(full_patch(
            "openai-compatible",
            Some("m"),
            Some("http://localhost:1234/v1"),
            Some(8_192),
        ))
        .unwrap();
    store
        .set_provider_settings(ProviderSettingsPatch {
            provider: "openai-compatible".to_string(),
            base_url: Some(None),
            context_window: Some(None),
            ..Default::default()
        })
        .unwrap();

    let cfg = store.active_config();
    let row = cfg.providers.get("openai-compatible").unwrap();
    assert_eq!(row.base_url, None);
    assert_eq!(row.context_window, None);
    assert_eq!(row.model.as_deref(), Some("m"), "and only what was nulled");
}

/// The MERGED result is validated, not just the changed fields: patching in a
/// cross-family model must fail even though the caller sent nothing else.
#[test]
fn a_patch_is_validated_against_the_merged_row() {
    let (_dir, store) = new_store();
    store
        .set_provider_settings(full_patch("openai", Some("gpt-4o"), None, None))
        .unwrap();
    assert!(store
        .set_provider_settings(ProviderSettingsPatch {
            provider: "openai".to_string(),
            model: Some(Some("claude-3-5-sonnet".to_string())),
            ..Default::default()
        })
        .is_err());
    assert_eq!(
        store
            .active_config()
            .providers
            .get("openai")
            .and_then(|c| c.model.clone())
            .as_deref(),
        Some("gpt-4o"),
        "a rejected patch must leave the stored row untouched",
    );
}

// ── Seed: single-shot, row-presence gated ─────────────────────────────────────

#[test]
fn seed_applies_once_then_never_clobbers_a_later_set() {
    let (_dir, store) = new_store();
    let mut providers = std::collections::BTreeMap::new();
    providers.insert("openai".to_string(), provider_cfg(Some("gpt-4o"), None));
    let snapshot = AiConfigSnapshot {
        active_provider: Some("openai".to_string()),
        providers,
        stage_overrides: Default::default(),
    };

    assert!(
        store.seed_if_empty(&snapshot).unwrap(),
        "first seed applies"
    );
    assert_eq!(store.active_provider().as_deref(), Some("openai"));

    // A later explicit switch — then a second seed must NOT overwrite it.
    store.set_active_provider("ollama").unwrap();
    let seeded_again = store.seed_if_empty(&snapshot).unwrap();
    assert!(
        !seeded_again,
        "second seed must be a no-op (row-presence gated)"
    );
    assert_eq!(
        store.active_provider().as_deref(),
        Some("ollama"),
        "seed must not clobber the later explicit set",
    );
}

#[test]
fn seed_scrubs_a_malicious_base_url() {
    // A first-run XSS could seed a malicious base_url; the seed path scrubs it.
    let (_dir, store) = new_store();
    let mut providers = std::collections::BTreeMap::new();
    providers.insert(
        "openai-compatible".to_string(),
        provider_cfg(None, Some("http://169.254.169.254/")),
    );
    let snapshot = AiConfigSnapshot {
        active_provider: Some("openai-compatible".to_string()),
        providers,
        stage_overrides: Default::default(),
    };
    assert!(store.seed_if_empty(&snapshot).unwrap());
    assert_eq!(
        store
            .active_config()
            .providers
            .get("openai-compatible")
            .and_then(|c| c.base_url.clone()),
        None,
        "the cloud-metadata base_url must be scrubbed on seed",
    );
}

#[test]
fn seed_and_import_drop_base_url_for_a_native_provider() {
    // Mirrors `writer_drops_base_url_to_null_for_a_native_provider` but for the
    // lenient `scrub_settings` path (seed + import) — the first-run-XSS seed
    // vector and the restored-backup vector the security review called out.
    let (_dir, store) = new_store();
    let mut providers = std::collections::BTreeMap::new();
    providers.insert(
        "openai".to_string(),
        provider_cfg(Some("gpt-4o"), Some("https://sneaky.example/v1")),
    );
    providers.insert(
        "openai-compatible".to_string(),
        provider_cfg(None, Some("http://localhost:1234/v1")),
    );
    let snapshot = AiConfigSnapshot {
        active_provider: Some("openai".to_string()),
        providers,
        stage_overrides: Default::default(),
    };

    assert!(store.seed_if_empty(&snapshot).unwrap());
    let cfg = store.active_config();
    assert_eq!(
        cfg.providers.get("openai").and_then(|c| c.base_url.clone()),
        None,
        "a native provider's base_url must be dropped to NULL on seed",
    );
    assert_eq!(
        cfg.providers
            .get("openai-compatible")
            .and_then(|c| c.base_url.clone())
            .as_deref(),
        Some("http://localhost:1234/v1"),
        "an openai-compatible base_url must be retained on seed",
    );

    // Same guard on the import (restored-backup) path, via a fresh store.
    let (_dir2, restored) = new_store();
    let bundle = serde_json::json!({
        "activeProvider": "openai",
        "providers": {
            "openai": { "model": "gpt-4o", "baseUrl": "https://sneaky.example/v1" },
            "openai-compatible": { "baseUrl": "http://localhost:1234/v1" },
        }
    });
    restored.import(&bundle).expect("import");
    let restored_cfg = restored.active_config();
    assert_eq!(
        restored_cfg
            .providers
            .get("openai")
            .and_then(|c| c.base_url.clone()),
        None,
        "a native provider's base_url must be dropped to NULL on import",
    );
    assert_eq!(
        restored_cfg
            .providers
            .get("openai-compatible")
            .and_then(|c| c.base_url.clone())
            .as_deref(),
        Some("http://localhost:1234/v1"),
        "an openai-compatible base_url must be retained on import",
    );
}

// ── Factory reset ─────────────────────────────────────────────────────────────

#[test]
fn clear_wipes_active_and_provider_settings() {
    let (_dir, store) = new_store();
    store
        .set_provider_settings(full_patch("openai", Some("gpt-4o"), None, None))
        .unwrap();
    store.set_active_provider("openai").unwrap();
    assert!(store.is_seeded());

    store.clear();
    assert!(!store.is_seeded());
    assert!(store.active_provider().is_none());
    assert!(store.active_config().providers.is_empty());
}

// ── Backup export / import round-trip ─────────────────────────────────────────

#[test]
fn export_import_roundtrips_the_snapshot() {
    let (_dir, store) = new_store();
    store
        .set_provider_settings(full_patch(
            "openai-compatible",
            Some("mixtral"),
            Some("http://localhost:1234/v1"),
            None,
        ))
        .unwrap();
    store.set_active_provider("openai-compatible").unwrap();

    let bundle = store.export();

    let (_dir2, restored) = new_store();
    let n = restored.import(&bundle).expect("import");
    assert_eq!(n, 1, "one provider row restored");
    assert_eq!(
        restored.active_provider().as_deref(),
        Some("openai-compatible"),
    );
    assert_eq!(
        restored
            .active_config()
            .providers
            .get("openai-compatible")
            .and_then(|c| c.base_url.clone())
            .as_deref(),
        Some("http://localhost:1234/v1"),
    );
}

#[test]
fn import_scrubs_a_tampered_base_url() {
    // A tampered backup bundle must never restore a cloud-metadata egress target.
    let (_dir, store) = new_store();
    let bundle = serde_json::json!({
        "activeProvider": "openai-compatible",
        "providers": {
            "openai-compatible": { "model": "x", "baseUrl": "http://169.254.169.254/" }
        }
    });
    store.import(&bundle).expect("import");
    assert_eq!(
        store
            .active_config()
            .providers
            .get("openai-compatible")
            .and_then(|c| c.base_url.clone()),
        None,
        "the tampered metadata base_url must be scrubbed on import",
    );
}
