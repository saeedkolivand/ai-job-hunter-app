use super::{support::*, *};

// ── Seed: single-shot, row-presence gated ─────────────────────────────────────

#[test]
fn seed_applies_once_then_never_clobbers_a_later_set() {
    let (_dir, store) = new_store();
    let snapshot = snapshot_of(
        "openai",
        vec![("openai", provider_cfg(Some("gpt-4o"), None))],
    );

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
    let snapshot = snapshot_of(
        "openai-compatible",
        vec![(
            "openai-compatible",
            provider_cfg(None, Some("http://169.254.169.254/")),
        )],
    );
    assert!(store.seed_if_empty(&snapshot).unwrap());
    assert_eq!(
        base_url_of(&store, "openai-compatible"),
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
    let snapshot = snapshot_of(
        "openai",
        vec![
            (
                "openai",
                provider_cfg(Some("gpt-4o"), Some("https://sneaky.example/v1")),
            ),
            (
                "openai-compatible",
                provider_cfg(None, Some("http://localhost:1234/v1")),
            ),
        ],
    );

    assert!(store.seed_if_empty(&snapshot).unwrap());
    assert_eq!(
        base_url_of(&store, "openai"),
        None,
        "a native provider's base_url must be dropped to NULL on seed",
    );
    assert_eq!(
        base_url_of(&store, "openai-compatible").as_deref(),
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
    assert_eq!(
        base_url_of(&restored, "openai"),
        None,
        "a native provider's base_url must be dropped to NULL on import",
    );
    assert_eq!(
        base_url_of(&restored, "openai-compatible").as_deref(),
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
        base_url_of(&restored, "openai-compatible").as_deref(),
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
        base_url_of(&store, "openai-compatible"),
        None,
        "the tampered metadata base_url must be scrubbed on import",
    );
}
