//! The backup [`DataStore`] surface (`export`/`import`) and the import-only
//! hardening it enforces: identity/text columns are REJECTED past their cap
//! (never truncated), and the bundle's own row counts are bounded.

use crate::data_store::DataStore;
use crate::error::{AppError, AppResult};

use super::model::{
    clamp_artifact, clamp_metrics, RunEventRow, RunRow, IMPORT_ID_CAP_BYTES,
    IMPORT_JOB_URL_CAP_BYTES, IMPORT_LABEL_CAP_BYTES, IMPORT_MAX_EVENTS, IMPORT_MAX_RUNS,
};
use super::store::{normalized_job_url, PipelineRunStore};

/// REJECT one imported identity/text column that exceeds `cap` bytes.
///
/// The opposite of `clamp_json` (see [`super::model::clamp_artifact`]/
/// [`super::model::clamp_metrics`]), on purpose: `metrics_json` and
/// `artifact_json` are free-form SUMMARIES, so a truncated one still tells
/// the truth about a run. An identity column does not degrade that way — a
/// truncated `id` is a DIFFERENT run, a truncated `job_url` points at a
/// different posting, and a truncated `run_id` orphans its event. Shortening
/// those silently corrupts the trail, so the bundle is refused instead.
///
/// The message names the column and the sizes but never echoes the VALUE: it
/// reaches a log and a renderer toast, and this module is content-free by
/// construction (ADR-027) — quoting an oversized column would be the one place
/// user data leaked out of it.
fn check_len(column: &str, value: &str, cap: usize) -> AppResult<()> {
    if value.len() > cap {
        return Err(AppError::Validation(format!(
            "pipelineRuns: {column} is {} bytes, over the {cap}-byte cap",
            value.len()
        )));
    }
    Ok(())
}

/// Bound every identity/text column of one imported run.
fn check_run(run: &RunRow) -> AppResult<()> {
    check_len("run id", &run.id, IMPORT_ID_CAP_BYTES)?;
    check_len("run jobUrl", &run.job_url, IMPORT_JOB_URL_CAP_BYTES)?;
    check_len("run kind", &run.kind, IMPORT_LABEL_CAP_BYTES)?;
    check_len("run depth", &run.depth, IMPORT_LABEL_CAP_BYTES)?;
    check_len("run status", &run.status, IMPORT_LABEL_CAP_BYTES)?;
    if let Some(reason) = &run.stopped_reason {
        check_len("run stoppedReason", reason, IMPORT_LABEL_CAP_BYTES)?;
    }
    Ok(())
}

/// Bound every identity/text column of one imported event.
///
/// `phase` is absent BY DESIGN: it is closed at the schema by the CHECK in
/// [`super::store::CREATE_PIPELINE_RUNS_SQL`], which is a strictly stronger bound than any
/// length cap (six legal bytes, and a rolled-back transaction for anything
/// else). A second, weaker guard here would only raise the question of which
/// one is authoritative.
fn check_event(event: &RunEventRow) -> AppResult<()> {
    check_len("event runId", &event.run_id, IMPORT_ID_CAP_BYTES)?;
    check_len("event stage", &event.stage, IMPORT_LABEL_CAP_BYTES)?;
    Ok(())
}

/// Bound how many rows one bundle may restore.
///
/// Separate from [`check_run`]/[`check_event`] and taking plain counts so the
/// decision is testable at its boundary without building a 50 000-row fixture.
pub(super) fn check_bundle_size(runs: usize, events: usize) -> AppResult<()> {
    if runs > IMPORT_MAX_RUNS {
        return Err(AppError::Validation(format!(
            "pipelineRuns: bundle carries {runs} runs, over the {IMPORT_MAX_RUNS}-run cap"
        )));
    }
    if events > IMPORT_MAX_EVENTS {
        return Err(AppError::Validation(format!(
            "pipelineRuns: bundle carries {events} events, over the {IMPORT_MAX_EVENTS}-event cap"
        )));
    }
    Ok(())
}

/// The export/restore section: `{ "runs": [...], "events": [...] }`.
///
/// An OBJECT of two arrays rather than nested events-inside-runs, because the
/// two tables restore independently and a nested shape would make an event
/// whose run failed to deserialize silently vanish with it.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct RunsBundle {
    #[serde(default)]
    runs: Vec<RunRow>,
    #[serde(default)]
    events: Vec<RunEventRow>,
}

impl DataStore for PipelineRunStore {
    fn key(&self) -> &'static str {
        "pipelineRuns"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::json!(RunsBundle {
            runs: self.all_runs(),
            events: self.all_events(),
        })
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        // Deserialize EVERYTHING before mutating, so a malformed row aborts the
        // import without having cleared the tables (mirrors the other stores).
        let bundle: RunsBundle = serde_json::from_value(data.clone())
            .map_err(|e| AppError::Validation(format!("pipelineRuns: {e}")))?;

        // …and VALIDATE everything before mutating, for the same reason. The
        // row caps and the identity/text caps both run here — before the
        // transaction exists — so a refused bundle leaves the existing history
        // untouched rather than rolled back, and a 5 001-run bundle is refused
        // without inserting 5 000 of them first.
        check_bundle_size(bundle.runs.len(), bundle.events.len())?;
        for run in &bundle.runs {
            check_run(run)?;
        }
        for event in &bundle.events {
            check_event(event)?;
        }

        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM pipeline_run_events", [])?;
        tx.execute("DELETE FROM pipeline_runs", [])?;
        for run in &bundle.runs {
            tx.execute(
                "INSERT OR REPLACE INTO pipeline_runs
                    (id, job_url, kind, depth, status, started_at, finished_at,
                     stopped_reason, metrics_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    run.id,
                    // Re-NORMALIZE on import, for the same reason the two
                    // clamps below re-apply their write-site rules: a restore
                    // must not be able to produce a row the live write path
                    // could not. Binding this raw made every backup taken
                    // before the normalization landed restore rows in a
                    // spelling no reader and no delete could reach — the runs
                    // panel came back empty, `delete_for_job` matched nothing
                    // and still reported success, and the strategy/evidence
                    // detail those rows carry outlived the owner that was
                    // supposed to delete it. `data_import` runs against the
                    // LIVE store with no re-open, so
                    // `normalize_existing_job_urls` never gets to repair them.
                    normalized_job_url(&run.job_url),
                    run.kind,
                    run.depth,
                    run.status,
                    crate::db::ts_to_db(run.started_at),
                    run.finished_at.map(crate::db::ts_to_db),
                    run.stopped_reason,
                    // Re-clamp on import, exactly like `artifact_json` below: a
                    // hand-edited bundle must not be able to restore a row the
                    // live write path could never have produced.
                    clamp_metrics(&run.metrics_json),
                ],
            )?;
        }
        for event in &bundle.events {
            tx.execute(
                "INSERT OR REPLACE INTO pipeline_run_events
                    (run_id, seq, ts, stage, phase, artifact_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    event.run_id,
                    event.seq,
                    crate::db::ts_to_db(event.ts),
                    event.stage,
                    event.phase,
                    // Re-clamp on import: a hand-edited or legacy bundle must
                    // not be able to write past the cap the live path enforces.
                    clamp_artifact(&event.artifact_json),
                ],
            )?;
        }
        tx.commit()?;
        Ok(bundle.runs.len())
    }
}
