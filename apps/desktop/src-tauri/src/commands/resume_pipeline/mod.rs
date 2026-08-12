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
//!
//! ## The two halves diverge, on purpose (the settled semantics)
//!
//! A run row is IMMUTABLE history; the aggregate is LIVE state. So the document
//! `get` returns is the posting's current résumé — which may be newer than the
//! run that produced it, because four things can move it: a later run,
//! [`resume_pipeline_regenerate_section`], the renderer's re-check save, and the
//! user's own editing (applying a "Remove" verdict is an ordinary hand edit).
//! Nothing tries to prevent that; the rules that make it coherent are:
//!
//! * **Run-owned, never overwritten:** `status`, `stoppedReason`, `metrics`,
//!   `depth`, `startedAt`/`finishedAt` and the stage trail. They describe what
//!   THIS run did.
//! * **Aggregate-owned, always current:** `resumeText` and `report`. Both come
//!   from `find_for_job`, so an older run's `get` shows the newest document — see
//!   [`ensure_latest_run`] for why the write commands refuse rather than fork.
//! * **`report.<slot>.sourceTextHash` is the join between them.** When it stops
//!   matching `resumeText`, the report describes an EARLIER version of the
//!   document; the renderer renders that as "checked before your edits" until a
//!   re-check. Stale-and-labelled is the honest state, and it is why an edit
//!   never has to invalidate a report.
//! * **A verdict survives the move.** [`report::record_decision`] stamps by
//!   `issueKey` inside the persisted wrapper and reads no text, and the editor's
//!   save path (`AiGenerationStore::update_texts`) never touches
//!   `quality_report`. What a caller must NOT do is write a fresh wrapper whose
//!   slot drops `fabrications`: `merge_quality_report` merges per TOP-LEVEL key,
//!   so an incoming `resume` slot replaces the stored one WHOLE. Carrying the
//!   review list forward is the writer's obligation (the renderer's
//!   `mergeRecheckedReport` does it; `report::build` reissues it from the fresh
//!   report). Both halves are pinned by
//!   `an_edit_that_moves_the_document_leaves_every_verdict_landable` and
//!   `a_save_replaces_a_slot_whole_so_the_writer_owns_the_review_list`.

pub mod hooks;
pub mod max;
pub mod notify;
pub mod report;

#[cfg(test)]
mod max_test;
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
use crate::pipeline::cache::KvCache;
use crate::pipeline::resume::stages::{self, regenerate_one_section, section_gen, SectionOutcome};
use crate::pipeline::resume::types::{GenerationDepth, SectionKey};
use crate::pipeline::resume::{QualityCtx, QualityInput, RunDeadline, RunLedger};
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
pub(crate) const STATUS_COMPLETED: &str = "completed";
pub(crate) const STATUS_NEEDS_REVIEW: &str = "needsReview";
pub(crate) const STATUS_FAILED: &str = "failed";
pub(crate) const STATUS_CANCELLED: &str = "cancelled";

/// The renderer-supplied free text of one run request, CLAMPED server-side.
///
/// The wire schema's `.max(…)` caps are Zod, and Zod does not run on this
/// transport (`tauri-client` calls `invoke` directly, no parse on the way out),
/// so serde accepts whatever a direct IPC caller sends. Every other command
/// that takes this same text mirrors its caps in Rust —
/// `commands::resume::resume_validate_content` is where these constants live,
/// and they are HOISTED from there rather than re-declared, because two copies
/// of "50 requirements, 300 bytes each" is exactly how one of them drifts.
struct ClampedRequest {
    job_url: String,
    target_language: String,
    top_requirements: Vec<String>,
    cover_letter: String,
}

/// Byte cap on the request's `jobUrl` — mirrors the schema's `.max(2_048)`.
/// This value is a STORAGE key (the run row's retention partition and the
/// aggregate lookup), so an unbounded one writes an unbounded row.
const JOB_URL_CAP: usize = 2_048;

/// Clamp every renderer-supplied free-text field of a run request. Pure, so the
/// caps are a test rather than a claim.
fn clamp_request(req: &ResumePipelineRunRequest) -> ClampedRequest {
    use crate::applications::{clamp_to_bytes, MAX_JOB_DESCRIPTION_BYTES};
    use crate::commands::resume::{
        TARGET_LANGUAGE_CAP, TOP_REQUIREMENTS_CAP, TOP_REQUIREMENT_BYTES_CAP,
    };

    ClampedRequest {
        job_url: clamp_to_bytes(req.job_url.clone(), JOB_URL_CAP),
        target_language: clamp_to_bytes(req.target_language.clone(), TARGET_LANGUAGE_CAP),
        top_requirements: req
            .top_requirements
            .iter()
            .take(TOP_REQUIREMENTS_CAP)
            .map(|r| clamp_to_bytes(r.clone(), TOP_REQUIREMENT_BYTES_CAP))
            .collect(),
        cover_letter: clamp_to_bytes(req.cover_letter_text.clone(), MAX_JOB_DESCRIPTION_BYTES),
    }
}

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
    if !max::is_staged(depth) {
        return Err(AppError::Validation(format!(
            "the staged pipeline runs at quality or max depth; {} is not handled here",
            depth.as_str()
        )));
    }

    let clamped = clamp_request(req);
    let completer = Completer::from_active(app)?;
    // ONE resolution per overridden stage, BEFORE the run starts: an override
    // edited mid-run must not take effect halfway through the document, and the
    // stage cache keys are derived from these same completers. Scoped to the
    // stages THIS depth runs AND CAN PAY FOR — a `draft` override costs nothing
    // on a max run, which has no draft stage, and a stage that makes no call
    // has no routing to resolve (see `max::paying_stages`).
    let stage_completers = Completer::for_stages(app, &max::paying_stages(depth))?;
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
        clamped.job_url.clone()
    } else {
        meta.url.clone()
    };

    let span = crate::observability::Span::begin(
        "pipeline:resume",
        // The TIER, never the raw string: `effort` is renderer-supplied free
        // text and this line lands in the diagnostics bundle. Same treatment as
        // the `key=` below, which logs a parsed `SectionKey`.
        format!(
            "op=run depth={} effort={}",
            depth.as_str(),
            timeouts::effort_tier(req.effort.as_deref())
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
    // ONE clock for the whole run: the hook checks it at every stage boundary,
    // the `sections` stage checks it between its per-section calls, and the
    // `repair` stage checks it between its own (it is the last stage, so there
    // is no boundary after it — see `RunDeadline`).
    let deadline = RunDeadline::starting_now(max::deadline_for(depth, req.effort.as_deref()));
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
            target_language: &clamped.target_language,
            top_requirements: &clamped.top_requirements,
            cover_letter: &clamped.cover_letter,
            effort: req.effort.as_deref(),
            job_id,
        },
        &completer,
        cache.as_deref(),
        deadline,
        Arc::clone(&ledger),
    )
    .with_stage_completers(&stage_completers);
    if depth == GenerationDepth::Max {
        // The hook is ALSO the section-wise observer: per-section events and
        // the progressive document stream are emits, so they belong to the one
        // layer allowed to emit. `RunHooks` outlives the context, and both
        // borrows are shared.
        ctx = ctx.for_max(Some(&hooks));
    }

    let outcome = max::pipeline_for(depth).run_hooked(&mut ctx, &hooks).await;

    // ── THE DELETE WINS ──────────────────────────────────────────────────────
    //
    // This run wrote its own `running` row before the first stage. If that row
    // is GONE now, something deleted this posting's data while the run was in
    // flight — `applications_delete`, `ai_generations_remove`, a factory reset,
    // or a backup restore — and every one of those is the user saying "remove
    // this". Writing the terminal state anyway does not merely miss the delete:
    // `upsert_run` is INSERT OR REPLACE, so it RESURRECTS the run row, and
    // `persist_document`'s merge-upsert re-creates the `ai_generations`
    // aggregate the delete removed. The posting comes back — in the runs panel,
    // in the Documents list, and in the next backup — with a permanently
    // PARTIAL trail, because the events from before the purge are already gone.
    // Executed, not reasoned: see
    // `a_run_whose_posting_was_deleted_mid_flight_does_not_resurrect_it`.
    //
    // So the run abandons its own output. No persist, no row, and a sweep of
    // whatever it appended into the gap between the purge and here.
    //
    // A terminal check rather than cancel-and-await: nothing maps a `job_url`
    // to an in-flight run's cancel token (`RunRow` has no job id, and
    // `CancelRegistry` is keyed by a per-run uuid), so cancelling at the delete
    // site needs a new index — and even with one, the in-flight window between
    // the cancel and the run noticing still lands here. This closes both delete
    // doors at the single place both must pass through.
    if store.run(run_id).is_none() {
        let swept = store.delete_events_for_run(run_id);
        span.end_with(
            &format!("status=cancelled stopped=deleted swept={swept}"),
            false,
        );
        crate::commands::jobs::job_cancel(app, job_id);
        return Ok(());
    }

    // Persist whatever the run produced BEFORE deciding how it ended: a run
    // stopped at the repair stage still wrote a real document, and discarding
    // it because the report is not clean is the opposite of what the terminal
    // review is for.
    let quality_report = persist_document(app, &job_url, &meta, &clamped, &ctx, depth.as_str());
    // A REFUSED save is not a successful run. `is_persistable` rejects a
    // document that lost the source's whole work history, and `terminal_state`
    // would otherwise read `outcome == Ok` and report `completed` — a run the
    // user is told succeeded, over a document that never changed, with nothing
    // anywhere saying why. Only `Refused` counts: `Nothing` is the unlinked /
    // empty-draft case, which is benign and already reported by its own path.
    let refused =
        save_verdict(ctx.input.source_resume, &ctx.draft, &job_url) == SaveVerdict::Refused;
    // The same texts `persist_document` built the wrapper over — fresh entries
    // carry no decisions yet, so the document-agreement half of the rule is
    // vacuous here, but the signature keeps ONE definition of "unresolved".
    let needs_review = quality_report.as_deref().is_some_and(|wrapper| {
        report::still_needs_review(wrapper, &ctx.draft, &clamped.cover_letter)
    }) || ctx.critical_count() > 0;

    // Status and reason together — see `hooks::terminal_state` for why a
    // cancelled draft used to come out `failed` + `"done"`, and why a run whose
    // deadline expired at a stage boundary is not a failure once
    // `persist_document` has saved its document.
    let (status, stopped_reason) = hooks::terminal_state(
        &ledger,
        outcome.is_ok() && !refused,
        cancel.is_cancelled(),
        needs_review,
        quality_report.is_some(),
    );
    let cancelled = status == STATUS_CANCELLED;
    row.status = status.to_string();
    row.finished_at = Some(crate::db::now_ms());
    row.stopped_reason = stopped_reason;
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

    // ADR-016: the run is long enough that the user is normally elsewhere when
    // it lands, so the terminal state goes to the Notification Center (plus an
    // OS banner while the window is unfocused). Counts only — the honest
    // "N claims still need a verdict" comes from the SAME `unresolved_count`
    // the review panel reads, never from a second definition.
    notify::notify_terminal(
        app,
        status,
        quality_report.as_deref().map_or(0, |wrapper| {
            report::unresolved_count(wrapper, &ctx.draft, &clamped.cover_letter)
        }),
        &meta,
    );

    match outcome {
        // The pipeline finished, and the document it produced was refused. The
        // row already says `failed`; the JOB has to agree, and the user needs
        // the reason — the alternative is a green run over an unchanged
        // document. `execute`'s caller turns this into `job_fail`.
        Ok(()) if refused => Err(AppError::Message(
            "The generated résumé came back without any of your work history, so your saved \
             document was left unchanged. Try again, or use quality depth."
                .to_string(),
        )),
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
        // The run row and the JOB must agree. `terminal_state` resolves a
        // deadline that expired at a stage boundary AFTER the document was saved
        // to `needsReview`/`completed` — the same outcome the in-loop check
        // produces — so failing the job here would tell the renderer to discard
        // a document its own run row calls reviewable.
        Err(_) if status != STATUS_FAILED => {
            crate::commands::jobs::job_complete(
                app,
                job_id,
                json!({ "runId": run_id, "status": status, "text": ctx.draft }),
            );
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
///
/// **An UNLINKED run (no resolvable `jobUrl`) saves nothing.** The aggregate is
/// keyed by posting url, so a row with an empty one is unreachable by every
/// reader in the app (`find_for_job` looks up a url; `applied_job_urls` filters
/// them out) and unreachable by the retention prune, which partitions on
/// `(job_url, kind)` — a permanent, invisible row holding a full résumé. The
/// plan calls an unlinked generation session-only, and the run row + the stream
/// the user watched are that session.
///
/// ## A STOPPED run overwrites the saved document, and what that trades
///
/// This save is an overwrite: there is one `ai_generations` row per posting and
/// no versioning, so whatever a run persists REPLACES what the user had. Since
/// a deadline-stopped fan-out now keeps its sections (`hooks::apply_stop` lets
/// `assemble` and `validate` run past the clock), that includes PARTIAL
/// documents — which is deliberate, and worth stating rather than discovering:
///
/// * **The kept trade.** A run stopped near the end loses tail sections
///   (Education before Projects before an employment entry — `plan_sections`
///   truncates from the tail for exactly this reason). A missing employer is a
///   visible `factual.dropped_role` Critical, the run lands `needsReview`, and
///   the user is looking at a document they can see is short. That beats
///   throwing away eleven paid sections, which is the whole argument that sized
///   `Budget::RESUME_MAX`'s deadline at the reachable number.
/// * **The refused trade** ([`is_persistable`]). A document that lost ALL of a
///   work history the SOURCE has is not a short résumé, it is not a résumé —
///   and overwriting a good previous document with it is a loss the review
///   panel cannot describe and the user cannot undo. Nothing is saved, and the
///   previous document survives. Source-RELATIVE on purpose: a candidate whose
///   own résumé has no employment section is a real input, not a failure.
fn persist_document(
    app: &AppHandle,
    job_url: &str,
    meta: &crate::commands::match_resume::JobPostingMeta,
    clamped: &ClampedRequest,
    ctx: &QualityCtx<'_>,
    depth: &str,
) -> Option<String> {
    let report = ctx.report.as_ref()?;
    if save_verdict(ctx.input.source_resume, &ctx.draft, job_url) != SaveVerdict::Save {
        return None;
    }
    let wrapper = report::build(
        depth,
        crate::db::now_ms(),
        Some((report, &ctx.draft)),
        ctx.letter_report
            .as_ref()
            .map(|letter| (letter, clamped.cover_letter.as_str())),
    );
    let store = app.try_state::<AiGenerationStore>()?;
    let record = AiGenerationRecord {
        id: make_generation_id(),
        created_at: crate::db::now_ms(),
        target_language: clamped.target_language.clone(),
        top_requirements: clamped.top_requirements.clone(),
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

/// What [`persist_document`] will do with this run's document, decided from the
/// two documents and the posting url alone.
///
/// Three outcomes rather than a bool, because two of them mean opposite things
/// to the RUN: `Nothing` is benign (an unlinked run is session-only by design,
/// an empty draft is a run that failed before it wrote anything and is already
/// reported as such), while `Refused` means the run produced a document and it
/// was rejected — which must not come out as a successful completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveVerdict {
    Save,
    /// There was nothing to save, or nowhere to save it.
    Nothing,
    /// There WAS a document, and [`is_persistable`] rejected it.
    Refused,
}

/// One definition of "will this save", shared by `persist_document` (which
/// acts on it) and `execute` (which has to report it).
pub(crate) fn save_verdict(source_resume: &str, draft: &str, job_url: &str) -> SaveVerdict {
    if draft.trim().is_empty() || job_url.trim().is_empty() {
        return SaveVerdict::Nothing;
    }
    if is_persistable(source_resume, draft) {
        SaveVerdict::Save
    } else {
        SaveVerdict::Refused
    }
}

/// Whether a run's document may OVERWRITE the posting's saved one.
///
/// One rule, and it is RELATIVE TO THE SOURCE: **refuse when the candidate's
/// own résumé has an employment section and the generated document does not.**
/// Everything else about completeness is a judgement the report already makes
/// visible (a missing employer is a `factual.dropped_role` Critical and
/// `needsReview`), but a document that lost ALL of a real work history is not a
/// shorter résumé — it is not one — and this save has no versioning to undo it
/// with.
///
/// **Why the source and not the run's outcome.** Two earlier versions of this
/// gate read the run instead of the documents, and each was wrong in its own
/// direction:
///
/// * keying on the recorded stop REASON refused a perfectly good run whose
///   source simply has no employment section (a new graduate, an academic CV)
///   the moment its repair round hit the daily cap — `stages::repair` records
///   `Budgeted`/`RunTimeout` for a stop it RECOVERED from and then returns
///   `Ok(())`. Status `completed`, document unchanged, no explanation anywhere;
/// * keying on the run's OUTCOME missed the case the gate exists for. The
///   sections fan-out treats a daily-cap refusal as `StoppedReason::Budgeted`,
///   breaks, and returns `Ok(())` — and nothing downstream converts that into
///   an `Err` (`apply_stop` errors only on the clock or a cancel, `assemble`
///   and `validate` are free, `repair` returns `Ok` on `RateLimited`, the judge
///   is skippable, and `run_hooked` never consults the ledger). So a max run
///   that hit the cap right after Summary produced a summary-only document with
///   `outcome == Ok`, and it overwrote the saved résumé.
///
/// Comparing the two DOCUMENTS answers both at once and needs no run state:
/// a source with no work history can never trip it, and a truncated fan-out
/// over a real one always does — however the run happened to end.
///
/// Both sides go through the SAME [`sections::find`] seam, so the
/// undated-entry caveat (`assemble::DATE_COLUMN_GAP`: an entry with no date
/// column is not a `LineKind::JobEntry`) applies equally to each and cannot
/// create a false asymmetry.
pub(crate) fn is_persistable(source_resume: &str, draft: &str) -> bool {
    has_work_history(draft) || !has_work_history(source_resume)
}

/// Whether `text` has an employment section with anything under it.
///
/// The SECTION with a body, not [`sections::entry_range`]: that one tests for
/// `LineKind::JobEntry`, which an entry with no date column legitimately fails,
/// and a résumé whose dates the source never carried is a real document rather
/// than an empty one.
fn has_work_history(text: &str) -> bool {
    let split = crate::pipeline::resume::stages::sections::split(text);
    let lines: Vec<&str> = text.lines().collect();
    crate::pipeline::resume::stages::sections::find(&split, SectionKey::Experience(0)).is_some_and(
        |section| {
            section
                .text(&lines)
                .lines()
                .skip(1)
                .any(|line| !line.trim().is_empty())
        },
    )
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

/// One run with its stage trail, plus the posting's CURRENT document and report.
/// `null` for an unknown id.
///
/// The two halves have different owners and different lifetimes — see the module
/// doc. `status`/`stoppedReason`/`metrics`/`events` are this run's own and never
/// change again; `resumeText`/`report` are the live `ai_generations` aggregate,
/// so they may be newer than the run (a later run, a section regeneration, a
/// re-check, or the user's own edit). A `report` whose `sourceTextHash` no longer
/// matches `resumeText` is stale BY DESIGN, not corrupt: it is the verdict on an
/// earlier version of the same document.
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

/// ONE persisted stage artifact, as the WIRE carries it: the counts, never the
/// detail.
///
/// The server keeps the full `strategy`/`match_evidence` artifacts a max-depth
/// per-entry regenerate needs (`RunLedger::record_detail`), and it keeps them
/// SERVER-SIDE. `record_detail`'s doc says "nowhere else"; without this it was
/// wrong — the trail is handed to the renderer verbatim, so every `get` of a max
/// run shipped up to 2 × 16 KiB of employment history and verbatim résumé quotes
/// over IPC to a reader that has never had a consumer for it (the contract types
/// `artifact` as `unknown`; the only renderer reference is a `{}` test fixture).
///
/// **An unparseable artifact becomes a content-free MARKER, not the raw
/// string.** Returning the raw bytes was the right answer while artifacts were
/// counts-only — a reader must not see a silent `{}` claiming the stage
/// reported nothing. But the only artifact large enough for the store's clamp
/// to truncate is a detail-bearing one, so the raw-string arm WAS the leak,
/// with the truncation marker on the end. The marker keeps the one thing the
/// reader needed (this artifact did not survive intact) and carries nothing
/// else.
fn wire_artifact(artifact_json: &str) -> Value {
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

// ── Write surface ────────────────────────────────────────────────────────────

/// The DOCUMENT a run's write commands act on belongs to the NEWEST run of that
/// posting, not to the run whose id was passed.
///
/// `pipeline_runs` keeps one row per run, but every run of a posting merges into
/// the SAME `ai_generations` aggregate (see the module doc), so `listForJob`
/// legitimately advertises three runs while `find_for_job` can only ever return
/// one document — the newest run's. A `regenerateSection` against an older run
/// id would therefore rewrite the NEWEST document and hand back a detail whose
/// `resumeText` was never that run's; a `resolveFabrication` would record a
/// verdict against findings from a report the run never produced.
///
/// Refusing is the honest answer, and the smallest one: making it work needs a
/// per-run document, which is a schema change and a second copy of every
/// résumé. Read-only `get` still returns the older run's own row — its status,
/// metrics and stage trail are genuinely its — with the aggregate document
/// alongside; the contract's doc comment says so.
///
/// An UNLINKED run (empty `job_url`) is exempt: it has no aggregate at all, so
/// the "no saved résumé" error below is the accurate one.
///
/// **The rule is about newer RUNS, not about newer TEXT** — deliberately, and
/// the message is worded that way. The user editing the document (applying a
/// "Remove", or any hand edit) moves `ai_generations.resume_text` while this
/// stays the newest run, so `resolveFabrication` and `regenerateSection` keep
/// working on their own posting; the report simply reads stale until a re-check.
/// Refusing an edited-but-latest run would strand the exact review the panel is
/// asking the user to finish.
fn ensure_latest_run(store: &PipelineRunStore, row: &RunRow) -> AppResult<()> {
    if row.job_url.trim().is_empty() {
        return Ok(());
    }
    let newest = store
        .runs_for_job(&row.job_url)
        .into_iter()
        .find(|candidate| candidate.kind == row.kind);
    match newest {
        // Deliberately does NOT say "open the latest run": the newest run may
        // still be running, or may have failed before it saved anything, so
        // pointing the user at a document that does not exist yet is the second
        // wrong answer after the edit itself.
        Some(newest) if newest.id != row.id => Err(AppError::Validation(
            "There is a newer run for this posting, and all of its runs share one saved \
             résumé — only the newest one can change it. If that run is still going, wait \
             for it to finish."
                .to_string(),
        )),
        _ => Ok(()),
    }
}

/// Re-generate ONE section of a finished run and splice it back.
///
/// **`"header"` is rejected here, at the boundary**, and not by a special case:
/// [`SectionKey::from_wire`] runs the generated `is_pipeline_section_key`
/// grammar, which has no header token — so the contact header the editor owns
/// at export time (ADR-0021) is unreachable from this command by construction,
/// along with every other invented section name.
///
/// **Admission-limited like every other provider-calling command.** It makes a
/// full completion (plus a re-validation) per click, and
/// `PROVIDER_DAILY_MAX` is a per-DAY total, not a rate — so an unadmitted
/// button is a renderer loop that can burn a day's ceiling in seconds. It takes
/// the same `agent_run` bucket as the run itself, and `acquire` (not
/// `acquire_queued`): this is a click, and a click that has to wait behind a
/// 45-minute run should be refused with a retriable error, not parked.
#[tauri::command]
pub async fn resume_pipeline_regenerate_section(
    app: AppHandle,
    req: ResumePipelineRegenerateSectionRequest,
) -> AppResult<Value> {
    let _guard = app.state::<Arc<crate::limits::Limiter>>().inner().acquire(
        "agent_run",
        crate::limits::AGENT_RUN_RATE_MAX,
        crate::limits::AGENT_RUN_CONCURRENCY_MAX,
    )?;

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
    ensure_latest_run(&store, &row)?;
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
    let source = source_resume_for(&app, &row, &record);
    // Routed through the SAME per-stage path a run takes — and through the
    // stage whose PROMPT each sub-path actually uses, resolved lazily so a
    // click only ever resolves the one it takes:
    //
    // * the max artifact-rebuild path re-runs `sections`' own generator over
    //   the run's stored analysis/strategy, so it follows the `sections`
    //   override — the stage that produced the text being replaced;
    // * the fallback is a whole-section REWRITE using `repair`'s prompt and
    //   grounding, and `repair` is the stage that exists at BOTH depths (a
    //   quality run has no `sections` stage at all), so it follows `repair`.
    //
    // Routing both through one override would make the button disagree with
    // whichever stage the user actually configured.
    let outcome = match regenerate_max_entry(
        &store,
        &row,
        &record,
        &Completer::from_active_for_stage(&app, section_gen::NAME)?,
        &source,
        key,
        &req,
    )
    .await
    {
        Some(outcome) => outcome?,
        // Not a max run, not an employment entry, or the run's artifacts are
        // gone/unparseable — the whole-section rewrite that has always been
        // here. A degraded click is a worse click; a failed one is a bug.
        None => {
            regenerate_one_section(
                &Completer::from_active_for_stage(&app, stages::REPAIR_STAGE)?,
                &source,
                &record.target_language,
                &record.resume_text,
                key,
                // No validator issues on this path: the user, not a report,
                // asked for the change. The note carries the "why", fenced.
                &[],
                req.note.as_deref(),
            )
            .await?
        }
    };
    let spliced = match outcome {
        SectionOutcome::Replaced(spliced) => spliced,
        SectionOutcome::Unusable => {
            span.end(false);
            return Err(AppError::Provider(
                "The model's replacement section came back empty or truncated, so nothing was \
                 changed. Try again."
                    .to_string(),
            ));
        }
        // No provider call was made — say so, rather than blaming the model for
        // a section this document does not have.
        SectionOutcome::Missing => {
            span.end(false);
            return Err(AppError::Validation(format!(
                "this résumé has no {} section to regenerate",
                key.to_wire()
            )));
        }
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
    let needs_review = report::still_needs_review(&wrapper, &spliced, &record.cover_letter_text);
    // ONE write, not two: the merge rule above says the text and its report
    // move together, and two statements leave a window where a crash (or a
    // failing second statement) persists a document with the PREVIOUS
    // document's report — the exact state the rule exists to make impossible.
    generations.update_text_and_report(&record.id, spliced, wrapper)?;
    span.end_with(
        &format!(
            "issues={} blocking={}",
            report.issues.len(),
            report::has_criticals(&report)
        ),
        true,
    );

    // The mirror of `resolve_fabrication`'s clearing arm: a regenerated
    // section can INTRODUCE a fresh finding on a run whose row still says
    // `completed`, and the panel keys its headline (and whether the review
    // block renders at all) on that row — so leaving it untouched would
    // suppress the very review the regeneration just created.
    if let Some(next) = recomputed_status(&row.status, needs_review) {
        let mut row = row.clone();
        row.status = next.to_string();
        store.upsert_run(&row)?;
        return Ok(detail(&app, &row));
    }
    Ok(detail(&app, &row))
}

/// The MAX-depth per-entry regenerate, when this click qualifies for it.
///
/// `None` means "not this path" — and every one of the four reasons is a normal
/// state, not a failure:
///
/// * the run was quality depth (its document was drafted whole, so an entry is
///   not independently regenerable — that is exactly what `sections::find`'s
///   Experience-collapse says);
/// * the key is not an employment entry (every other section IS its own
///   section, and the whole-section rewrite addresses it correctly);
/// * the run's persisted artifacts are missing or unparseable — a run older
///   than the detail, or one whose artifact the store's clamp truncated;
/// * the document no longer has that entry.
///
/// The caller degrades to the whole-section path in all four cases, which is
/// why this returns `Option<AppResult<_>>` rather than folding the miss into an
/// error: a click must not fail because an optimization was unavailable.
#[allow(clippy::too_many_arguments)]
async fn regenerate_max_entry(
    store: &PipelineRunStore,
    row: &RunRow,
    record: &AiGenerationRecord,
    completer: &Completer,
    source: &str,
    key: SectionKey,
    req: &ResumePipelineRegenerateSectionRequest,
) -> Option<AppResult<SectionOutcome>> {
    if row.depth != GenerationDepth::Max.as_str() {
        return None;
    }
    let SectionKey::Experience(index) = key else {
        return None;
    };
    let artifacts = max::artifacts_for(store, &row.id)?;
    let outcome = max::regenerate_entry(
        completer,
        &artifacts,
        source,
        // The aggregate's own copy of the posting, which is often EMPTY on this
        // path: the pipeline's save deliberately writes no `job_ad` (the
        // postings cache is keyed by the live job id, which a finished run no
        // longer holds), so this is only populated when a fast-path generation
        // stored one first. Empty is a degradation, not a failure — the posting
        // reaches the prompt through the persisted `job_analysis` and
        // `evidence_map` either way, and the FACTS the entry is rebuilt from
        // are the source résumé's, which is always present. The one thing it
        // costs is `extract_evidence`'s keyword scoring, which orders bullets
        // it does not select.
        &record.job_ad,
        &record.target_language,
        &record.resume_text,
        index,
        req.note.as_deref(),
    )
    .await;
    match outcome {
        // The entry is not in the document (or not in the roster): fall back
        // rather than telling the user their résumé has no such section.
        Ok(SectionOutcome::Missing) => None,
        other => Some(other),
    }
}

/// The status a run row should move to after its persisted wrapper changed —
/// `None` when it should not move at all.
///
/// Only the two REVIEW-terminal states convert into each other: a wrapper
/// write can un-clean a `completed` run (a regenerated section introducing a
/// fresh fabrication) and can finish a `needsReview` one (the last verdict
/// landing). `failed` and `cancelled` describe how the RUN ended, which no
/// amount of report movement rewrites — and `running` is not terminal.
fn recomputed_status(current: &str, needs_review: bool) -> Option<&'static str> {
    let next = if needs_review {
        STATUS_NEEDS_REVIEW
    } else {
        STATUS_COMPLETED
    };
    (matches!(current, STATUS_COMPLETED | STATUS_NEEDS_REVIEW) && current != next).then_some(next)
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
    // Same aggregate, same rule (see `ensure_latest_run`): a verdict recorded
    // against an older run would land in the NEWEST run's report.
    ensure_latest_run(&store, &row)?;
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
        // every flagged bullet decided — with the document AGREEING with every
        // Remove (a recorded-but-unapplied removal is intent, not fact; see
        // `report::entry_resolved`) — and no Critical the review cannot clear
        // (`factual.dropped_role` names an absence, so it is not in the panel —
        // and a run that flipped to `completed` because every *reviewable*
        // finding was decided would present a résumé that silently lost an
        // employer as clean). `record.resume_text` is current: the live panel
        // applies the removal edit BEFORE recording the verdict.
        let needs_review =
            report::still_needs_review(&updated, &record.resume_text, &record.cover_letter_text);
        generations.update_quality_report(&record.id, updated)?;
        if let Some(next) = recomputed_status(&row.status, needs_review) {
            let mut row = row.clone();
            row.status = next.to_string();
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
