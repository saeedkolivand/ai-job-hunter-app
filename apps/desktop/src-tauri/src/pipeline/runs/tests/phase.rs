//! The closed `phase` vocabulary: the schema CHECK matches the generated TS
//! contract, accepts every contract phase, rejects anything else, and the
//! import path enforces it too.

use std::collections::BTreeSet;

use crate::data_store::DataStore;
use crate::ipc_contracts::events::PIPELINE_STAGE_PHASES;
use crate::pipeline::runs::store::CREATE_PIPELINE_RUNS_SQL;

use super::support::{event, run, store};

/// The literals inside the events table's `CHECK (phase IN (...))`, as a set.
///
/// Parses the migration's STATIC SQL — the same string the migration executes,
/// so the two cannot diverge. A reformat that breaks this parse fails loudly
/// (the `expect` below) rather than passing vacuously.
fn phases_in_the_schema_check() -> BTreeSet<String> {
    const NEEDLE: &str = "CHECK (phase IN (";
    let after = CREATE_PIPELINE_RUNS_SQL
        .find(NEEDLE)
        .map(|i| &CREATE_PIPELINE_RUNS_SQL[i + NEEDLE.len()..])
        .expect("the events table must close `phase` with a `CHECK (phase IN (...))` clause");
    let body = &after[..after.find(')').expect("unterminated CHECK clause")];
    body.split(',')
        .map(|value| value.trim().trim_matches('\'').to_string())
        .collect()
}

/// DRIFT GUARD. `phase` is closed BY DESIGN (unlike `stopped_reason`, which is
/// deliberately loose TEXT so a new variant needs no migration), and its
/// vocabulary is frozen in TS — `PIPELINE_STAGE_PHASES` in
/// `packages/shared/src/events/pipeline.ts`, which `pnpm gen:ipc` emits into
/// `ipc_contracts::events` and CI re-checks. So: widening the TS array widens
/// the generated const, and this assertion then FAILS until someone appends the
/// migration that widens the CHECK on already-migrated installs too. Widening
/// the CHECK alone (a schema that accepts a phase the contract does not know)
/// fails here as well — the comparison is a set equality, not a subset.
#[test]
fn phase_check_matches_the_generated_contract() {
    let schema = phases_in_the_schema_check();
    let contract: BTreeSet<String> = PIPELINE_STAGE_PHASES
        .iter()
        .map(|phase| (*phase).to_string())
        .collect();
    assert!(
        !contract.is_empty(),
        "an empty contract vocabulary would make this comparison vacuous"
    );
    assert_eq!(
        schema, contract,
        "the `phase` CHECK and PIPELINE_STAGE_PHASES have drifted; a phase added \
         to the TS contract needs an APPENDED migration widening the CHECK"
    );
}

/// The CHECK is real at the write site, and it accepts exactly the contract's
/// values — a typo'd CHECK that dropped `start` would still reject the bogus
/// phase below, so the accepted half is pinned too.
#[test]
fn the_schema_accepts_every_contract_phase() {
    let (_dir, store) = store();
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    for (seq, phase) in PIPELINE_STAGE_PHASES.iter().enumerate() {
        let mut e = event("run-1", seq as u32, "{}");
        e.phase = (*phase).to_string();
        store
            .append_event(&e)
            .unwrap_or_else(|err| panic!("the schema must accept contract phase '{phase}': {err}"));
    }
    assert_eq!(
        store.events_for_run("run-1").len(),
        PIPELINE_STAGE_PHASES.len()
    );
}

/// …and rejects anything else, at the SCHEMA rather than in the caller: the
/// write path takes `phase: String`, so the table is the only place that can
/// stop a stage emitter (or a future one) from inventing a fourth phase.
#[test]
fn the_schema_rejects_an_out_of_vocabulary_phase() {
    let (_dir, store) = store();
    store.upsert_run(&run("run-1", "job-a", 10)).unwrap();
    let mut bogus = event("run-1", 0, "{}");
    bogus.phase = "cancelled".to_string();

    let err = store.append_event(&bogus).unwrap_err();
    assert!(
        err.to_string().to_uppercase().contains("CHECK"),
        "an unknown phase must violate the schema CHECK, got: {err}"
    );
    assert!(
        store.events_for_run("run-1").is_empty(),
        "the rejected event must not have landed"
    );
}

/// The import path enforces it too — that is the WHOLE point of putting the
/// vocabulary in the schema rather than in the emitter. A hand-edited bundle
/// with a bogus phase aborts the whole import (the transaction is dropped
/// without committing) and the pre-existing rows survive, exactly like the
/// deserialize-time abort above.
#[test]
fn import_rejects_an_out_of_vocabulary_phase_and_preserves_existing_data() {
    let (_dir, store) = store();
    store.upsert_run(&run("keep", "job-a", 1)).unwrap();

    let bundle = serde_json::json!({
        "runs": [{
            "id": "r", "jobUrl": "j", "kind": "resume", "depth": "full",
            "status": "done", "startedAt": 1, "metricsJson": "{}"
        }],
        "events": [{
            "runId": "r", "seq": 0, "ts": 1, "stage": "draft",
            "phase": "cancelled", "artifactJson": "{}"
        }]
    });
    assert!(store.import(&bundle).is_err());
    assert!(
        store.run("keep").is_some(),
        "existing rows survive an import rejected by the schema"
    );
    assert!(
        store.run("r").is_none(),
        "the run inserted before the bad event must be rolled back, not committed"
    );
}
