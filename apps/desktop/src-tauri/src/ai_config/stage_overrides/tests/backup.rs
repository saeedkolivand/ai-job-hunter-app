use super::{support::*, *};

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
