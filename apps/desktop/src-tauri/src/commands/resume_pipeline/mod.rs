//! The staged résumé pipeline's IPC surface (L3).
//!
//! Five commands over `pipeline::resume`, plus [`hooks::RunHooks`] — the ONE
//! place a `pipeline:stage` event is emitted or a run row is written.
//!
//! ## What is backend-owned
//!
//! * **Routing** — `Completer::from_active`, never the request (task #25).
//! * **The budget** — `Budget::RESUME_QUALITY`, a compile-time constant. The
//!   wire schema has no `maxSteps`/`maxTokens`/`runTimeout` field to bind, so a
//!   compromised renderer has no unbounded-spend knob (pinned by
//!   `run_request_carries_only_identity_no_budget`).
//! * **The inputs** — the résumé text comes from the `DocumentStore` by id and
//!   the posting text from the postings cache by id, so a prompt is never built
//!   from renderer-supplied document bodies.
//!
//! ## Where a run's pieces live
//!
//! The run store (`pipeline_runs.db`) holds the LIFECYCLE — status, stopped
//! reason, metrics, the per-stage trail. The DOCUMENT and its quality report
//! live in `ai_generations`, keyed by the posting url, because that is already
//! the per-job aggregate every other surface reads. `get`/`listForJob` join the
//! two rather than storing a second copy of a résumé.

pub mod hooks;
pub mod report;

#[cfg(test)]
mod test;

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::ai_generations::{make_generation_id, AiGenerationRecord, AiGenerationStore};
use crate::commands::ai_provider::timeouts;
use crate::db::new_job_id;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};
use crate::ipc_contracts::resume_pipeline::{
    ResumePipelineRegenerateSectionRequest, ResumePipelineResolveFabricationRequest,
    ResumePipelineRunRequest,
};
use crate::jobs::cancel::CancelRegistry;
use crate::pipeline::budget::{Budget, StoppedReason};
use crate::pipeline::cache::KvCache;
use crate::pipeline::resume::stages::regenerate_one_section;
use crate::pipeline::resume::types::{GenerationDepth, SectionKey};
use crate::pipeline::resume::{
    quality_pipeline, run_deadline, QualityCtx, QualityInput, RunLedger,
};
use crate::pipeline::runs::{PipelineRunStore, RunRow};
use crate::pipeline::Completer;

use self::hooks::RunHooks;

/// The `pipeline_runs.kind` discriminator for a résumé run.
///
/// Deliberately the FLOW, not the depth: the store's retention partitions on
/// `(job_url, kind)`, so every résumé run of a posting shares one three-run
/// history regardless of depth, while a future agent run of the same posting
/// keeps its own. Depth rides in the `depth` column, where it can change
/// without re-partitioning anyone's history.
pub const RUN_KIND: &str = "resume";

const STATUS_RUNNING: &str = "running";
const STATUS_COMPLETED: &str = "completed";
const STATUS_NEEDS_REVIEW: &str = "needsReview";
const STATUS_FAILED: &str = "failed";
const STATUS_CANCELLED: &str = "cancelled";

/// Start one staged résumé run. Returns `{ runId, jobId }` immediately; stage
/// progress streams as `pipeline:stage` and the draft's deltas as `ai:stream`
/// under the same `jobId`.
///
/// Every fail-able step runs INSIDE the spawned task, for the reason
/// `commands::agent::agent_run` documents at length: the returned `jobId` is
/// the renderer's only handle, so a terminal event emitted before this returns
/// is silently dropped and the run looks stuck at pending forever.
#[tauri::command]
pub async fn resume_pipeline_run(app: AppHandle, req: ResumePipelineRunRequest) -> Value {
    let job_id = new_job_id();
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    crate::commands::jobs::job_start(&app, &job_id, "resumePipeline.run");

    let cancel = CancellationToken::new();
    let cancels = app.state::<Arc<CancelRegistry>>().inner().clone();
    // Registered BEFORE the spawn (mirrors `agent_run`/`scrape_boards`) so a
    // `jobs_cancel` arriving between this return and the task waking is not a
    // no-op.
    cancels.register(&job_id, cancel.clone()).await;

    let limiter = app.state::<Arc<crate::limits::Limiter>>().inner().clone();
    let app_task = app.clone();
    let job_id_task = job_id.clone();
    let run_id_task = run_id.clone();

    tauri::async_runtime::spawn(async move {
        // `acquire_queued`, not `acquire`: this is a deliberate human action, so
        // the 3rd concurrent run waits its turn instead of being thrown away —
        // the same call the tailoring path makes for the same reason. The wait
        // happens after the ids were returned, so the renderer can show the run
        // as queued.
        let _guard = match limiter
            .acquire_queued(
                "agent_run",
                crate::limits::AGENT_RUN_RATE_MAX,
                crate::limits::AGENT_RUN_CONCURRENCY_MAX,
                crate::limits::AGENT_RUN_QUEUE_MAX,
                |ahead| crate::commands::jobs::job_queued(&app_task, &job_id_task, ahead),
            )
            .await
        {
            Ok((guard, parked)) => {
                if parked {
                    crate::commands::jobs::job_dequeued(&app_task, &job_id_task);
                }
                guard
            }
            Err(e) => {
                fail(&app_task, &cancels, &job_id_task, e.to_string()).await;
                return;
            }
        };

        if let Err(e) = execute(&app_task, &run_id_task, &job_id_task, &req, &cancel).await {
            fail(&app_task, &cancels, &job_id_task, e.to_string()).await;
            return;
        }
        cancels.unregister(&job_id_task).await;
    });

    json!({ "runId": run_id, "jobId": job_id })
}

/// Mark the job failed and release its cancel registration — the two calls
/// every early return owes.
async fn fail(app: &AppHandle, cancels: &CancelRegistry, job_id: &str, message: String) {
    crate::commands::jobs::job_fail(app, job_id, message);
    cancels.unregister(job_id).await;
}

/// The run itself, from resolution through persistence.
///
/// Split out of the spawn so every failure is ONE `?` and the task body stays
/// readable — and so the ordering (resolve → record `running` → run → record
/// terminal) is visible in one place.
async fn execute(
    app: &AppHandle,
    run_id: &str,
    job_id: &str,
    req: &ResumePipelineRunRequest,
    cancel: &CancellationToken,
) -> AppResult<()> {
    let depth = GenerationDepth::from_wire(&req.depth)
        .ok_or_else(|| AppError::Validation(format!("unknown generation depth: {}", req.depth)))?;
    match depth {
        GenerationDepth::Quality => {}
        // `fast` is the untouched single-shot TS path — routing it here would
        // silently change what the user asked for. `max` is Phase 4; accepting
        // it and running the quality stages would be a lie about what ran.
        GenerationDepth::Fast | GenerationDepth::Max => {
            return Err(AppError::Validation(format!(
                "the staged pipeline runs at quality depth; {} is not handled here",
                depth.as_str()
            )))
        }
    }

    let completer = Completer::from_active(app)?;
    let resume = app
        .state::<DocumentStore>()
        .get(&req.resume_id)
        .ok_or_else(|| AppError::Validation(format!("resume not found: {}", req.resume_id)))?;
    let job_ad = crate::commands::match_resume::job_text_for(app, &req.job_id)
        .ok_or_else(|| AppError::Validation(format!("job not found in cache: {}", req.job_id)))?;
    let meta = crate::commands::match_resume::job_meta_for(app, &req.job_id).unwrap_or_default();
    // The posting's OWN url wins over the request's: it was resolved
    // server-side from the cache, and it is the retention + aggregate key.
    let job_url = if meta.url.trim().is_empty() {
        req.job_url.clone()
    } else {
        meta.url.clone()
    };

    let span = crate::observability::Span::begin(
        "pipeline:resume",
        format!(
            "op=run depth={} effort={}",
            depth.as_str(),
            req.effort.as_deref().unwrap_or("-")
        ),
    );

    let store = app.state::<PipelineRunStore>();
    let started_at = crate::db::now_ms();
    let mut row = RunRow {
        id: run_id.to_string(),
        job_url: job_url.clone(),
        kind: RUN_KIND.to_string(),
        depth: depth.as_str().to_string(),
        status: STATUS_RUNNING.to_string(),
        started_at,
        finished_at: None,
        stopped_reason: None,
        metrics_json: "{}".to_string(),
    };
    // Written BEFORE the run: a crash mid-run leaves a `running` row a user can
    // see, rather than no evidence the run happened.
    store.upsert_run(&row)?;

    let ledger = Arc::new(RunLedger::new());
    let deadline = run_deadline(
        Budget::RESUME_QUALITY,
        timeouts::quality_run_deadline(req.effort.as_deref()),
    );
    let hooks = RunHooks::new(
        app.clone(),
        run_id.to_string(),
        job_id.to_string(),
        cancel.clone(),
        deadline,
        Arc::clone(&ledger),
    );

    let cache = app.try_state::<KvCache>();
    let mut ctx = QualityCtx::new(
        QualityInput {
            source_resume: &resume.text,
            job_ad: &job_ad,
            target_language: &req.target_language,
            top_requirements: &req.top_requirements,
            cover_letter: &req.cover_letter_text,
            effort: req.effort.as_deref(),
            job_id,
        },
        &completer,
        cache.as_deref(),
        Arc::clone(&ledger),
    );

    let outcome = quality_pipeline().run_hooked(&mut ctx, &hooks).await;
    let stopped = ledger.stopped();
    let cancelled = stopped == Some(StoppedReason::Cancelled);

    // Persist whatever the run produced BEFORE deciding how it ended: a run
    // stopped at the repair stage still wrote a real document, and discarding
    // it because the report is not clean is the opposite of what the terminal
    // review is for.
    let quality_report = persist_document(app, &job_url, &meta, req, &ctx, depth.as_str());
    let needs_review = quality_report
        .as_deref()
        .is_some_and(report::still_needs_review)
        || ctx.critical_count() > 0;

    let status = match (&outcome, cancelled) {
        (_, true) => STATUS_CANCELLED,
        (Err(_), _) => STATUS_FAILED,
        (Ok(()), _) if needs_review => STATUS_NEEDS_REVIEW,
        (Ok(()), _) => STATUS_COMPLETED,
    };
    row.status = status.to_string();
    row.finished_at = Some(crate::db::now_ms());
    row.stopped_reason = Some(
        serde_json::to_value(stopped.unwrap_or(StoppedReason::Done))
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "done".to_string()),
    );
    let mut metrics = ledger.metrics();
    if let Some(object) = metrics.as_object_mut() {
        object.insert("ms".to_string(), json!(hooks.elapsed_ms()));
        object.insert(
            "issueCount".to_string(),
            json!(ctx.report.as_ref().map(|r| r.issues.len())),
        );
        object.insert("criticalCount".to_string(), json!(ctx.critical_count()));
        // PROVENANCE, not a metric — and the only place it can live without a
        // migration. `regenerateSection` re-validates the spliced document, and
        // a re-validation is only meaningful against the SOURCE résumé; the
        // aggregate stores the output, not the input. An id is content-free
        // (ADR-027), which is why this column can carry it at all.
        object.insert("sourceResumeId".to_string(), json!(req.resume_id));
    }
    row.metrics_json = metrics.to_string();
    store.upsert_run(&row)?;
    // Retention runs at the END of a run rather than on a timer: this is the
    // moment a fourth run for this posting exists.
    store.prune();

    // Codes and counts only (ADR-027) — never the résumé, the posting, or an
    // evidence span.
    span.end_with(
        &format!(
            "status={status} stopped={} criticals={}",
            row.stopped_reason.as_deref().unwrap_or("-"),
            ctx.critical_count()
        ),
        outcome.is_ok(),
    );

    match outcome {
        Ok(()) => {
            crate::commands::jobs::job_complete(
                app,
                job_id,
                json!({ "runId": run_id, "status": status, "text": ctx.draft }),
            );
            Ok(())
        }
        Err(_) if cancelled => {
            crate::commands::jobs::job_cancel(app, job_id);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Merge the finished document + a FRESH quality report into the per-job
/// aggregate, returning the wrapper that was written.
///
/// The Phase-1 merge rule, mechanically: **any save writing `resume_text`
/// carries a fresh `quality_report`.** They go in the same record for exactly
/// that reason — a save that wrote the text and left the report to a second
/// call would leave a window where the panel describes the previous document.
///
/// `None` when there was nothing to save (an empty draft, or a run that failed
/// before validation): a record with no text and no report is not an aggregate
/// update, it is noise.
fn persist_document(
    app: &AppHandle,
    job_url: &str,
    meta: &crate::commands::match_resume::JobPostingMeta,
    req: &ResumePipelineRunRequest,
    ctx: &QualityCtx<'_>,
    depth: &str,
) -> Option<String> {
    let report = ctx.report.as_ref()?;
    if ctx.draft.trim().is_empty() {
        return None;
    }
    let wrapper = report::build(
        depth,
        crate::db::now_ms(),
        Some((report, &ctx.draft)),
        ctx.letter_report
            .as_ref()
            .map(|letter| (letter, req.cover_letter_text.as_str())),
    );
    let store = app.try_state::<AiGenerationStore>()?;
    let record = AiGenerationRecord {
        id: make_generation_id(),
        created_at: crate::db::now_ms(),
        target_language: req.target_language.clone(),
        top_requirements: req.top_requirements.clone(),
        resume_text: ctx.draft.clone(),
        job_ad: String::new(),
        job_url: job_url.to_string(),
        board: meta.board.clone(),
        company_name: meta.company.clone(),
        job_title: meta.title.clone(),
        quality_report: wrapper.clone(),
        ..empty_record()
    };
    match store.save_application(record) {
        Ok(_) => Some(wrapper),
        Err(e) => {
            // Non-fatal, and logged rather than swallowed: the run itself
            // succeeded and its text is already in the renderer's hands via the
            // stream, so failing the run here would discard a good document
            // because a merge-upsert lost a race.
            log::warn!("[pipeline] could not persist the generated résumé (non-fatal): {e}");
            None
        }
    }
}

/// The all-empty record the pipeline's own save fills three fields of.
///
/// `merge_application` keeps the EXISTING value for every field an incoming
/// record leaves empty, so an aggregate's cover letter, answers, interview
/// questions and company brief survive a résumé-only save untouched. Written as
/// one helper rather than `Default` because `AiGenerationRecord` has no
/// meaningful default id or timestamp.
fn empty_record() -> AiGenerationRecord {
    AiGenerationRecord {
        id: String::new(),
        created_at: 0,
        candidate_name: String::new(),
        job_title: String::new(),
        company_name: String::new(),
        resume_language: String::new(),
        job_ad_language: String::new(),
        target_language: String::new(),
        mismatch: false,
        top_requirements: Vec::new(),
        mode: String::new(),
        resume_text: String::new(),
        cover_letter_text: String::new(),
        job_ad: String::new(),
        job_url: String::new(),
        board: String::new(),
        application_answers: Vec::new(),
        company_brief: String::new(),
        interview_questions: Vec::new(),
        email_subject: String::new(),
        email_body: String::new(),
        application_id: None,
        quality_report: String::new(),
    }
}

// ── Read surface ─────────────────────────────────────────────────────────────

/// One run with its stage trail, its report and its document. `null` for an
/// unknown id.
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

/// The full run: its summary, its stage trail, and the document + report it
/// produced (joined from `ai_generations` — see the module doc).
fn detail(app: &AppHandle, row: &RunRow) -> Value {
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
                // A CLAMPED artifact is not parseable JSON by design (the
                // truncation marker is not), so a reader must see the raw
                // string rather than a silent `{}` that claims the stage
                // reported nothing.
                "artifact": serde_json::from_str::<Value>(&event.artifact_json)
                    .unwrap_or_else(|_| Value::String(event.artifact_json.clone())),
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

// ── Write surface ────────────────────────────────────────────────────────────

/// Re-generate ONE section of a finished run and splice it back.
///
/// **`"header"` is rejected here, at the boundary**, and not by a special case:
/// [`SectionKey::from_wire`] runs the generated `is_pipeline_section_key`
/// grammar, which has no header token — so the contact header the editor owns
/// at export time (ADR-0021) is unreachable from this command by construction,
/// along with every other invented section name.
#[tauri::command]
pub async fn resume_pipeline_regenerate_section(
    app: AppHandle,
    req: ResumePipelineRegenerateSectionRequest,
) -> AppResult<Value> {
    let key = SectionKey::from_wire(&req.section_key).ok_or_else(|| {
        AppError::Validation(format!(
            "{:?} is not a section this pipeline can regenerate. The contact header is owned \
             by the editor at export time and is never model-written.",
            req.section_key
        ))
    })?;

    let store = app.state::<PipelineRunStore>();
    let row = store
        .run(&req.run_id)
        .ok_or_else(|| AppError::Validation(format!("run not found: {}", req.run_id)))?;
    let generations = app
        .try_state::<AiGenerationStore>()
        .ok_or_else(|| AppError::Storage("the generation store is unavailable".to_string()))?;
    let record = generations
        .find_for_job(&row.job_url)
        .filter(|record| !record.resume_text.trim().is_empty())
        .ok_or_else(|| {
            AppError::Validation(
                "this run has no saved résumé to regenerate a section of".to_string(),
            )
        })?;

    let span = crate::observability::Span::begin(
        "pipeline:resume",
        format!("op=regenerate_section key={}", key.to_wire()),
    );
    let completer = Completer::from_active(&app)?;
    let source = source_resume_for(&app, &row, &record);
    let spliced = regenerate_one_section(
        &completer,
        &source,
        &record.target_language,
        &record.resume_text,
        key,
        // No validator issues on this path: the user, not a report, asked for
        // the change. The note carries the "why", fenced.
        &[],
        req.note.as_deref(),
    )
    .await?;
    let Some(spliced) = spliced else {
        span.end(false);
        return Err(AppError::Provider(
            "The model's replacement section came back empty or truncated, so nothing was \
             changed. Try again."
                .to_string(),
        ));
    };

    // The merge rule again: this save writes `resume_text`, so it carries a
    // FRESH report over the spliced document — never the stale one the panel
    // was showing.
    let (report, letter) = crate::pipeline::resume::stages::validate_documents(
        spliced.clone(),
        source,
        record.job_ad.clone(),
        record.top_requirements.clone(),
        record.target_language.clone(),
        record.cover_letter_text.clone(),
    )
    .await?;
    let wrapper = report::build(
        &row.depth,
        crate::db::now_ms(),
        Some((&report, &spliced)),
        letter
            .as_ref()
            .map(|letter| (letter, record.cover_letter_text.as_str())),
    );
    generations.update_texts(&record.id, Some(spliced), None)?;
    generations.update_quality_report(&record.id, wrapper)?;
    span.end_with(
        &format!(
            "issues={} blocking={}",
            report.issues.len(),
            report::has_criticals(&report)
        ),
        true,
    );

    Ok(detail(&app, &row))
}

/// Record the user's Remove/Keep verdict on ONE surviving fabrication finding.
///
/// Nothing is removed here — the decision is RECORDED. Removing a bullet is a
/// text edit the user makes (or accepts) in the editor; a command that silently
/// deleted lines from a document on a single click would be exactly the
/// "nothing is removed silently" rule inverted.
#[tauri::command]
pub async fn resume_pipeline_resolve_fabrication(
    app: AppHandle,
    req: ResumePipelineResolveFabricationRequest,
) -> AppResult<Value> {
    if req.decision != "remove" && req.decision != "keep" {
        return Err(AppError::Validation(format!(
            "unknown fabrication decision: {}",
            req.decision
        )));
    }
    let store = app.state::<PipelineRunStore>();
    let row = store
        .run(&req.run_id)
        .ok_or_else(|| AppError::Validation(format!("run not found: {}", req.run_id)))?;
    let generations = app
        .try_state::<AiGenerationStore>()
        .ok_or_else(|| AppError::Storage("the generation store is unavailable".to_string()))?;
    let record = generations
        .find_for_job(&row.job_url)
        .ok_or_else(|| AppError::Validation("this run has no saved report".to_string()))?;

    if let Some(updated) =
        report::record_decision(&record.quality_report, &req.issue_key, &req.decision)
    {
        // The run leaves `needsReview` only when NOTHING is blocking any more:
        // every flagged bullet decided AND no Critical the review cannot clear
        // (`factual.dropped_role` names an absence, so it is not in the panel —
        // and a run that flipped to `completed` because every *reviewable*
        // finding was decided would present a résumé that silently lost an
        // employer as clean).
        let cleared = !report::still_needs_review(&updated);
        generations.update_quality_report(&record.id, updated)?;
        if cleared && row.status == STATUS_NEEDS_REVIEW {
            let mut row = row.clone();
            row.status = STATUS_COMPLETED.to_string();
            store.upsert_run(&row)?;
            return Ok(detail(&app, &row));
        }
    }
    Ok(detail(&app, &row))
}

/// The SOURCE résumé a re-validation must measure against.
///
/// A generation record stores the OUTPUT, not the input it was built from, so
/// the source is looked up through the id the run recorded as provenance (see
/// `execute`'s `sourceResumeId`). When that document is gone — deleted,
/// replaced, or the run predates the field — the fallback is the generated text
/// itself: measured against itself, the factual checks find nothing, so the
/// re-validated report comes back WEAKER than the run's original rather than
/// wrong. Weaker-and-honest beats a fabricated Critical against a source this
/// document was never written from.
fn source_resume_for(app: &AppHandle, row: &RunRow, record: &AiGenerationRecord) -> String {
    serde_json::from_str::<Value>(&row.metrics_json)
        .ok()
        .and_then(|metrics| {
            metrics
                .get("sourceResumeId")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .and_then(|id| app.state::<DocumentStore>().get(&id))
        .map(|document| document.text)
        .unwrap_or_else(|| record.resume_text.clone())
}
