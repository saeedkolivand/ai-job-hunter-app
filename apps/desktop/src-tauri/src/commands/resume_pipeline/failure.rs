//! A run that failed must be readable (#1496): the failure's kind and sanitized message, on the
//! run row, whether the run died before its row existed or after.
//!
//! `execute` writes the `running` row only after it has resolved the résumé, the posting and a
//! provider, so a failure in any of those left no row and `resume_pipeline_get` answered `null`
//! — indistinguishable from an unknown id. [`record`] makes the row exist. The kind and message
//! ride in the existing `metrics_json` (no migration); old rows simply lack them.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::commands::ai_provider::redact_provider_error;
use crate::error::{AppError, FailureKind};
use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;
use crate::jobs::cancel::CancelRegistry;
use crate::pipeline::runs::{PipelineRunStore, RunRow};

use super::{resolve, RUN_DEPTH, RUN_KIND, STATUS_FAILED, STATUS_RUNNING};

/// The sanitized message for a failure: the diagnostics-bundle redactor plus the length bound
/// (ADR-032 point 7 — observability is content-free). The text is the provider's or the app's
/// own error, never a prompt or résumé; the redactor strips URLs, paths, hosts and credentials.
pub(super) fn message_of(error: &AppError) -> String {
    redact_provider_error(AppError::Message(error.to_string()), &[]).to_string()
}

/// Make the failed run readable: create the row if the run died before `execute` wrote one,
/// close it if it was left `running`, and stamp the kind + message into its metrics.
/// `fallback_job_url` is the store key for a row that did not exist; `None` means no row may be
/// created (the run's own row was written and has since been deleted — the delete wins — or the
/// posting has no resolvable URL, which would pool every such failure under one empty key).
pub(super) fn record(
    store: &PipelineRunStore,
    run_id: &str,
    fallback_job_url: Option<&str>,
    started_at: u64,
    error: &AppError,
) {
    let now = crate::db::now_ms();
    let Some(mut row) = store.run(run_id).or_else(|| {
        Some(RunRow {
            id: run_id.to_string(),
            job_url: fallback_job_url?.to_string(),
            kind: RUN_KIND.to_string(),
            depth: RUN_DEPTH.to_string(),
            status: STATUS_FAILED.to_string(),
            started_at,
            finished_at: Some(now),
            stopped_reason: None,
            metrics_json: "{}".to_string(),
        })
    }) else {
        return;
    };
    if row.status == STATUS_RUNNING {
        row.status = STATUS_FAILED.to_string();
        row.finished_at = Some(now);
    }
    let mut metrics =
        serde_json::from_str::<Value>(&row.metrics_json).unwrap_or_else(|_| json!({}));
    if let Some(object) = metrics.as_object_mut() {
        if let Some(kind) = FailureKind::of(error) {
            object.insert("errorKind".to_string(), json!(kind.as_str()));
        }
        object.insert("errorMessage".to_string(), json!(message_of(error)));
    }
    row.metrics_json = metrics.to_string();
    if let Err(e) = store.upsert_run(&row) {
        log::warn!(
            "[resume_pipeline] could not record the failed run: {}",
            crate::observability::sanitize_reason(&e.to_string())
        );
        return;
    }
    store.prune();
}

/// The run store's key for a row `execute` never wrote: the same decision it makes once it has
/// resolved the posting (the cached posting's own URL wins on the `jobId` path). `None` when no
/// URL can be resolved — an empty key would drop the failure from the posting's run list.
pub(super) fn fallback_job_url(
    job_url: &str,
    source: Option<resolve::JobSource<'_>>,
    cached_url: impl FnOnce(&str) -> Option<String>,
) -> Option<String> {
    let own = match source {
        Some(resolve::JobSource::Cache(id)) => cached_url(id).filter(|u| !u.trim().is_empty()),
        _ => None,
    };
    let url = own.unwrap_or_else(|| job_url.to_string());
    let key = match source {
        Some(source) => resolve::run_store_job_url(&url, source),
        None => url,
    };
    (!key.trim().is_empty()).then_some(key)
}

/// Leave the failed run readable through `resume_pipeline_get` (see [`record`]).
/// `row_written`: `execute` already wrote this run's row, so a missing row was deleted and stays so.
pub(super) fn record_failure(
    app: &AppHandle,
    run_id: &str,
    req: &ResumePipelineRunRequest,
    started_at: u64,
    error: &AppError,
    row_written: bool,
) {
    let clamped = resolve::clamp_request(req);
    let job_url = (!row_written)
        .then(|| {
            fallback_job_url(
                &clamped.job_url,
                resolve::job_source(&clamped.job_id, &clamped.job_ad_text),
                |id| crate::commands::match_resume::job_meta_for(app, id).map(|m| m.url),
            )
        })
        .flatten();
    if let Some(store) = app.try_state::<PipelineRunStore>() {
        record(&store, run_id, job_url.as_deref(), started_at, error);
    }
}

/// A failed run's whole teardown: the readable row ([`record_failure`]), then the job and its
/// cancel registration ([`fail`]).
pub(super) async fn fail_run(
    app: &AppHandle,
    job_id: &str,
    run_id: &str,
    req: &ResumePipelineRunRequest,
    started_at: u64,
    failure: ExecuteFailure,
) {
    record_failure(
        app,
        run_id,
        req,
        started_at,
        &failure.error,
        failure.row_written,
    );
    let cancels = app
        .state::<std::sync::Arc<CancelRegistry>>()
        .inner()
        .clone();
    fail(app, &cancels, job_id, &failure.error, failure.data).await;
}

/// The failure [`execute`] hands its caller: a typed message plus OPTIONAL
/// structured data for `job.failed`'s event payload — for the one case a
/// caller can say more than the message text alone (currently just the
/// per-call timeout: WHICH stage, and how long — see
/// `hooks::timeout_failure_data`). `From<AppError>` covers every ordinary `?`
/// inside `execute` with `data: None`, so only the arm that actually has more
/// to say constructs this by hand.
pub(super) struct ExecuteFailure {
    pub(super) error: AppError,
    pub(super) data: Option<Value>,
    /// `execute` had already written this run's row when it failed (see [`record_failure`]).
    pub(super) row_written: bool,
}

impl ExecuteFailure {
    /// A failure after `execute` wrote the run's row.
    pub(super) fn after_row(error: AppError, data: Option<Value>) -> Self {
        Self {
            error,
            data,
            row_written: true,
        }
    }
}

impl From<AppError> for ExecuteFailure {
    fn from(error: AppError) -> Self {
        Self {
            error,
            data: None,
            row_written: false,
        }
    }
}

/// Mark the job failed and release its cancel registration — the two calls
/// every early return owes. `data`, when present, becomes `job.failed`'s
/// event payload INSTEAD of `message` (see
/// [`crate::commands::jobs::job_fail_with_data`]) — currently only
/// [`ExecuteFailure`]'s per-call-timeout arm ever sets one; every other
/// failure keeps riding as the plain message string it always has. The job row
/// also carries the failure's [`FailureKind`] (#1496).
pub(super) async fn fail(
    app: &AppHandle,
    cancels: &CancelRegistry,
    job_id: &str,
    error: &AppError,
    data: Option<Value>,
) {
    crate::commands::jobs::job_fail_classified(
        app,
        job_id,
        error.to_string(),
        data,
        FailureKind::of(error),
    );
    cancels.unregister(job_id).await;
}
