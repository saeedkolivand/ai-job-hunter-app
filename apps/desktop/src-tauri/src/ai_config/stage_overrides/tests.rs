//! Guards for the per-stage override table.
//!
//! Every test below was mutation-checked: the comment names the change that
//! makes it fail, and each was applied and reverted rather than assumed.

use tempfile::TempDir;

use super::{is_overridable_stage, is_pipeline_stage, StageOverride, MAX_STAGE_OVERRIDES};
use crate::ai_config::{AiConfigSnapshot, AiConfigStore};
use crate::data_store::DataStore;
use crate::ipc_contracts::events::PIPELINE_STAGES;

fn new_store() -> (TempDir, AiConfigStore) {
    let dir = TempDir::new().unwrap();
    let store = AiConfigStore::open(&dir.path().to_path_buf()).expect("open store");
    (dir, store)
}

fn over(provider: &str, model: &str) -> StageOverride {
    StageOverride {
        provider: provider.to_string(),
        model: model.to_string(),
        context_window: None,
    }
}

// ── Round-trip ──────────────────────────────────────────────────────────────

/// Mutation check: make `set_stage_override` a no-op and the read is empty;
/// drop `context_window` from the upsert and the last assertion fails.
#[test]
fn a_set_override_round_trips_with_every_field() {
    let (_dir, store) = new_store();
    store
        .set_stage_override(
            "strategy",
            StageOverride {
                provider: "openai-compatible".to_string(),
                model: "big-model".to_string(),
                context_window: Some(32_768),
            },
        )
        .expect("set override");

    let stored = store.stage_override("strategy").expect("row present");
    assert_eq!(stored.provider, "openai-compatible");
    assert_eq!(stored.model, "big-model");
    assert_eq!(stored.context_window, Some(32_768));
    assert_eq!(store.stage_overrides().len(), 1);
}

/// The load-bearing default: a stage nobody configured has NO row, so the
/// resolver falls through to the active provider instead of a guess.
///
/// Mutation check: seed a row for every stage in `open` and this fails.
#[test]
fn an_unset_stage_has_no_override() {
    let (_dir, store) = new_store();
    store
        .set_stage_override("draft", over("ollama", "small"))
        .unwrap();
    assert!(store.stage_override("draft").is_some());
    for stage in PIPELINE_STAGES.iter().filter(|s| **s != "draft") {
        assert!(
            store.stage_override(stage).is_none(),
            "{stage} must stay on the active provider until the user says otherwise",
        );
    }
}

/// Mutation check: make `clear_stage_override` a no-op and this fails.
#[test]
fn clearing_an_override_returns_the_stage_to_the_active_provider() {
    let (_dir, store) = new_store();
    store
        .set_stage_override("repair", over("ollama", "small"))
        .unwrap();
    store.clear_stage_override("repair").unwrap();
    assert!(store.stage_override("repair").is_none());
    // Clearing a stage that has no row is a no-op, not an error — the Settings
    // UI must be able to call it without first reading.
    store.clear_stage_override("repair").unwrap();
}

// ── Write-time validation (the SAME chain as the active config) ─────────────

/// The stage vocabulary is closed. Mutation check: delete the
/// `is_pipeline_stage` guard from `validate_stage_override` and this passes a
/// row nothing will ever read.
#[test]
fn an_unknown_stage_is_rejected() {
    let (_dir, store) = new_store();
    for stage in ["", "rewrite", "Draft", "draft ", "header"] {
        assert!(
            store
                .set_stage_override(stage, over("ollama", "small"))
                .is_err(),
            "{stage:?} is not a pipeline stage and must not be storable",
        );
    }
    assert!(store.stage_overrides().is_empty());
}

/// Mutation check: swap `ProviderId::parse` for a bare `to_string` and this
/// stores a provider nothing can resolve.
#[test]
fn an_unknown_provider_is_rejected() {
    let (_dir, store) = new_store();
    assert!(store
        .set_stage_override("draft", over("not-a-provider", "m"))
        .is_err());
    assert!(store.set_stage_override("draft", over("", "m")).is_err());
}

/// An override with no model is the one thing an override cannot be — unless
/// the provider is a CLI agent, which has its own configured default.
///
/// Mutation check: drop the `None =>` arm's error and an empty-model row
/// persists, failing at generation time instead of at the click.
#[test]
fn an_empty_model_is_rejected_except_for_cli_agents() {
    let (_dir, store) = new_store();
    assert!(store
        .set_stage_override("draft", over("ollama", "  "))
        .is_err());
    store
        .set_stage_override("draft", over("claude-code", ""))
        .expect("a CLI agent runs on its own default model");
    assert_eq!(store.stage_override("draft").unwrap().model, "");
}

/// A per-stage base URL is not merely validated — it is UNREPRESENTABLE, on
/// the wire and in the table. The endpoint follows the named provider's own
/// stored row, so an override can never point at an endpoint Settings does not
/// display.
///
/// The import path is the interesting one: a bundle is untrusted input, and it
/// is the only way a `baseUrl` key can still arrive. Serde drops the unknown
/// field, so the row lands with routing that resolves through the provider's
/// configured URL — a smuggled endpoint has nowhere to be stored.
///
/// Mutation check (executed): re-add a `base_url` field to `StageOverride` and
/// the import assertion below stops proving anything, because the smuggled URL
/// deserializes into it.
#[test]
fn an_import_bundle_cannot_smuggle_a_per_stage_base_url() {
    let (_dir, store) = new_store();
    let bundle = serde_json::json!({
        "providers": {},
        "stageOverrides": {
            "draft": {
                "provider": "openai-compatible",
                "model": "m",
                "baseUrl": "http://169.254.169.254/latest",
            },
        },
    });
    store.import(&bundle).unwrap();

    // The row is accepted on its provider+model, and carries no endpoint of its
    // own — the smuggled cloud-metadata URL is simply not part of the shape.
    let stored = store.stage_override("draft").expect("row present");
    assert_eq!(stored.provider, "openai-compatible");
    let json = serde_json::to_value(&stored).unwrap();
    assert!(
        json.get("baseUrl").is_none(),
        "an override must not carry an endpoint: {json}"
    );

    // And what it WILL resolve through is the provider's own row, which the
    // bundle left unset — not the smuggled value.
    assert_eq!(store.provider_base_url("openai-compatible"), None);
}

/// The endpoint FOLLOWS the provider's settings rather than snapshotting them
/// at write time: there is exactly one base URL per provider, so Settings can
/// never show one endpoint while a stage quietly uses another.
///
/// Mutation check (executed): make `provider_base_url` read a cached/copied
/// value instead of the live row and the post-change assertion fails.
#[test]
fn a_stage_override_follows_the_providers_current_base_url() {
    let (_dir, store) = new_store();
    let point_at = |url: &str| {
        store
            .set_provider_settings(crate::ai_config::ProviderSettingsPatch {
                provider: "openai-compatible".to_string(),
                model: Some(Some("m".to_string())),
                base_url: Some(Some(url.to_string())),
                context_window: None,
            })
            .expect("provider settings");
    };

    point_at("http://127.0.0.1:1234/v1");
    store
        .set_stage_override("draft", over("openai-compatible", "m"))
        .expect("set override");
    assert_eq!(
        store.provider_base_url("openai-compatible").as_deref(),
        Some("http://127.0.0.1:1234/v1")
    );

    // The user moves their local server. The override was never touched…
    point_at("http://127.0.0.1:9999/v1");
    assert_eq!(
        store.provider_base_url("openai-compatible").as_deref(),
        Some("http://127.0.0.1:9999/v1"),
        "the override must follow the provider, not a snapshot"
    );
    assert_eq!(
        store.stage_override("draft").unwrap().provider,
        "openai-compatible"
    );
}

/// `num_ctx` reaches a local inference server, where an absurd value is an
/// out-of-memory kill rather than a wrong answer.
///
/// Mutation check: remove the `validate_context_window` call from
/// `validate_settings` and both of these persist.
#[test]
fn an_out_of_range_context_window_is_rejected() {
    let (_dir, store) = new_store();
    for bad in [1_u32, 511, 131_073, u32::MAX] {
        let mut o = over("ollama", "small");
        o.context_window = Some(bad);
        assert!(
            store.set_stage_override("draft", o).is_err(),
            "{bad} is outside the supported context-window range",
        );
    }
    let mut ok = over("ollama", "small");
    ok.context_window = Some(512);
    store.set_stage_override("draft", ok).unwrap();
}

// ── Backup round-trip + import hardening (ADR-007 checklist) ────────────────

/// Overrides must survive a backup, or a restore silently returns every stage
/// to the active provider while Settings still shows the old plan.
///
/// Mutation check: drop `stage_overrides` from `snapshot()` (or from
/// `apply_snapshot_conn`) and this fails.
#[test]
fn overrides_survive_an_export_import_round_trip() {
    let (_dir, source) = new_store();
    source.set_active_provider("ollama").unwrap();
    source
        .set_provider_settings(crate::ai_config::ProviderSettingsPatch {
            provider: "ollama".to_string(),
            model: Some(Some("small".to_string())),
            base_url: None,
            context_window: Some(Some(8_192)),
        })
        .unwrap();
    source
        .set_stage_override(
            "strategy",
            StageOverride {
                provider: "ollama".to_string(),
                model: "big".to_string(),
                context_window: Some(16_384),
            },
        )
        .unwrap();

    let bundle = source.export();
    let (_dir2, restored) = new_store();
    restored.import(&bundle).expect("import the bundle");

    assert_eq!(restored.active_provider().as_deref(), Some("ollama"));
    assert_eq!(restored.active_config().context_window, Some(8_192));
    let over = restored
        .stage_override("strategy")
        .expect("override restored");
    assert_eq!(over.model, "big");
    assert_eq!(over.context_window, Some(16_384));
}

/// A tampered bundle cannot introduce a stage that never runs, a provider that
/// does not exist, or an egress endpoint the writer would have refused — and it
/// cannot fail the whole restore on one bad row either.
///
/// Mutation check: drop the `is_pipeline_stage` filter (or the
/// `validate_stage_override` call) from `apply_stage_overrides_conn` and the
/// junk rows land.
#[test]
fn import_drops_invalid_override_rows_and_keeps_the_good_one() {
    let (_dir, store) = new_store();
    let bundle = serde_json::json!({
        "providers": {},
        "stageOverrides": {
            "draft": { "provider": "ollama", "model": "good" },
            "rewrite": { "provider": "ollama", "model": "phantom-stage" },
            "repair": { "provider": "not-a-provider", "model": "x" },
            "validate": {
                "provider": "openai-compatible",
                "model": "x",
                "baseUrl": "http://169.254.169.254/latest",
            },
        },
    });
    store
        .import(&bundle)
        .expect("a bad row must not fail the restore");

    let stored = store.stage_overrides();
    assert_eq!(stored.keys().collect::<Vec<_>>(), vec!["draft"]);
    assert_eq!(stored["draft"].model, "good");
}

/// The named cap, not just the filter above it. `stage` is a PRIMARY KEY drawn
/// from a closed vocabulary, so a bundle cannot legitimately carry more rows
/// than there are stages — and an over-stuffed one WRITES at most the cap.
///
/// Counted against the raw table, not `stage_overrides()`: the read applies the
/// vocabulary filter too, so a read-side assertion would pass even with the
/// write side wide open. (Found exactly that way — the first version of this
/// test survived deleting both the filter and the `take`.)
///
/// Mutation checks (both executed): remove the vocabulary filter and the
/// `validate_stage_override` call from `apply_stage_overrides_conn`, and 509
/// rows land in the table. Separately, revert the filter alone to
/// `is_pipeline_stage` — free-stage rows then eat cap slots ahead of the real
/// stages they sort before, and this test fails while the free-stage test above
/// still passes. This is THE pin for the filter.
#[test]
fn import_never_persists_more_rows_than_the_vocabulary_has_stages() {
    let (_dir, store) = new_store();
    let mut overrides = serde_json::Map::new();
    for i in 0..500 {
        overrides.insert(
            format!("stage-{i}"),
            serde_json::json!({ "provider": "ollama", "model": "m" }),
        );
    }
    // Every real stage, free ones included — the free ones must be dropped and
    // must not count toward the cap.
    for stage in PIPELINE_STAGES {
        overrides.insert(
            (*stage).to_string(),
            serde_json::json!({ "provider": "ollama", "model": "m" }),
        );
    }
    let bundle = serde_json::json!({ "providers": {}, "stageOverrides": overrides });
    let written = store.import(&bundle).unwrap();

    let persisted: i64 = store
        .conn
        .lock()
        .query_row("SELECT COUNT(*) FROM ai_stage_overrides", [], |r| r.get(0))
        .unwrap();
    assert_eq!(persisted as usize, MAX_STAGE_OVERRIDES);
    assert_eq!(written, MAX_STAGE_OVERRIDES, "import reports what it wrote");
    assert!(
        MAX_STAGE_OVERRIDES < PIPELINE_STAGES.len(),
        "the free stages are not overridable, so the cap is below the vocabulary",
    );
    // The bundle really was over-stuffed relative to the cap being asserted.
    const _: () = assert!(MAX_STAGE_OVERRIDES < 500);
}

/// A bundle written before overrides existed still restores — the field is
/// defaulted, not required.
#[test]
fn a_pre_override_bundle_still_imports() {
    let (_dir, store) = new_store();
    let legacy = serde_json::json!({
        "activeProvider": "ollama",
        "providers": { "ollama": { "model": "small" } },
    });
    store.import(&legacy).expect("legacy bundle");
    assert_eq!(store.active_provider().as_deref(), Some("ollama"));
    assert!(store.stage_overrides().is_empty());
    // …and the absent context window stays absent rather than becoming 0.
    assert_eq!(store.active_config().context_window, None);
}

/// A factory reset / import-replace sweeps the overrides too. Mutation check:
/// remove the `DELETE FROM ai_stage_overrides` from `clear_conn` and a
/// "cleared" store still routes `strategy` at the old model.
#[test]
fn clear_removes_the_overrides_as_well() {
    let (_dir, store) = new_store();
    store
        .set_stage_override("strategy", over("ollama", "big"))
        .unwrap();
    store.clear();
    assert!(store.stage_overrides().is_empty());
}

// ── Vocabulary ──────────────────────────────────────────────────────────────

/// `is_pipeline_stage` answers off the GENERATED list, not a local copy.
#[test]
fn the_stage_guard_accepts_exactly_the_generated_vocabulary() {
    for stage in PIPELINE_STAGES {
        assert!(is_pipeline_stage(stage));
    }
    for other in ["", " ", "DRAFT", "draft2", "header", "fast"] {
        assert!(!is_pipeline_stage(other));
    }
}

/// A stage that makes no provider call has no model to choose — and, before
/// this, a malformed row on one could still abort a whole run at resolve time
/// (`Completer::for_stages` propagates every override's error).
///
/// Mutation check (executed): delete the `is_overridable_stage` arm from
/// `validate_stage_override` and both writes succeed.
#[test]
fn a_stage_that_makes_no_ai_call_cannot_be_overridden() {
    use crate::ipc_contracts::events::PIPELINE_STAGES_FREE;

    let (_dir, store) = new_store();
    for stage in PIPELINE_STAGES_FREE {
        let Err(err) = store.set_stage_override(stage, over("ollama", "small")) else {
            panic!("{stage} makes no AI call, so the write must be refused")
        };
        assert!(format!("{err}").contains("no AI call"), "got {err}");
        assert!(!is_overridable_stage(stage));
        // …and it is still a REAL stage — the two checks answer different
        // questions, and conflating them would reject a live stage name.
        assert!(is_pipeline_stage(stage));
    }
    assert!(store.stage_overrides().is_empty());
}

/// The READ side refuses a free stage too — the third belt, and the one that
/// covers a row already sitting in the table from an older release whose
/// vocabulary still paid for that stage.
///
/// Written through the raw table rather than through `set_stage_override`,
/// which would refuse it: the point is a row that is already there.
///
/// Mutation check (executed): neuter the `is_overridable_stage` filter in
/// `stage_overrides_conn` (the row-level `out.insert` guard) and the inert row
/// is handed to the resolver through BOTH readers.
///
/// Precisely what this does NOT pin: the early return in `stage_override`.
/// Deleting it leaves this test green — and that is correct, because it is a
/// redundant fast path, not a belt: `stage_override` reads through
/// `stage_overrides_conn`, which filters the row out anyway. No test can
/// distinguish its presence, so none claims to.
#[test]
fn a_free_stage_row_already_in_the_table_is_not_read_back() {
    let (_dir, store) = new_store();
    store
        .conn
        .lock()
        .execute(
            "INSERT INTO ai_stage_overrides
                 (stage, provider, model, context_window, updated_at)
             VALUES ('validate', 'ollama', 'inert', NULL, 0)",
            [],
        )
        .unwrap();

    // Present in the table…
    let raw: i64 = store
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM ai_stage_overrides WHERE stage = 'validate'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw, 1, "the fixture must actually be in the table");

    // …and invisible to both readers, so nothing can resolve it.
    assert!(store.stage_override("validate").is_none());
    assert!(!store.stage_overrides().contains_key("validate"));
}

/// The same refusal on the import path, where the row arrives from an untrusted
/// bundle rather than from the Settings writer.
///
/// This pins the OUTCOME (no free-stage row is ever persisted), which two
/// independent layers guarantee: the `is_overridable_stage` filter, and
/// `validate_stage_override`'s own arm via the `else { continue }`. So it does
/// NOT pin the filter by itself — reverting the filter to `is_pipeline_stage`
/// leaves this test green, because the validate arm still drops the row.
/// (An earlier version of this comment claimed otherwise; the mutation was run
/// and did not reproduce.) What pins the filter specifically is
/// `import_never_persists_more_rows_than_the_vocabulary_has_stages`: the filter
/// runs BEFORE `.take(MAX_STAGE_OVERRIDES)`, so it is what stops a free-stage
/// row from consuming a cap slot that a real stage needed.
#[test]
fn import_drops_an_override_on_a_free_stage() {
    let (_dir, store) = new_store();
    let bundle = serde_json::json!({
        "providers": {},
        "stageOverrides": {
            "validate": { "provider": "ollama", "model": "inert" },
            "draft": { "provider": "ollama", "model": "good" },
        },
    });
    store.import(&bundle).unwrap();
    assert_eq!(
        store.stage_overrides().keys().collect::<Vec<_>>(),
        vec!["draft"]
    );
}

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
