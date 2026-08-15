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
//! * **The inputs, two ways in per side, ID WINS.** The résumé and the posting
//!   each resolve from EITHER a server-side lookup (`DocumentStore` by
//!   `resumeId`, the live postings cache by `jobId`) OR renderer-supplied text
//!   (`resumeText`/`jobAdText`) — added so a pasted job ad or an Autopilot
//!   found job, neither of which has a cache id or a stored résumé id, can
//!   still start a staged run. [`resume_source`]/[`job_source`] are the pure
//!   decision: a nonempty id ALWAYS wins over its matching text field (a run
//!   that sends both never silently prefers the text), and a miss on the id
//!   path is a hard error — it never falls back to the text field. Renderer
//!   text still reaches a prompt ONLY through the existing `fenced(...)` paths
//!   (ADR-010); this does not add a second way in for untrusted text.
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
mod resolve;

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
use crate::pipeline::resume::stages::{self, regenerate_one_section, SectionOutcome};
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::resume::{
    projects, quality_pipeline, QualityCtx, QualityInput, RunDeadline, RunLedger,
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

/// The `pipeline_runs.depth` value every NEW run writes. The column itself
/// stays (existing rows carry `"fast"`/`"quality"`/`"max"`, and the renderer
/// still needs the historic vocabulary to render an old run's label) — this
/// is the one value a run started by this build can produce, now that `fast`
/// never reached this pipeline and `max` was removed. No migration touches an
/// existing row.
const RUN_DEPTH: &str = "quality";

const STATUS_RUNNING: &str = "running";
pub(crate) const STATUS_COMPLETED: &str = "completed";
pub(crate) const STATUS_NEEDS_REVIEW: &str = "needsReview";
pub(crate) const STATUS_FAILED: &str = "failed";
pub(crate) const STATUS_CANCELLED: &str = "cancelled";

// The clamp + the ID-WINS résumé/job-ad resolution decision live in
// `resolve` — split out to stay under R8's per-module LOC cap
// (`docs/architecture-rules.md`). Re-imported below so `execute`/
// `persist_document` read exactly as they did inline.
use self::resolve::{
    clamp_request, job_ad_for_persist, job_meta_from_request, job_source, resume_source,
    ClampedRequest,
};

/// Start one staged résumé run. Returns `{ runId, jobId }` immediately; stage
/// progress streams as `pipeline:stage` and the draft's deltas as `ai:stream`
/// under the same `jobId`.
///
/// Every fail-able step runs INSIDE the spawned task, for the same reason the
/// now-deleted `commands::agent::agent_run` used to document at length: the
/// returned `jobId` is the renderer's only handle, so a terminal event
/// emitted before this returns is silently dropped and the run looks stuck at
/// pending forever.
#[tauri::command]
pub async fn resume_pipeline_run(app: AppHandle, req: ResumePipelineRunRequest) -> Value {
    let job_id = new_job_id();
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    crate::commands::jobs::job_start(&app, &job_id, "resumePipeline.run");

    let cancel = CancellationToken::new();
    let cancels = app.state::<Arc<CancelRegistry>>().inner().clone();
    // Registered BEFORE the spawn (mirrors `scrape_boards`) so a
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
                fail(&app_task, &cancels, &job_id_task, e.to_string(), None).await;
                return;
            }
        };

        if let Err(failure) = execute(&app_task, &run_id_task, &job_id_task, &req, &cancel).await {
            fail(
                &app_task,
                &cancels,
                &job_id_task,
                failure.error.to_string(),
                failure.data,
            )
            .await;
            return;
        }
        cancels.unregister(&job_id_task).await;
    });

    json!({ "runId": run_id, "jobId": job_id })
}

/// The failure [`execute`] hands its caller: a typed message plus OPTIONAL
/// structured data for `job.failed`'s event payload — for the one case a
/// caller can say more than the message text alone (currently just the
/// per-call timeout: WHICH stage, and how long — see
/// `hooks::timeout_failure_data`). `From<AppError>` covers every ordinary `?`
/// inside `execute` with `data: None`, so only the arm that actually has more
/// to say constructs this by hand.
struct ExecuteFailure {
    error: AppError,
    data: Option<Value>,
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
/// failure keeps riding as the plain message string it always has.
async fn fail(
    app: &AppHandle,
    cancels: &CancelRegistry,
    job_id: &str,
    message: String,
    data: Option<Value>,
) {
    match data {
        Some(data) => crate::commands::jobs::job_fail_with_data(app, job_id, message, data),
        None => crate::commands::jobs::job_fail(app, job_id, message),
    }
    cancels.unregister(job_id).await;
}

/// The run itself, from resolution through persistence.
///
/// Split out of the spawn so every failure is ONE `?` and the task body stays
/// readable — and so the ordering (resolve → record `running` → run → record
/// terminal) is visible in one place. Returns [`ExecuteFailure`], not a bare
/// `AppError`, so the ONE arm that has structured `job.failed` data to give
/// (the per-call timeout) can hand it to the caller alongside the message.
async fn execute(
    app: &AppHandle,
    run_id: &str,
    job_id: &str,
    req: &ResumePipelineRunRequest,
    cancel: &CancellationToken,
) -> Result<(), ExecuteFailure> {
    let clamped = clamp_request(req);
    let completer = Completer::from_active(app)?;
    // ONE resolution per overridden stage, BEFORE the run starts: an override
    // edited mid-run must not take effect halfway through the document, and the
    // stage cache keys are derived from these same completers. Scoped to the
    // stages the pipeline actually CAN PAY FOR — a stage that makes no call has
    // no routing to resolve (see `max::paying_stages`).
    let stage_completers = Completer::for_stages(app, &max::paying_stages())?;

    // ID WINS, no silent fallback: a nonempty `resumeId`/`jobId` is looked up
    // and a miss is a hard error — `resumeText`/`jobAdText` are never
    // consulted on that path, even when the request carries both. Resolved
    // from the CLAMPED ids (never `req.resume_id`/`req.job_id` directly):
    // both reach a renderer-visible error message on a miss and the run's
    // `metrics_json` on a hit, so an unbounded copy would let a hostile
    // direct-IPC caller grow either without limit. See the module doc and
    // `resolve::{resume_source, job_source, resolve_resume, resolve_job}` —
    // the decision AND its behavior are pure there, provable without an
    // `AppHandle`; only the actual store/cache reads stay here.
    let resume_choice =
        resume_source(&clamped.resume_id, &clamped.resume_text).ok_or_else(|| {
            AppError::Validation("either a résumé id or résumé text is required".to_string())
        })?;
    let resume_text = resolve::resolve_resume(resume_choice, |id| {
        app.state::<DocumentStore>()
            .get(id)
            .map(|record| record.text)
    })?;

    let job_choice = job_source(&clamped.job_id, &clamped.job_ad_text).ok_or_else(|| {
        AppError::Validation("either a job id or job ad text is required".to_string())
    })?;
    let (job_ad, meta) = resolve::resolve_job(
        job_choice,
        |id| crate::commands::match_resume::job_text_for(app, id),
        |id| crate::commands::match_resume::job_meta_for(app, id),
        || job_meta_from_request(&clamped),
    )?;
    // The posting's OWN url wins over the request's: it was resolved
    // server-side from the cache, and it is the AGGREGATE's key
    // (`AiGenerationRecord.job_url` — see `resolve::job_ad_for_persist`'s doc
    // for the trust asymmetry between the two paths). On the text path
    // `meta.url` IS the request's `jobUrl` (`job_meta_from_request`), so this
    // still resolves to the same value.
    let job_url = if meta.url.trim().is_empty() {
        clamped.job_url.clone()
    } else {
        meta.url.clone()
    };
    // The run-STORE's OWN key — see `resolve::run_store_job_url`'s doc for
    // why an unlinked text-path run does not simply reuse `job_url` here.
    let run_job_url = resolve::run_store_job_url(&job_url, job_choice);

    let span = crate::observability::Span::begin(
        "pipeline:resume",
        // The TIER, never the raw string: `effort` is renderer-supplied free
        // text and this line lands in the diagnostics bundle. Same treatment as
        // the `key=` below, which logs a parsed `SectionKey`.
        format!(
            "op=run effort={}",
            timeouts::effort_tier(req.effort.as_deref())
        ),
    );

    let store = app.state::<PipelineRunStore>();
    let started_at = crate::db::now_ms();
    let mut row = RunRow {
        id: run_id.to_string(),
        job_url: run_job_url,
        kind: RUN_KIND.to_string(),
        depth: RUN_DEPTH.to_string(),
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
    // and `repair` checks it again between its own calls — a single round can
    // make several section round-trips, and a boundary check alone would not
    // interrupt one mid-round. See `RunDeadline`.
    let deadline = RunDeadline::starting_now(max::deadline_for(req.effort.as_deref()));
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
            source_resume: &resume_text,
            job_ad: &job_ad,
            target_language: &clamped.target_language,
            top_requirements: &clamped.top_requirements,
            cover_letter: &clamped.cover_letter,
            include_cover_letter: req.include_cover_letter,
            effort: req.effort.as_deref(),
            job_id,
        },
        &completer,
        cache.as_deref(),
        deadline,
        Arc::clone(&ledger),
    )
    .with_stage_completers(&stage_completers);

    let outcome = quality_pipeline().run_hooked(&mut ctx, &hooks).await;

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
    let quality_report = persist_document(
        app,
        &job_url,
        &meta,
        &clamped,
        &job_ad_for_persist(job_choice),
        &ctx,
        RUN_DEPTH,
    );
    // A REFUSED save is not a successful run. `is_persistable` rejects a
    // document that lost the source's whole work history, and `terminal_state`
    // would otherwise read `outcome == Ok` and report `completed` — a run the
    // user is told succeeded, over a document that never changed, with nothing
    // anywhere saying why. Only `Refused` counts: `Nothing` is the unlinked /
    // empty-draft case, which is benign and already reported by its own path.
    // Gated on `ctx.letter` — the stage's OWN output, the only letter text
    // this run can actually persist — never `ctx.letter_text()`'s fallback to
    // `input.cover_letter` (the user's own pasted reference letter, read when
    // `include_cover_letter = false`). That fallback is real user text this
    // run never generates and `persist_document` never stores, so gating on
    // it made a fence tag INCIDENTAL to a pasted reference letter refuse a
    // save that had nothing wrong with it — discarding a whole run's résumé
    // and report over text that was never going to be written anywhere. See
    // `persist_document`'s identical gate below for the save decision itself;
    // this one only has to AGREE with it for `refused`/`terminal_state` to be
    // consistent with what was actually written.
    let verdict = save_verdict(ctx.input.source_resume, &ctx.draft, &ctx.letter, &job_url);
    let refused = matches!(verdict, SaveVerdict::Refused(_));
    // The same texts `persist_document` built the wrapper over — fresh entries
    // carry no decisions yet, so the document-agreement half of the rule is
    // vacuous here, but the signature keeps ONE definition of "unresolved".
    let needs_review = quality_report
        .as_deref()
        .is_some_and(|wrapper| report::still_needs_review(wrapper, &ctx.draft, ctx.letter_text()))
        || ctx.critical_count() > 0;

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
        //
        // ONLY on the `Store` path: `resumeText` never lived in the
        // `DocumentStore`, so there is no id to carry, and writing an EMPTY
        // one would be indistinguishable from a real (if deleted) id to
        // `source_resume_for`'s `sourceResumeId` read. Leaving the key out
        // entirely reads back as `None`, which sends `source_resume_for` down
        // its documented weaker fallback (measure against the run's own
        // output) — and that fallback is exactly why
        // `normalize_regenerated_projects` refuses to write from it
        // (`source_is_provenanced`, PR-1): a text-path run's later
        // `regenerateSection` re-validates fine but does not re-normalize
        // Projects from an untracked source.
        if let Some(id) = resolve::source_resume_id_for_metrics(resume_choice) {
            object.insert("sourceResumeId".to_string(), json!(id));
        }
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
            report::unresolved_count(wrapper, &ctx.draft, ctx.letter_text())
        }),
        &meta,
    );

    match outcome {
        // The pipeline finished; whether that is a success depends on the SAME
        // `verdict` `refused` (above) was computed from — matched here
        // exhaustively, once, rather than re-derived from a bool guard, so a
        // future `SaveVerdict` variant is a compile error in this match, never
        // a runtime `unreachable!()` on a spawned run task with no terminal
        // event to show for it.
        Ok(()) => match verdict {
            // The document the pipeline produced was refused. The row already
            // says `failed`; the JOB has to agree, and the user needs the
            // reason — the alternative is a green run over an unchanged
            // document. `execute`'s caller turns this into `job_fail`.
            SaveVerdict::Refused(reason) => Err(AppError::Message(reason.to_string()).into()),
            SaveVerdict::Save | SaveVerdict::Nothing => {
                crate::commands::jobs::job_complete(
                    app,
                    job_id,
                    json!({ "runId": run_id, "status": status, "text": ctx.draft }),
                );
                Ok(())
            }
        },
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
        // A per-call deadline failure: the propagated stage error already
        // carries a reasonable message (see each provider's `complete_impl`),
        // but `hooks::apply_timeout` recorded exactly which STAGE and how
        // long — strictly more than the provider layer alone can know — so
        // `job_fail` gets the actionable version instead of the generic one,
        // PLUS `timeout_failure_data` for the renderer to localize instead of
        // splicing the raw stage key into an un-translatable English sentence.
        Err(e) => Err(match ledger.timeout_detail() {
            Some((stage, ms)) => ExecuteFailure {
                error: AppError::Timeout(hooks::timeout_message(stage, ms)),
                data: Some(hooks::timeout_failure_data(stage, ms)),
            },
            None => e.into(),
        }),
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
/// a deadline-stopped run keeps whatever it already produced (a `repair` round
/// that ran out of time mid-fan-out still KEEPS its accumulated corrections and
/// returns `Ok`, and `hooks::apply_stop` lets the free `validate` boundary run
/// past a clock that expired during the preceding paid stage), that includes
/// PARTIAL documents — which is deliberate, and worth stating rather than
/// discovering:
///
/// * **The kept trade.** A run stopped near the end still has a real, checked
///   draft — a missing correction is a visible Critical the report already
///   describes, the run lands `needsReview`, and the user is looking at a
///   document they can see is unfinished. That beats discarding everything the
///   run had already paid for.
/// * **The refused trade** ([`is_persistable`]). A document that lost ALL of a
///   work history the SOURCE has is not a short résumé, it is not a résumé —
///   and overwriting a good previous document with it is a loss the review
///   panel cannot describe and the user cannot undo. Nothing is saved, and the
///   previous document survives. Source-RELATIVE on purpose: a candidate whose
///   own résumé has no employment section is a real input, not a failure.
///
/// `job_ad_for_persist` is [`job_ad_for_persist`]'s output: empty on the
/// `Cache` path (unchanged — see that fn's doc for why), the request's own
/// job-ad text on the `Text` path. `merge_application`'s `pick` keeps the
/// existing aggregate value whenever the incoming one is empty, so the
/// `Cache` path's empty string never erases a `job_ad` an earlier save wrote.
///
/// **Establishes the Application FK, the same way [`ai_generations_save`]
/// does — this writer used to skip that step entirely.** ADR 0001 demoted
/// the generation to a child Document of an Application, but
/// `ai_generations::merge_application`'s `application_id:
/// incoming.application_id.or(existing.application_id)` only ever PRESERVES
/// an id that already arrived on the row; nothing in this pipeline ever
/// SET one, so the first staged-pipeline save for a posting with no prior
/// linked generation persisted with `application_id` permanently `NULL` —
/// invisible to every reader that joins/filters `ai_generations` by
/// `applicationId`. The résumé masked it: it has its own run-scoped read
/// path (`PipelineRunDetail.resumeText`, joined by run id, no FK involved),
/// but the cover letter has no such channel and is unreachable for that
/// whole class of application.
///
/// **Read-only lookup, never create-on-miss** — the same posture as
/// [`crate::applications::ApplicationStore::link_orphaned_generations`] (own
/// doc), for the same reason: a staged run is always launched FROM an
/// existing Application's page, so it's already there by the time this runs.
/// A run takes minutes; if the user deletes that Application while it's in
/// flight, an upsert would silently resurrect it the moment the run lands.
/// A miss here just leaves `application_id: None` — the row stays findable
/// by `AiGenerationStore::find_for_job` (keyed on `job_url`, not the FK),
/// only Application-scoped readers miss it, exactly like the error path
/// below.
///
/// [`ai_generations_save`]: crate::commands::ai_generations::ai_generations_save
fn persist_document(
    app: &AppHandle,
    job_url: &str,
    meta: &crate::commands::match_resume::JobPostingMeta,
    clamped: &ClampedRequest,
    job_ad_for_persist: &str,
    ctx: &QualityCtx<'_>,
    depth: &str,
) -> Option<String> {
    let report = ctx.report.as_ref()?;
    // The gate gates `ctx.letter` — what `cover_letter_text` below actually
    // stores — NOT `ctx.letter_text()`'s fallback to the user's own pasted
    // reference letter (see the identical comment on `execute`'s own
    // `save_verdict` call, which this one must agree with). `letter_text` is
    // still what the wrapper below is built over: the report legitimately
    // describes whichever text `report::build` was given, stage output or
    // fallback, and that's what `letter_text()` is for.
    let letter_text = ctx.letter_text();
    if save_verdict(ctx.input.source_resume, &ctx.draft, &ctx.letter, job_url) != SaveVerdict::Save
    {
        return None;
    }
    let wrapper = report::build(
        depth,
        crate::db::now_ms(),
        Some((report, &ctx.draft)),
        ctx.letter_report
            .as_ref()
            .map(|letter| (letter, letter_text)),
    );
    let store = app.try_state::<AiGenerationStore>()?;
    // Read-only lookup, never create-on-miss — see this function's own doc.
    // A miss (no Application, or the store isn't running) just leaves the FK
    // unset; the row is still found by `AiGenerationStore::find_for_job`
    // (keyed on `job_url`, not the FK) — only Application-scoped readers
    // miss it, and a later save retries the same idempotent lookup.
    let application_id = app
        .try_state::<crate::applications::ApplicationStore>()
        .and_then(|apps| {
            let normalized = crate::applications::normalize_job_url(job_url);
            apps.find_by_job_url(&normalized).map(|found| found.id)
        });
    let record = AiGenerationRecord {
        id: make_generation_id(),
        created_at: crate::db::now_ms(),
        target_language: clamped.target_language.clone(),
        top_requirements: clamped.top_requirements.clone(),
        resume_text: ctx.draft.clone(),
        // ONLY the stage-generated letter, never the fallback: `save_application`'s
        // merge-upsert treats an empty `cover_letter_text` as "keep the
        // existing value" (`ai_generations::pick`), so writing the legacy
        // validate-only request text here would overwrite whatever letter the
        // posting's aggregate already had with a document this run never
        // generated.
        cover_letter_text: ctx.letter.clone(),
        job_ad: job_ad_for_persist.to_string(),
        job_url: job_url.to_string(),
        board: meta.board.clone(),
        company_name: meta.company.clone(),
        job_title: meta.title.clone(),
        quality_report: wrapper.clone(),
        application_id,
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
    /// There WAS a document, and it was rejected — for the user-facing reason
    /// carried here, surfaced verbatim by `execute` (see
    /// [`LOST_WORK_HISTORY_MESSAGE`]/[`LEAKED_FENCE_TAG_MESSAGE`]).
    Refused(&'static str),
}

/// [`SaveVerdict::Refused`] reason: [`is_persistable`] rejected the draft for
/// dropping the source's whole work history.
const LOST_WORK_HISTORY_MESSAGE: &str = "The generated résumé came back without any of your work \
    history, so your saved document was left unchanged. Try again.";

/// [`SaveVerdict::Refused`] reason: the résumé or letter echoed one of the
/// internal prompt-fence wrapper tags (`<generated_resume>`,
/// `<candidate_resume>`, …) instead of real content — see
/// [`crate::prompt_fence::contains_fence_tag`]. `draft`/`cover_letter` are the
/// SOLE producers of these documents (unlike `humanize`/`repair`, which can
/// discard a bad candidate and keep the last-good text they started from), so
/// `save_verdict` is the last chokepoint that can catch a leak here before it
/// reaches the saved aggregate and the exported PDF. Refusing to save —
/// rather than saving with a flagged report — is the deliberate choice: it
/// costs the run producing nothing this time, but a raw framework artifact
/// reaching an employer is worse than a document the user has to retry, and
/// it keeps this gate consistent with [`LOST_WORK_HISTORY_MESSAGE`] above
/// rather than inventing a second, weaker failure mode for a defect that is
/// arguably more visible to the reader.
const LEAKED_FENCE_TAG_MESSAGE: &str = "The generated document came back with an internal \
    formatting artifact that must never reach your résumé or cover letter, so your saved \
    document was left unchanged. Try again.";

/// One definition of "will this save", shared by `persist_document` (which
/// acts on it) and `execute` (which has to report it). `letter` is checked
/// independently of `draft`: `persist_document` writes both documents into
/// ONE `AiGenerationRecord`, so there is no lower-granularity save to fall
/// back to — a defect in EITHER document refuses the whole save, the same way
/// [`is_persistable`] already treats a résumé-only defect as blocking it.
pub(crate) fn save_verdict(
    source_resume: &str,
    draft: &str,
    letter: &str,
    job_url: &str,
) -> SaveVerdict {
    if draft.trim().is_empty() || job_url.trim().is_empty() {
        return SaveVerdict::Nothing;
    }
    if !is_persistable(source_resume, draft) {
        return SaveVerdict::Refused(LOST_WORK_HISTORY_MESSAGE);
    }
    // `humanize`/`sections` already gate their OWN rewrite candidates against
    // an echoed fence tag (`is_usable_rewrite`, the splice guard in
    // `sections.rs`) — but `draft`/`cover_letter` are what FIRST produce this
    // text, and neither had a shape check of its own before this gate.
    if crate::prompt_fence::contains_fence_tag(draft)
        || crate::prompt_fence::contains_fence_tag(letter)
    {
        return SaveVerdict::Refused(LEAKED_FENCE_TAG_MESSAGE);
    }
    SaveVerdict::Save
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
///   removed `max` depth's now-deleted per-section fan-out treated a
///   daily-cap refusal as `StoppedReason::Budgeted`, broke, and returned
///   `Ok(())` — and nothing downstream converted that into an `Err`. So a max
///   run that hit the cap right after Summary produced a summary-only
///   document with `outcome == Ok`, and it overwrote the saved résumé.
///
/// Comparing the two DOCUMENTS answers both at once and needs no run state:
/// a source with no work history can never trip it, and a truncated draft
/// over a real one always does — however the run happened to end.
///
/// Both sides go through the SAME [`sections::find`] seam, so the
/// undated-entry caveat (an entry with no date column is not a
/// `LineKind::JobEntry`) applies equally to each and cannot create a false
/// asymmetry.
pub(crate) fn is_persistable(source_resume: &str, draft: &str) -> bool {
    has_work_history(draft) || !has_work_history(source_resume)
}

/// Whether `text` has an employment section with anything under it.
///
/// The SECTION with a body, not a per-entry line range: `LineKind::JobEntry`
/// legitimately fails for an entry with no date column, and a résumé whose
/// dates the source never carried is a real document rather than an empty
/// one.
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
    let (source, source_is_provenanced) = source_resume_for(&app, &row, &record);
    // A whole-section REWRITE using `repair`'s prompt and grounding — the ONLY
    // path now that the max-depth per-entry artifact rebuild is gone. It
    // follows `repair`'s own override, the stage whose prompt this uses.
    let outcome = regenerate_one_section(
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
    .await?;
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

    let (spliced, projects_normalize_skipped) =
        normalize_regenerated_projects(key, &source, source_is_provenanced, spliced);

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
    // Content-free (ADR-027): a code, never the document — the same reason
    // vocabulary `Draft::run`'s ledger artifact uses, so a declined Projects
    // normalization on a manual regenerate is as observable as it is on a run.
    span.end_with(
        &format!(
            "issues={} blocking={}{}",
            report.issues.len(),
            report::has_criticals(&report),
            projects_normalize_skipped
                .map(|reason| format!(" projectsNormalizeSkipped={reason}"))
                .unwrap_or_default()
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

/// The SOURCE résumé a re-validation must measure against, plus whether it is
/// a REAL provenance hit rather than the fallback.
///
/// A generation record stores the OUTPUT, not the input it was built from, so
/// the source is looked up through the id the run recorded as provenance (see
/// `execute`'s `sourceResumeId`). When that document is gone — deleted,
/// replaced, or the run predates the field — the fallback is the generated text
/// itself: measured against itself, the factual checks find nothing, so the
/// re-validated report comes back WEAKER than the run's original rather than
/// wrong. Weaker-and-honest beats a fabricated Critical against a source this
/// document was never written from.
///
/// The `bool` is why this returns a pair rather than the string alone: that
/// fallback is fine for VALIDATION (grading against your own output finds
/// nothing, so it degrades safely), but it is not a WRITE authority — seeding
/// `projects::normalize_projects` from the generated text would "restore"
/// links out of the document it is about to overwrite, i.e. codify whatever
/// the last generation happened to say as fact. Callers that write must check
/// this flag; callers that only validate may ignore it.
fn source_resume_for(app: &AppHandle, row: &RunRow, record: &AiGenerationRecord) -> (String, bool) {
    let hit = serde_json::from_str::<Value>(&row.metrics_json)
        .ok()
        .and_then(|metrics| {
            metrics
                .get("sourceResumeId")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .and_then(|id| app.state::<DocumentStore>().get(&id));
    match hit {
        Some(document) => (document.text, true),
        None => (record.resume_text.clone(), false),
    }
}

/// The pure decision behind `resume_pipeline_regenerate_section`'s Projects
/// normalization — pulled out of the command so it is unit-testable without
/// an `AppHandle`/store. Deterministic, zero-cost: a REGENERATE-PROJECTS
/// click can rewrite the Projects section too (the whole-section rewrite
/// fallback is a free-text rewrite, not scoped by content), so the same
/// code-owned normalization the run itself applies must run here before the
/// fresh report is computed — otherwise a click could persist an altered
/// project link the run would never have let through.
///
/// Scoped to `key == Projects` ONLY: normalizing on every OTHER section's
/// regenerate would silently revert the user's own manual edits to Projects
/// every time they regenerate, say, Skills.
///
/// Gated on `source_is_provenanced`: [`source_resume_for`] falls back to the
/// run's own GENERATED text when `sourceResumeId` is missing/deleted, which
/// is fine for grading (measuring text against itself finds nothing) but not
/// for WRITING — seeding the normalizer from the document it is about to
/// overwrite would "restore" whatever the last generation said as if it were
/// the candidate's own source.
///
/// Returns the (possibly normalized) text PLUS a content-free reason
/// (ADR-027) when normalization declined to run — `key != Projects` reports
/// none (there was nothing to attempt), but every other decline is
/// observable, the same way `Draft::run`'s `apply_projects_normalization`
/// reports one on the run's ledger. The caller folds it into the command's
/// own `Span`.
pub(crate) fn normalize_regenerated_projects(
    key: SectionKey,
    source: &str,
    source_is_provenanced: bool,
    spliced: String,
) -> (String, Option<&'static str>) {
    if key != SectionKey::Projects {
        return (spliced, None);
    }
    if !source_is_provenanced {
        return (spliced, Some("unprovenanced_source"));
    }
    let (project_seeds, seed_skip_reason) = projects::seed_projects_for_normalize(source);
    match projects::normalize_projects_outcome(&spliced, &project_seeds) {
        projects::ProjectsNormalizeOutcome::Applied(text, _) => (text, None),
        projects::ProjectsNormalizeOutcome::Skipped(reason) => (spliced, Some(reason)),
        projects::ProjectsNormalizeOutcome::NoOp => (spliced, seed_skip_reason),
    }
}
