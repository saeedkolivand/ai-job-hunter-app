use super::{support::*, *};

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
