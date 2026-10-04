use super::{support::*, *};

/// An out-of-range window is scrubbed as a FIELD on the IMPORT path — the row
/// keeps its provider and model, because a window is a tuning knob rather than
/// part of the override's identity. The interactive writer still errors (a
/// human typed it and can be told); a restore has nobody to tell.
///
/// The VALUES matter here. `9_999_999` fits in a `u32`, so it only ever
/// exercised the post-parse scrub; `-1` and `2^32` do not fit the derived
/// `Option<u32>` at all, and before `lenient_context_window` they failed the
/// whole `aiProviderConfig` section — the sibling override, the provider rows
/// and the active provider all vanished with them. A worst case that stops
/// short of the real type boundary is not a worst case.
///
/// Mutation checks (both executed): remove the `validate_context_window` scrub
/// from `apply_stage_overrides_conn` and the in-range-but-absurd row is dropped
/// whole; revert `context_window` to a derived `Option<u32>` and the
/// unrepresentable cases take the entire restore down with them.
#[test]
fn import_scrubs_an_out_of_range_window_but_keeps_the_row() {
    for bad in [
        serde_json::json!(9_999_999u64),     // fits u32, outside the range
        serde_json::json!(-1),               // not a u32 at all
        serde_json::json!(4_294_967_296u64), // 2^32 — one past the type
        serde_json::json!("32768"),          // not even a number
        serde_json::json!(1.5),
    ] {
        let (_dir, store) = new_store();
        let bundle = serde_json::json!({
            "activeProvider": "ollama",
            "providers": { "ollama": { "model": "provider-model" } },
            "stageOverrides": {
                "draft": { "provider": "ollama", "model": "keep-me", "contextWindow": bad },
                "strategy": { "provider": "ollama", "model": "sibling" },
            },
        });
        store.import(&bundle).unwrap();

        let stored = store
            .stage_override("draft")
            .unwrap_or_else(|| panic!("the row must survive contextWindow={bad}"));
        assert_eq!(stored.model, "keep-me");
        assert_eq!(stored.context_window, None, "only the bad field is dropped");

        // Nothing else in the bundle pays for it.
        assert_eq!(
            store.stage_override("strategy").map(|o| o.model),
            Some("sibling".to_string()),
            "a sibling override must survive contextWindow={bad}"
        );
        let active = store.active_config();
        assert_eq!(active.active_provider.as_deref(), Some("ollama"));
        assert_eq!(active.model.as_deref(), Some("provider-model"));
    }

    // …and the interactive writer still refuses the in-range-but-absurd value
    // outright, because there a human typed it and can be told.
    let (_dir, store) = new_store();
    let mut bad = over("ollama", "keep-me");
    bad.context_window = Some(9_999_999);
    assert!(store.set_stage_override("draft", bad).is_err());
}

/// A row whose SHAPE cannot be salvaged costs only itself. The scrub test above
/// covers rows whose FIELDS are recoverable; these have nothing to recover —
/// and before the per-entry parse, each aborted the whole section.
///
/// Mutation check (executed): restore
/// `serde_json::from_value::<AiConfigSnapshot>(...)` in `import` and every case
/// below returns `Err` against an empty store.
#[test]
fn one_unparseable_override_entry_does_not_fail_the_restore() {
    for (name, bad) in [
        ("missing provider", serde_json::json!({ "model": "orphan" })),
        ("not an object", serde_json::json!("nonsense")),
        (
            "wrong-typed model",
            serde_json::json!({ "provider": "ollama", "model": 123 }),
        ),
        ("null entry", serde_json::json!(null)),
    ] {
        let (_dir, store) = new_store();
        let bundle = serde_json::json!({
            "activeProvider": "ollama",
            "providers": { "ollama": { "model": "provider-model" } },
            "stageOverrides": {
                "draft": bad,
                "strategy": { "provider": "ollama", "model": "sibling" },
            },
        });
        store
            .import(&bundle)
            .unwrap_or_else(|e| panic!("{name} must not fail the restore: {e}"));

        // The bad entry is gone…
        assert!(
            store.stage_override("draft").is_none(),
            "{name}: an unsalvageable row must not be restored"
        );
        // …and nothing else in the same bundle is.
        assert_eq!(
            store.stage_override("strategy").map(|o| o.model),
            Some("sibling".to_string()),
            "{name}: the sibling override must survive"
        );
        let active = store.active_config();
        assert_eq!(
            active.active_provider.as_deref(),
            Some("ollama"),
            "{name}: the active provider must survive"
        );
        assert_eq!(active.model.as_deref(), Some("provider-model"));
    }
}

/// The SEED path does not go through `from_untrusted` — `ai_seed_active_config`
/// is a Tauri command taking `AiConfigSnapshot` directly, so Tauri's own strict
/// deserialization is what a first-run renderer seed meets. That is where the
/// field-level defaults earn their keep: without them one malformed override
/// row rejects the entire seed payload, and the app first-runs with no provider
/// configured at all.
///
/// Mutation check (executed): remove `#[serde(default)]` from
/// `StageOverride::provider` and the parse below fails outright — the import
/// tests stay green, because the per-entry parse already covers that path. This
/// is the ONLY test that pins the attribute.
#[test]
fn a_strict_parse_still_tolerates_a_recoverable_override_row() {
    // Exactly what Tauri does with the command's argument.
    let snapshot: AiConfigSnapshot = serde_json::from_value(serde_json::json!({
        "activeProvider": "ollama",
        "providers": { "ollama": { "model": "seeded" } },
        "stageOverrides": {
            "draft": { "model": "orphan" },
            "strategy": { "provider": "ollama", "model": "sibling", "contextWindow": -1 },
        },
    }))
    .expect("a recoverable override row must not reject the whole seed");

    let (_dir, store) = new_store();
    assert!(store.seed_if_empty(&snapshot).expect("seed"));

    // The provider-less row is dropped by `validate_stage_override`, the
    // out-of-range window by the lenient read — and the seed itself lands.
    assert!(store.stage_override("draft").is_none());
    let kept = store
        .stage_override("strategy")
        .expect("the sibling survives");
    assert_eq!(kept.model, "sibling");
    assert_eq!(kept.context_window, None);
    assert_eq!(
        store.active_config().active_provider.as_deref(),
        Some("ollama")
    );
}

/// The same tolerance on the PROVIDER map and on `activeProvider`. The finding
/// was reported against stage overrides, but both take the identical
/// all-or-nothing path through the section parse, and the probe showed one bad
/// provider row killing the restore just as thoroughly.
///
/// Mutation check (executed): restore the strict section parse and both halves
/// return `Err`.
#[test]
fn one_bad_provider_row_does_not_fail_the_restore() {
    let (_dir, store) = new_store();
    store
        .import(&serde_json::json!({
            "activeProvider": "ollama",
            "providers": {
                "ollama": { "model": "good" },
                "openai": { "contextWindow": -1 },
                "anthropic": "nonsense",
            },
        }))
        .expect("one bad provider row must not fail the restore");
    let active = store.active_config();
    assert_eq!(active.active_provider.as_deref(), Some("ollama"));
    assert_eq!(active.model.as_deref(), Some("good"));
    // The recoverable row keeps its existence and loses only the bad field; the
    // unsalvageable one is simply absent.
    assert_eq!(
        active
            .providers
            .get("openai")
            .and_then(|c| c.context_window),
        None
    );
    assert!(!active.providers.contains_key("anthropic"));

    // A wrong-typed `activeProvider` reads as unseeded rather than fatal — a
    // state the store already handles — and the provider rows still land.
    let (_dir, store) = new_store();
    store
        .import(&serde_json::json!({
            "activeProvider": 123,
            "providers": { "ollama": { "model": "good" } },
        }))
        .expect("a wrong-typed activeProvider must not fail the restore");
    assert_eq!(store.active_config().active_provider, None);
    assert_eq!(store.active_config().providers.len(), 1);
}

/// The seed path shares the snapshot applier, so a first-run seed can carry
/// overrides — and is still row-presence gated.
#[test]
fn seed_carries_overrides_and_stays_gated() {
    let (_dir, store) = new_store();
    let mut snapshot = AiConfigSnapshot::default();
    snapshot
        .stage_overrides
        .insert("draft".to_string(), over("ollama", "seeded"));
    assert!(store.seed_if_empty(&snapshot).unwrap());
    assert_eq!(store.stage_override("draft").unwrap().model, "seeded");

    let mut second = AiConfigSnapshot::default();
    second
        .stage_overrides
        .insert("draft".to_string(), over("ollama", "clobber"));
    assert!(!store.seed_if_empty(&second).unwrap(), "seed is one-shot");
    assert_eq!(store.stage_override("draft").unwrap().model, "seeded");
}

/// A hand-edited out-of-range window must not take the whole ROW with it.
///
/// `get::<Option<u32>>` fails the row on a negative/oversized INTEGER, and the
/// `rows.flatten()` in `stage_overrides_conn` would swallow that as "no
/// override" — the stage would then run on the active provider, the silent
/// fallback `from_active_for_stage` promises never to make. The routing the
/// user chose survives; only the window they could not have set is dropped.
///
/// Mutation check (executed): read column 3 as `Option<u32>` again and both
/// rows vanish from the map.
#[test]
fn an_out_of_range_stored_window_drops_the_field_not_the_row() {
    let (_dir, store) = new_store();
    for (stage, raw) in [("draft", -1_i64), ("strategy", 999_999_999_i64)] {
        store
            .conn
            .lock()
            .execute(
                "INSERT INTO ai_stage_overrides
                     (stage, provider, model, context_window, updated_at)
                 VALUES (?1, 'ollama', 'chosen-model', ?2, 0)",
                rusqlite::params![stage, raw],
            )
            .unwrap();
    }

    let all = store.stage_overrides();
    for stage in ["draft", "strategy"] {
        let over = all
            .get(stage)
            .unwrap_or_else(|| panic!("{stage} must survive its bad window"));
        assert_eq!(over.provider, "ollama");
        assert_eq!(over.model, "chosen-model");
        assert_eq!(over.context_window, None, "the bad window is what drops");
    }
}
