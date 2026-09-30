//! The read surface: [`resume_pipeline_get`] and [`resume_pipeline_list_for_job`].
//!
//! One run with its stage trail, plus the posting's CURRENT document and
//! report. The two halves have different owners and different lifetimes —
//! see the module doc on [`super`]. `status`/`stoppedReason`/`metrics`/`events`
//! are this run's own and never change again; `resumeText`/`report` are the
//! live `ai_generations` aggregate, so they may be newer than the run (a
//! later run, a section regeneration, a re-check, or the user's own edit). A
//! `report` whose `sourceTextHash` no longer matches `resumeText` is stale BY
//! DESIGN, not corrupt: it is the verdict on an earlier version of the same
//! document.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ai_generations::AiGenerationStore;
use crate::pipeline::runs::{PipelineRunStore, RunRow};

use super::hooks;
use super::RUN_KIND;

#[tauri::command]
pub async fn resume_pipeline_get(app: AppHandle, run_id: String) -> Value {
    let store = app.state::<PipelineRunStore>();
    match store.run(&run_id) {
        Some(row) => detail(&app, &row),
        None => Value::Null,
    }
}

/// The retained runs for one posting, newest first.
///
/// Filtered to this flow's own `kind`: the tables host every staged run, so an
/// unfiltered list would show a future agent run in the résumé runs panel.
///
/// Every field of a summary is the RUN's own — there is no document or report
/// here, so nothing in this list can go stale against the shared aggregate. Only
/// [`resume_pipeline_get`] joins the two.
#[tauri::command]
pub async fn resume_pipeline_list_for_job(app: AppHandle, job_url: String) -> Value {
    let store = app.state::<PipelineRunStore>();
    let runs: Vec<Value> = store
        .runs_for_job(&job_url)
        .into_iter()
        .filter(|row| row.kind == RUN_KIND)
        .map(|row| summary(&row))
        .collect();
    json!(runs)
}

/// The summary half of a run — everything but the trail and the document.
fn summary(row: &RunRow) -> Value {
    json!({
        "runId": row.id,
        "jobUrl": row.job_url,
        "kind": row.kind,
        "depth": row.depth,
        "status": row.status,
        "startedAt": row.started_at,
        "finishedAt": row.finished_at,
        "stoppedReason": row.stopped_reason,
        "metrics": serde_json::from_str::<Value>(&row.metrics_json).unwrap_or_else(|_| json!({})),
    })
}

/// ONE persisted stage artifact, as the WIRE carries it: the counts, never a
/// nested detail.
///
/// The max-depth per-entry regenerate this once served (`RunLedger::record_detail`)
/// was removed with the `max` generation depth, so no NEW row ever carries a
/// nested `hooks::DETAIL_KEY` value — but an EXISTING `pipeline_run_events` row
/// from before this deletion still can, and there is no migration touching it.
/// Stripping the key here is what keeps such a row's employment history and
/// verbatim résumé quotes from reaching the renderer over IPC (the contract
/// types `artifact` as `unknown`; there has never been a consumer for it).
///
/// **An unparseable artifact becomes a content-free MARKER, not the raw
/// string.** Returning the raw bytes was the right answer while artifacts were
/// counts-only — a reader must not see a silent `{}` claiming the stage
/// reported nothing. But the only artifact large enough for the store's clamp
/// to truncate was a detail-bearing one, so the raw-string arm WAS the leak,
/// with the truncation marker on the end. The marker keeps the one thing the
/// reader needed (this artifact did not survive intact) and carries nothing
/// else.
pub(super) fn wire_artifact(artifact_json: &str) -> Value {
    match serde_json::from_str::<Value>(artifact_json) {
        Ok(Value::Object(mut object)) => {
            object.remove(hooks::DETAIL_KEY);
            Value::Object(object)
        }
        Ok(other) => other,
        Err(_) => json!({ "truncated": true }),
    }
}

/// The full run: its summary, its stage trail, and the posting's CURRENT
/// document + report (joined from `ai_generations` — see the module doc; the
/// join is by `job_url`, so what comes back is the aggregate's state now, not a
/// snapshot of what this run emitted).
pub(super) fn detail(app: &AppHandle, row: &RunRow) -> Value {
    let store = app.state::<PipelineRunStore>();
    let events: Vec<Value> = store
        .events_for_run(&row.id)
        .into_iter()
        .map(|event| {
            json!({
                "seq": event.seq,
                "ts": event.ts,
                "stage": event.stage,
                "phase": event.phase,
                "artifact": wire_artifact(&event.artifact_json),
            })
        })
        .collect();

    let record = app
        .try_state::<AiGenerationStore>()
        .and_then(|store| store.find_for_job(&row.job_url));
    let mut out = summary(row);
    if let Some(object) = out.as_object_mut() {
        object.insert("events".to_string(), json!(events));
        object.insert(
            "resumeText".to_string(),
            json!(record
                .as_ref()
                .map(|r| r.resume_text.clone())
                .unwrap_or_default()),
        );
        object.insert(
            "report".to_string(),
            record
                .as_ref()
                .and_then(|r| serde_json::from_str::<Value>(&r.quality_report).ok())
                .unwrap_or(Value::Null),
        );
    }
    out
}
