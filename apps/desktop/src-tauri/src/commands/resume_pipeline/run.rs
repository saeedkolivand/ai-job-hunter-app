//! [`resume_pipeline_run`] — start one staged résumé run — and [`execute`],
//! the run itself from resolution through persistence.

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::commands::ai_provider::timeouts;
use crate::documents::DocumentStore;
use crate::error::AppError;
use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;
use crate::jobs::cancel::CancelRegistry;
use crate::pipeline::cache::KvCache;
use crate::pipeline::resume::early_research::race_background;
use crate::pipeline::resume::{quality_pipeline, QualityCtx, QualityInput, RunDeadline, RunLedger};
use crate::pipeline::runs::{PipelineRunStore, RunRow};
use crate::pipeline::Completer;

use super::hooks::{self, RunHooks};
use super::persist::persist_document;
use super::resolve::{
    self, clamp_request, job_ad_for_persist, job_meta_from_request, job_source, resume_source,
};
use super::save::{save_verdict, SaveVerdict};
use super::{
    max, notify, report, RUN_DEPTH, RUN_KIND, STATUS_CANCELLED, STATUS_FAILED, STATUS_RUNNING,
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
    let job_id = crate::db::new_job_id();
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
    // A1 (hardening plan): neither source above sanitizes `company` — a
    // scraped posting's own field least of all — so this is the one place
    // every run's company name passes through
    // `crate::scraping::trust::is_implausible_company` before it can reach
    // `QualityInput::company_name` (company research) or the persisted
    // `AiGenerationRecord.company_name` below. See `resolve::sanitize_job_meta`.
    let meta = resolve::sanitize_job_meta(meta);
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
            market: &clamped.market,
            today: &clamped.today,
            cover_letter: &clamped.cover_letter,
            include_cover_letter: req.include_cover_letter,
            include_resume: req.include_resume,
            company_name: &meta.company,
            research_company: req.research_company,
            effort: req.effort.as_deref(),
            job_id,
        },
        &completer,
        cache.as_deref(),
        deadline,
        Arc::clone(&ledger),
    )
    .with_stage_completers(&stage_completers);

    // Company research for the letter runs beside the stages that precede it
    // instead of inside the letter stage; dropped with the pipeline future,
    // and on `cancel`. `None` unless the run writes a letter AND asked for it.
    let early_research = ctx.start_early_research(cancel.clone());
    let outcome = race_background(
        quality_pipeline().run_hooked(&mut ctx, &hooks),
        early_research,
    )
    .await;

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
    // produced-nothing case, which is benign and already reported by its own
    // path. `ctx.input.include_resume` is what tells the verdict WHICH document
    // to ask those two questions of — without it, a cover-letter-only run's
    // empty draft reads as a failed résumé and the letter is either silently
    // discarded (`Nothing`) or refused with a work-history message about a
    // document the run was never asked to write.
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
    let verdict = save_verdict(
        ctx.input.source_resume,
        &ctx.draft,
        &ctx.letter,
        &job_url,
        ctx.input.include_resume,
    );
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
        // PROVENANCE again, same reasoning and the same home: did THIS run
        // write the résumé the posting's aggregate now holds? Read back by
        // `run_wrote_a_resume` to keep `regenerateSection` off a document the
        // run never produced. Written unconditionally (unlike `sourceResumeId`
        // above) because ABSENT has to keep meaning `true` — every run that
        // predates the cover-letter-only mode wrote one, and a missing key
        // must not lock those out of a section regenerate.
        object.insert("resumeInRun".to_string(), json!(ctx.input.include_resume));
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
