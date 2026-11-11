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

/// The message a failed run reports (ADR-032 point 7 — observability is content-free).
///
/// **A known kind gets its fixed sentence and none of the provider's text.** Provider free text
/// can echo prompt or résumé content (a refusal quoting a name, a 422 echoing the input) or
/// carry an injected instruction, and shape redaction catches none of that; this message reaches
/// agents unfenced. Free text survives only for the app-authored `Validation` and `Config`
/// errors (redacted and bounded); anything else is a generic sentence.
pub(super) fn message_of(error: &AppError) -> String {
    if let Some(kind) = FailureKind::of(error) {
        return kind.summary().to_string();
    }
    match error {
        AppError::Validation(_) | AppError::Config(_) => {
            redact_provider_error(AppError::Message(error.to_string()), &[]).to_string()
        }
        _ => "The run failed.".to_string(),
    }
}

/// Make the failed run readable: create the row if the run died before `execute` wrote one,
/// close it if it was left `running`, and stamp the kind + message into its metrics.
/// `job_url` is only used for a row that did not exist.
pub(super) fn record(
    store: &PipelineRunStore,
    run_id: &str,
    job_url: &str,
    started_at: u64,
    error: &AppError,
) {
    let now = crate::db::now_ms();
    let mut row = store.run(run_id).unwrap_or_else(|| RunRow {
        id: run_id.to_string(),
        job_url: job_url.to_string(),
        kind: RUN_KIND.to_string(),
        depth: RUN_DEPTH.to_string(),
        status: STATUS_FAILED.to_string(),
        started_at,
        finished_at: Some(now),
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    });
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

/// The retention key for a failed run that never got a row from `execute`: the resolved posting
/// where it can be known, else a key unique to this run. Never the shared `""` — failures pooled
/// there would be pruned three at a time, and the fourth would evict the first and bring back
/// the null this module exists to remove.
pub(super) fn failure_job_url(
    request_url: &str,
    source: Option<resolve::JobSource<'_>>,
    cached_url: Option<String>,
    run_id: &str,
) -> String {
    if !request_url.trim().is_empty() {
        return request_url.to_string();
    }
    match source {
        Some(resolve::JobSource::Cache(_)) => cached_url
            .filter(|url| !url.trim().is_empty())
            .unwrap_or_else(|| resolve::unlinked_run_key(run_id)),
        Some(resolve::JobSource::Text(text)) => resolve::unlinked_run_key(text),
        None => resolve::unlinked_run_key(run_id),
    }
}

/// Leave the failed run readable through `resume_pipeline_get` (see [`failure::record`]).
pub(super) fn record_failure(
    app: &AppHandle,
    run_id: &str,
    req: &ResumePipelineRunRequest,
    started_at: u64,
    error: &AppError,
) {
    // The run store's key for a row that never got one from `execute`: the same decision
    // `execute` makes once it has resolved the posting.
    let clamped = resolve::clamp_request(req);
    let source = resolve::job_source(&clamped.job_id, &clamped.job_ad_text);
    let cached_url = match source {
        Some(resolve::JobSource::Cache(id)) => {
            crate::commands::match_resume::job_meta_for(app, id).map(|meta| meta.url)
        }
        _ => None,
    };
    let job_url = failure_job_url(&clamped.job_url, source, cached_url, run_id);
    if let Some(store) = app.try_state::<PipelineRunStore>() {
        record(&store, run_id, &job_url, started_at, error);
    }
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
}

impl From<AppError> for ExecuteFailure {
    fn from(error: AppError) -> Self {
        Self { error, data: None }
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
    // A timeout's own text is app-authored and actionable; every other failure reports the same
    // message the run row does, so the job row never carries provider free text either.
    let message = match error {
        AppError::Timeout(_) => error.to_string(),
        _ => message_of(error),
    };
    crate::commands::jobs::job_fail_classified(app, job_id, message, data, FailureKind::of(error));
    cancels.unregister(job_id).await;
}
