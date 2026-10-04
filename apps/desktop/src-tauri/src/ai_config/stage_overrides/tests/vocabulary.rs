use super::{support::*, *};

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
    use crate::ipc_contracts::events::PIPELINE_STAGES_FREE;

    let (_dir, store) = new_store();
    // Both free stages, written through the raw table rather than through
    // `set_stage_override`, which would refuse them: the point is rows that
    // are already there — from an older release whose vocabulary still paid
    // for one of these stages.
    for stage in PIPELINE_STAGES_FREE {
        store
            .conn
            .lock()
            .execute(
                "INSERT INTO ai_stage_overrides
                     (stage, provider, model, context_window, updated_at)
                 VALUES (?1, 'ollama', 'inert', NULL, 0)",
                [stage],
            )
            .unwrap();
    }

    // Present in the table…
    let raw: i64 = store
        .conn
        .lock()
        .query_row("SELECT COUNT(*) FROM ai_stage_overrides", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        raw,
        PIPELINE_STAGES_FREE.len() as i64,
        "the fixtures must actually be in the table"
    );

    // …and invisible to both readers, so nothing can resolve them.
    assert!(store.stage_overrides().is_empty());
    for stage in PIPELINE_STAGES_FREE {
        assert!(store.stage_override(stage).is_none());
    }
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
