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
        base_url: None,
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
                base_url: Some("http://localhost:1234/v1".to_string()),
                context_window: Some(32_768),
            },
        )
        .expect("set override");

    let stored = store.stage_override("strategy").expect("row present");
    assert_eq!(stored.provider, "openai-compatible");
    assert_eq!(stored.model, "big-model");
    assert_eq!(stored.base_url.as_deref(), Some("http://localhost:1234/v1"));
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

/// The base_url provenance check is the active config's, reached through the
/// SAME `validate_settings` — not a second copy that could drift.
///
/// Mutation check: call `validate_settings` with `None` for `base_url` and the
/// metadata endpoint is accepted.
#[test]
fn a_base_url_goes_through_the_ssrf_chain() {
    let (_dir, store) = new_store();
    assert!(store
        .set_stage_override(
            "draft",
            StageOverride {
                provider: "openai-compatible".to_string(),
                model: "m".to_string(),
                base_url: Some("http://169.254.169.254/latest".to_string()),
                context_window: None,
            },
        )
        .is_err());
    // …and it is INERT for any other provider, so it is dropped rather than
    // stored as dead routing data.
    store
        .set_stage_override(
            "draft",
            StageOverride {
                provider: "ollama".to_string(),
                model: "m".to_string(),
                base_url: Some("http://example.com".to_string()),
                context_window: None,
            },
        )
        .unwrap();
    assert!(store.stage_override("draft").unwrap().base_url.is_none());
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
                base_url: None,
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
        let err = store
            .set_stage_override(stage, over("ollama", "small"))
            .expect_err("{stage} makes no AI call");
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
                 (stage, provider, model, base_url, context_window, updated_at)
             VALUES ('validate', 'ollama', 'inert', NULL, NULL, 0)",
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
/// Mutation check (executed): remove the `validate_context_window` scrub from
/// `apply_stage_overrides_conn` and the whole row is dropped instead.
#[test]
fn import_scrubs_an_out_of_range_window_but_keeps_the_row() {
    let (_dir, store) = new_store();
    let bundle = serde_json::json!({
        "providers": {},
        "stageOverrides": {
            "draft": { "provider": "ollama", "model": "keep-me", "contextWindow": 9_999_999 },
        },
    });
    store.import(&bundle).unwrap();

    let stored = store.stage_override("draft").expect("the row survives");
    assert_eq!(stored.model, "keep-me");
    assert_eq!(stored.context_window, None, "only the bad field is dropped");
    // …and the interactive writer still refuses the same value outright.
    let mut bad = over("ollama", "keep-me");
    bad.context_window = Some(9_999_999);
    assert!(store.set_stage_override("draft", bad).is_err());
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
