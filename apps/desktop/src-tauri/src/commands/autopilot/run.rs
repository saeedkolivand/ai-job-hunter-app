//! One autopilot run end to end: the in-flight guard, the scrape -> rank -> notes -> record pipeline
//! and the cancel handling around it. The thin `#[tauri::command]` wrapper is `autopilot_run` in the
//! parent module.

use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use super::keyword_rank::{
    cluster_aware_retain, matches_keyword_filters, readmit_stale_known_jobs,
};
use super::phases::{
    assistant_notes_step, report_rerank, semantic_rerank_step, spawn_linkedin_enrichment,
    StepEmitter,
};
use super::rerank::by_rank;
use super::{build_found_job, snapshot_dedup_inputs, store};
use crate::autopilot::{FoundJob, RunStatus};
use crate::autopilot_helpers::autopilot_scrape;
use crate::db::{new_job_id, now_ms};
use crate::scraping::{JobPosting, ScraperEngine};

/// Process-global set of autopilot ids with a run currently in flight. Backs the
/// concurrent-run guard on [`autopilot_run`]: a double-invoke of the SAME
/// autopilot (the scheduler's retry racing a fresh occurrence, a scheduled run
/// racing a manual one, or two manual clicks) must not double-run. It is
/// process-local and transient (holds no user data), so it lives in a module
/// static rather than managed Tauri state / the reset registry, and resets on
/// restart — where any run left `InProgress` is separately reconciled to
/// `Interrupted`.
static RUNS_IN_FLIGHT: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// RAII claim on an in-flight autopilot run. [`RunGuard::try_acquire`] returns
/// `None` when a run for `id` is already in flight (the caller must no-op);
/// dropping the returned guard removes the id, so the claim is released on EVERY
/// exit path — a normal return, an early `?`, or a panic unwind. The lock is
/// held only for the check-and-insert (and the drop), never across an `.await`.
pub(super) struct RunGuard(String);

impl RunGuard {
    pub(super) fn try_acquire(id: &str) -> Option<RunGuard> {
        let mut in_flight = RUNS_IN_FLIGHT.lock();
        if in_flight.contains(id) {
            None
        } else {
            in_flight.insert(id.to_string());
            Some(RunGuard(id.to_string()))
        }
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        RUNS_IN_FLIGHT.lock().remove(&self.0);
    }
}

/// Finalize a user-cancelled autopilot run identically at both cancel sites (a
/// Stop caught in the scrape `Err` arm, and a Stop caught before we record
/// results). Clears the live run status AND the prior run's stale summaries
/// (this run never reached `record_run`, so a lingering chip strip would render
/// stale board data as if it belonged to this cancelled run), marks the job
/// cancelled, ends the `span` with the site-specific `span_msg`, and returns the
/// cancelled payload. Extracted so the two sites can't drift.
///
/// The engine cancel token is unregistered by each caller at its own point (the
/// scrape `Err` arm has already done so before reaching here; the pre-record
/// site does it inline just before the call), so that is deliberately NOT part
/// of this helper.
fn finish_cancelled(
    app: &AppHandle,
    span: &crate::observability::Span,
    autopilot_id: &str,
    job_id: &str,
    span_msg: &str,
) -> Value {
    store(app)
        .lock()
        .set_run_status_clearing_summaries(autopilot_id, RunStatus::Completed);
    crate::commands::jobs::job_cancel(app, job_id);
    span.end_with(span_msg, false);
    json!({ "jobId": job_id, "cancelled": true })
}

pub(super) async fn autopilot_run(app: AppHandle, autopilot_id: String) -> Value {
    // Concurrent-run guard: a double-invoke of the SAME autopilot must not
    // double-run. Held for the whole command body — the RAII guard releases on
    // every exit path (each early return below, and a panic unwind). PR B's
    // startup reconcile only covers a stale `InProgress` after a crash; this
    // covers a live overlap (scheduler retry vs. fresh occurrence, or a manual
    // click racing either).
    let Some(_run_guard) = RunGuard::try_acquire(&autopilot_id) else {
        log::info!("[autopilot] run already in flight for {autopilot_id}; skipping double-invoke");
        return json!({ "skipped": "already-running" });
    };

    let autopilot = store(&app).lock().get(&autopilot_id);

    let Some(autopilot) = autopilot else {
        return json!({ "error": format!("autopilot not found: {autopilot_id}") });
    };

    let target = autopilot.target.clone();
    let filter = autopilot.filter.clone();

    let span = crate::observability::Span::begin(
        "autopilot",
        format!("run={autopilot_id} boards={}", target.boards.join(",")),
    );

    let job_id = new_job_id();
    crate::commands::jobs::job_start(&app, &job_id, "autopilot.run");

    // Mark the run live so the UI shows a "running" badge and a crash mid-run
    // is later reconciled to "interrupted" (see `mark_interrupted_runs`).
    store(&app)
        .lock()
        .set_run_status(&autopilot_id, RunStatus::InProgress);

    let engine = app.state::<Arc<ScraperEngine>>().inner().clone();
    let cancel_token = CancellationToken::new();
    engine.register_token(&job_id, cancel_token.clone()).await;

    let steps = StepEmitter {
        app: app.clone(),
        autopilot_id: autopilot_id.clone(),
        job_id: job_id.clone(),
    };

    steps.emit(
        "scrape_start",
        &format!("Scraping {}", target.boards.join(", ")),
    );

    let (postings, summaries) = match autopilot_scrape(&engine, &target, &job_id, &app).await {
        Ok(out) => out,
        Err(e) => {
            engine.unregister_token(&job_id).await;
            // A Stop before any item streamed surfaces as `Err("scrape cancelled")`
            // from the engine, so check the token FIRST — otherwise a run the user
            // stopped is persisted `Failed`, and the `{error}` payload makes
            // `outcome_failed` true, which re-runs the very scrape they stopped.
            // Mirrors the Ok-path cancel handler below.
            if cancel_token.is_cancelled() {
                return finish_cancelled(
                    &app,
                    &span,
                    &autopilot_id,
                    &job_id,
                    "cancelled during scrape",
                );
            }
            // Whole-batch failure never reached `record_run`, so there are no
            // fresh summaries for this run — clear the PRIOR run's, or a later
            // chip strip would render stale board data as if it were this run's.
            store(&app).lock().fail_run_without_summaries(&autopilot_id);
            crate::commands::jobs::job_fail(&app, &job_id, e.to_string());
            span.end(false);
            return json!({ "error": e, "jobId": job_id });
        }
    };

    // Passively harvest ATS company slugs from EVERY scraped posting URL BEFORE any
    // keyword/score filtering (ADR-030 §c "harvest every stored posting URL"),
    // matching the manual-scrape harvest point. Parse-only, zero network. Resolve the
    // store at this shell boundary; a missing store (startup failure) is a no-op.
    if let Some(store) = app.try_state::<crate::discovered::DiscoveredCompanyStore>() {
        crate::discovered::harvest_ats_refs(
            store.inner(),
            postings.iter().map(|p| (p.url.clone(), p.company.clone())),
            "scrape",
        );
    }

    // Raw count BEFORE the keyword filter, so `scrape_done` can distinguish "no
    // board returned anything" from "boards returned jobs but your keyword filter
    // dropped them all" — the difference between a scraping problem and an
    // over-restrictive filter (the autopilot zero-jobs bug).
    let raw = postings.len();

    // Surface *why* a run came up short: when any board errored or was skipped,
    // emit a diagnostic step (esp. relevant when `raw == 0`) so the UI can show
    // "aggregator: 429 rate limited" instead of a silent empty result.
    let reasons = crate::autopilot_helpers::scrape_diagnostics(&summaries);
    if !reasons.is_empty() {
        steps.emit("scrape_diag", &reasons);
    }

    // Apply the user's keyword filters to the scraped postings — must-include
    // (all keywords present) + exclude (any keyword present drops it). These were
    // dead config before; now they actually shape the fetched results.
    let postings: Vec<JobPosting> = postings
        .into_iter()
        .filter(|p| matches_keyword_filters(p, &filter))
        .collect();

    let total_found = postings.len();
    steps.emit(
        "scrape_done",
        &format!("Scraped {raw}; {total_found} passed your keyword filter"),
    );

    // Phase 1: snapshot each posting, scored 0–100 against the resume when one is
    // set, then sorted highest-first. The score is the keyword-coverage match % —
    // the SAME embedding-free kernel as the Jobs page's ATS sub-score (NOT the
    // Jobs *combined* %) — and this phase makes no embedding/API call whatsoever.
    // With semantic scoring off (the default) that is the whole ranking pipeline;
    // with it on, phase 2 below re-scores the head through the combined kernel.
    // Autopilot is a discovery agent either way: a run only finds, ranks and saves
    // results — the user applies with the tailoring assistant.
    let resume = autopilot.resume_text.as_deref().unwrap_or("");
    let found_at = now_ms();
    let mut found_jobs: Vec<FoundJob> = postings
        .iter()
        .map(|p| build_found_job(p, resume, found_at))
        .collect();

    // Highest keyword-coverage match first; unscored postings sort to the end.
    found_jobs.sort_by(by_rank);

    let scored_count = found_jobs.iter().filter(|f| f.score.is_some()).count();

    // Snapshot the durable dedup verdicts + agency extras ONCE for this run —
    // reused by the cluster-aware retention here and the annotation pass inside
    // `record_run` below, so retention and the persisted groups agree (ADR-029).
    let (tombstones, extra_agency) = snapshot_dedup_inputs(&app);

    // Honour the autopilot's minimum match score, cluster-aware (ADR-029 §g): a
    // cluster passes iff its best-scored member clears the bar, and a passing
    // cluster keeps ALL its members (a below-bar copy still contributes a source
    // chip + salary data). A fully-unscored cluster keeps today's keep-unscored
    // behavior. Until PR E `minMatchScore` was per-row; it is now per-cluster.
    let threshold = filter.min_match_score;

    // Snapshot the just-scored batch BEFORE the retain filter below can drop
    // any of it — `readmit_stale_known_jobs` needs the pre-filter score to
    // re-admit an already-known job the filter excludes (see its doc).
    let scored_before_retain = found_jobs.clone();

    let clusters;
    (found_jobs, clusters) =
        cluster_aware_retain(found_jobs, threshold, &tombstones, &extra_agency);
    let kept = found_jobs.len();
    let dropped = total_found - kept;

    // Phase 2 (opt-in, ADR-020 addendum): the semantic re-rank of the head of the ranking, placed
    // AFTER the retain so `minMatchScore` keeps its keyword-coverage meaning — see the step.
    let rerank = semantic_rerank_step(
        &app,
        &steps,
        resume,
        &postings,
        &mut found_jobs,
        &clusters,
        &cancel_token,
    )
    .await;
    // Re-sort: phase 2 replaced the head's scores, so the keyword ordering no
    // longer holds. `by_rank` keeps the two scales in separate blocks — see its
    // doc. A no-op when nothing was re-ranked.
    found_jobs.sort_by(by_rank);

    let rerank_detail = report_rerank(&steps, rerank.as_ref());

    steps.emit(
        "rank_done",
        &format!(
            "Keyword-matched {scored_count}/{total_found}; kept {kept} at or above {threshold:.0}% coverage (dropped {dropped}){rerank_detail}"
        ),
    );

    let notes_generated =
        assistant_notes_step(&app, &autopilot, &mut found_jobs, &cancel_token).await;

    // Bail cleanly if the run was cancelled (tray/UI) any time before we commit
    // — don't record results or fire a "new jobs" notification for an aborted
    // run. `cancel(job_id)` flips the token this run registered (engine reuses,
    // not overwrites, the slot), so cancels during scrape land here too.
    if cancel_token.is_cancelled() {
        engine.unregister_token(&job_id).await;
        return finish_cancelled(
            &app,
            &span,
            &autopilot_id,
            &job_id,
            "cancelled before recording results",
        );
    }

    // Derive the honest run outcome from the per-board summaries BEFORE they are
    // moved into `record_run` — so the command's resolved payload can carry the
    // same status the record persists, letting the renderer branch on an
    // all-boards-failed run (`failed`) instead of reading the success-shaped
    // `{ found: 0 }` as "done".
    let run_status = crate::autopilot::derive_run_status(&summaries);
    // Re-admit any already-known job the retain filter dropped above so its
    // fresh score still reaches `merge_found_jobs` — excluded from `kept`/
    // `dropped` and from phase 2/AI notes (both already ran on the filtered
    // list), it exists in this batch purely so the merge can refresh it.
    let found_jobs =
        readmit_stale_known_jobs(found_jobs, &scored_before_retain, &autopilot.found_jobs);
    let new_count = store(&app).lock().record_run(
        &autopilot_id,
        kept as u32,
        0,
        found_jobs,
        summaries,
        &tombstones,
        &extra_agency,
    );

    // LinkedIn-only post-discovery description enrichment (issue #1114), a best-effort background
    // pass right after `record_run` — see the step.
    spawn_linkedin_enrichment(&app, &autopilot_id);

    // Surface genuinely-new finds while the user is away: a permission-gated
    // notification + a "New jobs: N" tray counter that jumps back to this run.
    // `notes_generated` (≤ new_count, since only new matches are annotated) lets
    // the banner mention how many carry an AI note.
    crate::tray::on_new_jobs(
        &app,
        &autopilot_id,
        &autopilot.name,
        new_count,
        notes_generated,
    );

    engine.unregister_token(&job_id).await;

    crate::commands::jobs::job_complete(&app, &job_id, json!({ "found": kept, "applied": 0 }));

    steps.emit("complete", &format!("Found {kept}, saved for review"));

    span.end_with(&format!("found={kept} applied=0"), true);
    // `status` mirrors the outcome persisted on the record (`completed` /
    // `completedWithErrors` / `failed`) so a caller that only inspects the
    // resolved payload can still tell a run that found real jobs from one where
    // every board failed — the previous success-only shape hid the difference.
    json!({ "jobId": job_id, "found": kept, "applied": 0, "status": run_status })
}
