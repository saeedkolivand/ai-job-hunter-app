use super::support::*;

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
        base_url_of(&store, "openai"),
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
        base_url_of(&store, "openai-compatible").as_deref(),
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
