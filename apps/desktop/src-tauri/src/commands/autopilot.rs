use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

use parking_lot::Mutex;

use crate::autopilot::{
    Autopilot, AutopilotFilter, AutopilotStatus, AutopilotStore, FoundJob, RunStatus, ScoreSource,
};
use crate::autopilot_helpers::autopilot_scrape;
// The save-path `country_code` backfill (a location saved without a geocode
// pick) — shared with the manual scrape path since trust-fix #2.
use crate::commands::geocoding::derive_country_code;
use crate::db::{new_job_id, now_ms};
use crate::scraping::{JobPosting, ScraperEngine};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::events::{emit_event, AUTOPILOT_STEP};
use tokio_util::sync::CancellationToken;

// AutopilotCreateRequest / AutopilotUpdateRequest are generated from the Zod
// schemas in packages/shared by `pnpm gen:ipc`.
pub use crate::ipc_contracts::autopilot::{AutopilotCreateRequest, AutopilotUpdateRequest};

fn store(app: &AppHandle) -> Arc<Mutex<AutopilotStore>> {
    app.state::<Arc<Mutex<AutopilotStore>>>().inner().clone()
}

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
struct RunGuard(String);

impl RunGuard {
    fn try_acquire(id: &str) -> Option<RunGuard> {
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

/// Fill each found job's `applied` from the set of `job_url`s that have a saved
/// generation — so the badge reflects a real link (a generation exists for that
/// job) rather than a hand-set flag that could drift.
fn enrich_applied(app: &AppHandle, list: &mut [crate::autopilot::Autopilot]) {
    // "Applied" is now derived from the Application aggregate (ADR 0001): a URL
    // counts as applied when it has an Application whose status is past `saved`.
    // The set is keyed by the SAME normalization the store applies on write, so
    // found-job urls must be normalized before the membership check below.
    let applied = app
        .try_state::<crate::applications::ApplicationStore>()
        .map(|s| s.applied_job_urls())
        .unwrap_or_default();
    if applied.is_empty() {
        return;
    }
    for ap in list.iter_mut() {
        for job in ap.found_jobs.iter_mut() {
            let key = crate::applications::normalize_job_url(&job.url);
            job.applied = applied.contains(&key);
        }
    }
}

#[tauri::command]
pub fn autopilot_list(app: AppHandle) -> Value {
    let mut list = store(&app).lock().list();
    enrich_applied(&app, &mut list);
    json!(list)
}

#[tauri::command]
pub fn autopilot_get(app: AppHandle, autopilot_id: String) -> Value {
    let ap = store(&app).lock().get(&autopilot_id).map(|a| {
        let mut one = [a];
        enrich_applied(&app, &mut one);
        let [ap] = one;
        ap
    });
    json!(ap)
}

#[tauri::command]
pub async fn autopilot_create(app: AppHandle, mut req: AutopilotCreateRequest) -> Value {
    if req.target.country_code.is_none() {
        req.target.country_code = derive_country_code(req.target.location.as_deref()).await;
    }
    let ap = store(&app)
        .lock()
        .create(serde_json::to_value(&req).unwrap_or_default());
    json!(ap)
}

#[tauri::command]
pub async fn autopilot_update(
    app: AppHandle,
    autopilot_id: String,
    mut req: AutopilotUpdateRequest,
) -> Value {
    if let Some(target) = req.target.as_mut() {
        if target.country_code.is_none() {
            target.country_code = derive_country_code(target.location.as_deref()).await;
        }
    }
    let patch = serde_json::to_value(&req).unwrap_or_default();
    let ap = mutate_record(&app, &autopilot_id, |records| {
        records.lock().update(&autopilot_id, patch)
    });
    json!(ap)
}

#[tauri::command]
pub fn autopilot_remove(app: AppHandle, autopilot_id: String) -> Value {
    mutate_record(&app, &autopilot_id, |records| {
        records.lock().remove(&autopilot_id)
    });
    json!(null)
}

/// Run a mutation of ONE autopilot record and drop whatever résumé-derived
/// cache rows it orphaned.
///
/// Both halves live here so a mutation path cannot ship with only the first: a
/// DELETE and a `resume_text` REPLACE orphan the identical rows, because the
/// cache identity IS the résumé's content hash. (An UPDATE shipped without this
/// exact defect and it took a second review round to notice.)
///
/// The "what is orphaned" question needs no diff: the list snapshot taken AFTER
/// the mutation still contains this record carrying its NEW text, so an
/// unchanged résumé is its own live producer and keeps its rows — the same
/// content-addressed rule that lets two autopilots share one row.
fn mutate_record<T>(
    app: &AppHandle,
    autopilot_id: &str,
    mutate: impl FnOnce(&Mutex<AutopilotStore>) -> T,
) -> T {
    let records = store(app);
    let previous_resume = records.lock().get(autopilot_id).and_then(|a| a.resume_text);
    let out = mutate(&records);
    // `try_state` because a mutation must never panic on an unmanaged store.
    if let Some(docs) = app.try_state::<crate::documents::DocumentStore>() {
        let remaining = records.lock().list();
        drop_orphaned_resume_cache(docs.inner(), previous_resume.as_deref(), &remaining);
    }
    out
}

/// Delete the cache rows derived from a résumé no autopilot carries any more —
/// its snapshot vector AND its cached match scores.
///
/// The re-rank caches the résumé under a content-addressed
/// `autopilot-resume:<sha256(text)>` id (see
/// `match_resume::autopilot_resume_id`), and every `match_scores` row that id
/// produced holds résumé-derived content of its own (gaps, recommendations, the
/// explanation). Both live in caches whose only bounds are a TTL and a row cap,
/// so without this they outlive the record they came from by up to the TTL (7
/// days at the default tier). The same-text check is what keeps a shared résumé
/// working: the id is the CONTENT, so another autopilot with the same résumé is
/// still a live producer of those rows.
///
/// Best-effort: a failed delete is logged, never surfaced — the caller has
/// already mutated the record.
fn drop_orphaned_resume_cache(
    docs: &crate::documents::DocumentStore,
    removed_resume: Option<&str>,
    remaining: &[Autopilot],
) {
    let Some(text) = removed_resume.filter(|t| !t.is_empty()) else {
        return;
    };
    if remaining
        .iter()
        .any(|a| a.resume_text.as_deref() == Some(text))
    {
        return; // another autopilot still produces these exact rows
    }
    let id = crate::commands::match_resume::autopilot_resume_id(text);
    if let Err(e) = docs.delete_posting_vector(&id) {
        log::warn!("[autopilot] could not drop the résumé snapshot vector: {e}");
    }
    if let Err(e) = docs.delete_match_scores_for_resume(&id) {
        log::warn!("[autopilot] could not drop the résumé's cached match scores: {e}");
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

#[tauri::command]
pub async fn autopilot_run(app: AppHandle, autopilot_id: String) -> Value {
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

    let ap_id = autopilot_id.clone();
    let emit_step = move |app: &AppHandle, job_id: &str, step: &str, detail: &str| {
        emit_event(
            app,
            AUTOPILOT_STEP,
            json!({ "jobId": job_id, "autopilotId": ap_id, "step": step, "detail": detail }),
        );
    };

    emit_step(
        &app,
        &job_id,
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
        emit_step(&app, &job_id, "scrape_diag", &reasons);
    }

    // Apply the user's keyword filters to the scraped postings — must-include
    // (all keywords present) + exclude (any keyword present drops it). These were
    // dead config before; now they actually shape the fetched results.
    let postings: Vec<JobPosting> = postings
        .into_iter()
        .filter(|p| matches_keyword_filters(p, &filter))
        .collect();

    let total_found = postings.len();
    emit_step(
        &app,
        &job_id,
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
    let clusters;
    (found_jobs, clusters) =
        cluster_aware_retain(found_jobs, threshold, &tombstones, &extra_agency);
    let kept = found_jobs.len();
    let dropped = total_found - kept;

    // ── Phase 2 (opt-in, ADR-020 addendum): semantic re-rank ──────────────────
    // Everything above is phase 1 — the free, embedding-free keyword prefilter,
    // byte-for-byte the pre-existing pipeline. When (and ONLY when) the user has
    // semantic scoring on, the head of that ranking is re-scored through the
    // SAME combined kernel the Jobs page uses. With the setting off — the
    // default — the block below is not entered at all: no map is built, no
    // provider is resolved, and a scheduled run makes zero embed calls, exactly
    // as before.
    //
    // Placed AFTER the retain, deliberately: `minMatchScore` keeps its existing
    // keyword-coverage meaning (no silent threshold regression for existing
    // autopilots), and only jobs that survived dedup can cost an embed.
    //
    // A résumé-less autopilot short-circuits with the flag: phase 1 produced no
    // scores at all for it, so there is nothing to re-rank. The gate itself is
    // `should_semantic_rerank`, applied inside `semantic_rerank_phase` — which
    // is also what keeps the `setup` closure below (the state resolve + the blob
    // map) from running at all on a keyword-only run.
    //
    // `try_state` (not `state`) for the same reason the setup closure uses it:
    // a run must never fail because of scoring, and `state` PANICS on an
    // unmanaged type — reachable on the startup catch-up tick, which can fire
    // before every store is registered. The degrade is silent otherwise, so it
    // is logged here exactly like the setup closure logs its own missing-state
    // case: "Autopilot never re-ranks" with no line anywhere is not debuggable.
    let semantic_on = match app.try_state::<crate::job_preferences::JobPreferencesStore>() {
        Some(prefs) => prefs.semantic_scoring(),
        None => {
            log::warn!(
                "[autopilot] job-preferences state unavailable; this run cannot read the \
                 semantic-scoring setting and ranks keyword-only"
            );
            false
        }
    };
    let rerank = semantic_rerank_phase(
        semantic_on,
        resume,
        &mut found_jobs,
        &clusters,
        &cancel_token,
        |candidates| {
            // `try_state` (not `state`) for both stores: a run must never fail
            // because of scoring, and `state` PANICS on an unmanaged type. A
            // startup failure that left either store unregistered degrades this
            // run to keyword-only instead of unwinding a scheduled tick.
            let (doc_store, limiter) = app
                .try_state::<crate::documents::DocumentStore>()
                .zip(app.try_state::<Arc<crate::limits::Limiter>>())?;
            // The user is entitled to know a scheduled run entered a phase that
            // can take minutes and spend budget — the neighbouring notes step
            // sets the same expectation.
            emit_step(
                &app,
                &job_id,
                "rerank_start",
                &format!("Semantic re-rank of the top {SEMANTIC_RERANK_MAX} matches"),
            );
            // Reuse phase 1's EXACT scoring blob per posting — `FoundJob` drops
            // `requirements`, so re-deriving it here would score different text
            // than the keyword phase did on the boards that populate that field.
            //
            // Built for the RE-RANK CANDIDATES (see `rerank_candidate_urls`),
            // not for the whole harvest: the unscored rows and the hidden
            // cluster members can never be scored, so their blobs are dead
            // weight. It is deliberately NOT trimmed to the top-N — the loop
            // reaches past position N whenever it skips a row, and a blob the
            // map lacks is a candidate silently dropped.
            let blobs: std::collections::HashMap<String, String> = postings
                .iter()
                .filter(|p| candidates.contains(p.url.as_str()))
                .filter_map(|p| {
                    crate::documents::keywords::posting_text_blob(
                        &p.title,
                        p.description.as_deref(),
                        p.requirements.as_deref(),
                    )
                    .map(|blob| (p.url.clone(), blob))
                })
                .collect();
            let active = doc_store.embedding_config();
            Some((
                LiveRerankEnv {
                    app: &app,
                    store: doc_store.inner(),
                    resume,
                    budget: RerankBudget::new(limiter.inner().clone(), active.provider.clone()),
                    active,
                },
                blobs,
            ))
        },
    )
    .await;
    // Re-sort: phase 2 replaced the head's scores, so the keyword ordering no
    // longer holds. `by_rank` keeps the two scales in separate blocks — see its
    // doc. A no-op when nothing was re-ranked.
    found_jobs.sort_by(by_rank);

    // A timed-out pass reports its PARTIAL counts (it spent embeds and promoted
    // jobs — saying nothing would describe the run as keyword-only) plus a step
    // of its own, because "re-ranked 4 of 20" alone cannot say whether the other
    // 16 were skipped by the ceiling, the breaker, or the clock.
    if let Some(s) = rerank.as_ref().filter(|s| s.timed_out) {
        emit_step(
            &app,
            &job_id,
            "rerank_timeout",
            &format!(
                "Semantic re-rank ran out of time after {}s; {} of {} re-ranked, the rest stay keyword-only",
                RERANK_STEP_TIMEOUT.as_secs(),
                s.rescored,
                s.considered
            ),
        );
    }
    let rerank_detail = match &rerank {
        Some(s) if s.timed_out => format!(
            "; semantic re-rank {}/{} before the time limit (kept keyword for the rest)",
            s.rescored, s.considered
        ),
        Some(s) => format!(
            "; semantic re-rank {}/{} (kept keyword for {})",
            s.rescored, s.considered, s.degraded
        ),
        None => String::new(),
    };

    emit_step(
        &app,
        &job_id,
        "rank_done",
        &format!(
            "Keyword-matched {scored_count}/{total_found}; kept {kept} at or above {threshold:.0}% coverage (dropped {dropped}){rerank_detail}"
        ),
    );

    // Phase 4 (opt-in, headless, READ-ONLY): after the keyword rank, attach a
    // short AI-reasoned note to the top NEW matches. Bounded (≤ ASSISTANT_NOTES_MAX
    // provider calls, per-provider daily ceiling, cancellable mid-call, AND an
    // overall wall-clock timeout — see `generate_assistant_notes`) and best-effort —
    // a provider/config error just means no notes, never a failed run. `prior_keys`
    // (this record's pre-run found jobs) lets the step skip re-surfaced jobs, whose
    // notes the store merge preserves for free, so a steady-state run makes zero
    // provider calls. No-op unless `autopilot.assistant` is set. Runs BEFORE
    // `record_run`/`on_new_jobs` below, so the wall-clock timeout is what keeps a
    // hung provider from delaying the user-facing "new jobs" notification.
    // Keyed on `canonical_job_key` — the SAME identity `merge_found_jobs` uses —
    // not the raw URL. A job that re-surfaces under different tracking params is
    // the same job to the merge, so keying on the raw URL paid for a note the
    // merge then discarded, every single run.
    let prior_keys: std::collections::HashSet<String> = autopilot
        .found_jobs
        .iter()
        .map(|j| crate::scraping::boards::common::canonical_job_key(&j.url, &j.title, &j.company))
        .collect();

    // Resolve the active provider from the BACKEND-OWNED store (task #16) through
    // the SAME centralized layer `ai_generate` uses — no longer from the per-record
    // `assistant_provider/model/base_url` snapshot. Missing/unknown/invalid →
    // `generate_assistant_notes` skips gracefully (the discovery run still completes
    // normally). Resolved HERE (the L3 command, which already holds the `AppHandle`)
    // and passed down already-resolved so `autopilot_helpers` (L2) never reaches up
    // into `crate::commands`.
    //
    // SECURITY (MEDIUM-4 fix): the old renderer-provenance `assistant_base_url`
    // snapshot is gone — it was a DURABLE, unattended egress target (a one-time
    // renderer compromise persisted a custom endpoint every scheduled tick). Routing
    // now comes from `AiConfigStore`, whose base_url was write-validated (scheme +
    // cloud-metadata block) and is defensively re-validated in `from_active`.
    //
    // ACCEPTED SEMANTICS CHANGE (owner signed off): a scheduled run follows the
    // CURRENTLY-active provider, not the one pinned when the schedule was created.
    //
    // Gated on the opt-in flag itself (not the fuller `notes_enabled`, which also
    // needs a résumé) so the vast majority of autopilots — AI notes OFF — never pay
    // for a resolve attempt or its log line; only an assistant-enabled autopilot with
    // a bad/missing provider logs the reason a user needs to debug "notes never run".
    let completer = if autopilot.assistant {
        crate::pipeline::Completer::from_active(&app)
            .inspect_err(|e| log::info!("[autopilot] AI notes skipped: no usable provider ({e})"))
            .ok()
    } else {
        None
    };
    let limiter = app.state::<Arc<crate::limits::Limiter>>().inner().clone();

    let notes_generated = crate::autopilot_helpers::generate_assistant_notes(
        completer.as_ref(),
        limiter,
        &autopilot,
        &mut found_jobs,
        &prior_keys,
        &cancel_token,
    )
    .await;

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
    let new_count = store(&app).lock().record_run(
        &autopilot_id,
        kept as u32,
        0,
        found_jobs,
        summaries,
        &tombstones,
        &extra_agency,
    );

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

    emit_step(
        &app,
        &job_id,
        "complete",
        &format!("Found {kept}, saved for review"),
    );

    span.end_with(&format!("found={kept} applied=0"), true);
    // `status` mirrors the outcome persisted on the record (`completed` /
    // `completedWithErrors` / `failed`) so a caller that only inspects the
    // resolved payload can still tell a run that found real jobs from one where
    // every board failed — the previous success-only shape hid the difference.
    json!({ "jobId": job_id, "found": kept, "applied": 0, "status": run_status })
}

/// Take + clear the buffered autopilot-focus id. Split from the command so it's
/// unit-testable without a Tauri `State`. Atomic: the lock is held across the take.
pub(crate) fn take_pending_focus(buf: &crate::tray::PendingFocus) -> Option<String> {
    buf.0.lock().take()
}

/// Atomically take + clear the autopilot-focus intent buffered by
/// `tray::dispatch_focus` (a cold-start `ajh://autopilot/<id>` deep link fires
/// during Rust setup, before the renderer's `useAutopilotFocusNavigation`
/// listener attaches, so the `autopilot:focus` emit is lost). The renderer PULLS
/// this once its JS loop is provably live (on mount + on the emitted event). The
/// atomic take means an intent is delivered exactly once and can't re-fire on a
/// later unrelated focus. Returns `None` (the common case) when nothing is
/// buffered. Returns the `autopilotId` string. Infallible — just a lock take — so
/// no `AppResult`.
#[tauri::command]
pub fn autopilot_take_pending_focus(
    state: tauri::State<'_, crate::tray::PendingFocus>,
) -> Option<String> {
    take_pending_focus(state.inner())
}

#[tauri::command]
pub fn autopilot_pause(app: AppHandle, autopilot_id: String) -> Value {
    store(&app)
        .lock()
        .set_status(&autopilot_id, AutopilotStatus::Paused);
    json!(null)
}

#[tauri::command]
pub fn autopilot_resume(app: AppHandle, autopilot_id: String) -> Value {
    store(&app)
        .lock()
        .set_status(&autopilot_id, AutopilotStatus::Active);
    json!(null)
}

// Helper functions

/// `JobPosting.source` of the aggregator board (Adzuna → JSearch). Adzuna caps
/// descriptions to a snippet and its detail pages block anonymous fetches, so a
/// keyword-coverage score computed over that snippet can diverge from the detail
/// pane's full-text re-score (trust-audit root cause 6). A run's aggregator
/// scores are therefore flagged provisional; direct full-text boards are not.
/// Sourced directly from the aggregator scraper's own `id()` constant (not a
/// duplicated literal), so a rename there can't silently desync this check.
const AGGREGATOR_SNIPPET_SOURCE: &str = crate::scraping::boards::aggregator::AGGREGATOR_BOARD_ID;

/// Pure `JobPosting → FoundJob` projection — the same one `autopilot_run`'s
/// `postings.iter().map(..)` calls. Extracted so a unit test can exercise the
/// REAL projection (every field, plus the `assess_trust(&p.url, &p.company)`
/// call and its arg order) instead of a hand-retyped mirror that could
/// silently drift from this one (e.g. a dropped field or swapped args).
pub(crate) fn build_found_job(p: &JobPosting, resume: &str, found_at: u64) -> FoundJob {
    // Keyword-coverage match %: share of the JD's keywords present in the
    // résumé, scored over the SAME blob as `commands::match_resume`
    // (title + description + requirements via `posting_text_blob`).
    // Embedding-free.
    let score = if resume.is_empty() {
        None
    } else {
        crate::documents::keywords::posting_text_blob(
            &p.title,
            p.description.as_deref(),
            p.requirements.as_deref(),
        )
        .map(|blob| crate::documents::keywords::coverage_score(resume, &blob))
    };
    FoundJob {
        title: p.title.clone(),
        company: p.company.clone(),
        url: p.url.clone(),
        location: p.location.clone(),
        board: {
            let s = p.source.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        },
        description: p.description.clone(),
        salary_min: p.extra.get("salaryMin").and_then(|v| v.as_f64()),
        salary_max: p.extra.get("salaryMax").and_then(|v| v.as_f64()),
        salary_currency: p
            .extra
            .get("salaryCurrency")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        score,
        // Only a real score is qualified: an aggregator (snippet-ranked) score
        // is provisional, a full-text board's is authoritative, and an unscored
        // job (no résumé/description) is neither.
        score_provisional: score.is_some() && p.source.trim() == AGGREGATOR_SNIPPET_SOURCE,
        // Phase 1 of the rank always produces a keyword-coverage number. The
        // optional phase-2 semantic re-rank (`semantic_rerank`, only when the
        // user has semantic scoring on) is the ONLY thing that promotes a job to
        // `Combined` — so a build-time default of `Keyword` is always honest,
        // including for a job whose re-rank later degrades.
        score_source: ScoreSource::Keyword,
        found_at,
        // The posting's publish-or-last-updated date, copied straight from the
        // source — most boards report a genuine publish date, but a few
        // (Jooble, Comeet, the Bundesagentur für Arbeit) only expose an
        // "updated"/"current" timestamp upstream (see `FoundJob::posted_at`);
        // a board with no date field at all leaves it `None`.
        posted_at: p.posted_at,
        // Set by the dedup merge in `record_run`; `applied` is derived on read.
        is_new: false,
        applied: false,
        // `p` never went through the engine's streaming wrapper (this Vec is
        // `scraper.search()`'s own separately-returned copy, not the
        // on_item-streamed one `ScraperEngine::run_one` attaches trust to) —
        // compute it directly here, same pure call.
        trust: Some(crate::scraping::trust::assess_trust(&p.url, &p.company)),
        // Set later by the AI-notes step (`generate_assistant_notes`) for the top
        // matches when the autopilot opted in; `None` on every fresh build.
        assistant_notes: None,
        // Cluster annotations are computed + written by `record_run`'s clustering
        // pass (and the retention pass), never at build time — defaults here.
        cluster_id: None,
        cluster_canonical: true,
        cluster_members: Vec::new(),
        is_agency: false,
    }
}

// ── Phase 2: optional semantic re-rank (ADR-020 addendum) ─────────────────────
//
// Split into a sibling module (see its doc) to keep this file under R8's LOC
// cap. Glob-imported so the phase's items stay nameable here — and from the
// test module below — exactly as they were before the move.
mod rerank;
use rerank::*;

/// Whether a posting passes the autopilot's keyword filters: it must contain
/// **all** must-include keywords and **none** of the exclude keywords, matched
/// case-insensitively against the title + description. Empty/absent lists are
/// no-ops.
fn matches_keyword_filters(posting: &JobPosting, filter: &AutopilotFilter) -> bool {
    let haystack = format!(
        "{} {}",
        posting.title.to_lowercase(),
        posting
            .description
            .as_deref()
            .unwrap_or_default()
            .to_lowercase()
    );

    if let Some(excludes) = &filter.exclude_keywords {
        let hits_excluded = excludes.iter().any(|k| {
            let k = k.trim().to_lowercase();
            !k.is_empty() && haystack.contains(&k)
        });
        if hits_excluded {
            return false;
        }
    }

    if let Some(keywords) = &filter.keywords {
        let all_present = keywords.iter().all(|k| {
            let k = k.trim().to_lowercase();
            k.is_empty() || haystack.contains(&k)
        });
        if !all_present {
            return false;
        }
    }

    true
}

/// Whether a found job clears the autopilot's `min_match_score`. The score being
/// gated is the keyword-coverage match % (the shared embedding-free kernel from
/// `commands::match_resume`). Postings we could not score (no resume set, or no
/// description to compare against) carry no score and are always kept — the
/// threshold only gates rankable jobs.
fn passes_min_score(job: &FoundJob, min_match_score: f64) -> bool {
    job.score.is_none_or(|s| s >= min_match_score)
}

/// Snapshot the durable dedup verdicts + agency extras from app state — the two
/// store-owned inputs every clustering call needs. Best-effort: a missing store
/// yields empty inputs (clustering degrades to "no splits / built-in agencies
/// only"), never a failure.
pub(crate) fn snapshot_dedup_inputs(app: &AppHandle) -> (HashSet<(String, String)>, Vec<String>) {
    let tombstones = app
        .try_state::<crate::dedup::DedupStore>()
        .map(|s| s.all_pairs())
        .unwrap_or_default();
    let extra_agency = app
        .try_state::<crate::job_preferences::JobPreferencesStore>()
        .map(|s| s.get().extra_agency_companies.unwrap_or_default())
        .unwrap_or_default();
    (tombstones, extra_agency)
}

/// Whether `a` is a better cluster representative than `b` for the min-score
/// gate: a scored member always beats an unscored one, and a higher score beats
/// a lower one. So a cluster's representative is its best-scored member, or (when
/// none is scored) its first member — exactly what "best member passes" needs.
fn is_better_representative(a: &FoundJob, b: &FoundJob) -> bool {
    match (a.score, b.score) {
        (Some(x), Some(y)) => x > y,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Cluster-aware minimum-score retention (ADR-029 §g): cluster the batch with
/// the SAME pass the annotation step uses, then keep EVERY member of a cluster
/// whose representative (best-scored member) clears `threshold` via
/// [`passes_min_score`]. A cluster with no scored member keeps today's
/// keep-unscored behavior. So a below-bar copy survives when a cluster-mate
/// scores well (it still carries a source chip + salary), and a weak member can
/// now "hide" behind a strong one — a deliberate loosening.
///
/// Returns the retained jobs together with THEIR clustering verdicts, in the
/// same order. The verdicts are computed here anyway, and phase 2 needs them to
/// spend one embed per cluster (on the member the UI will display) rather than
/// one per board copy — the alternative, clustering a second time downstream,
/// could disagree with this pass.
fn cluster_aware_retain(
    found_jobs: Vec<FoundJob>,
    threshold: f64,
    tombstones: &HashSet<(String, String)>,
    extra_agency: &[String],
) -> (
    Vec<FoundJob>,
    Vec<crate::scraping::cluster::ClusterAssignment>,
) {
    if found_jobs.is_empty() {
        return (found_jobs, Vec::new());
    }
    let inputs = crate::autopilot::found_job_cluster_inputs(&found_jobs);
    let assignments = crate::scraping::cluster::assign_clusters(inputs, tombstones, extra_agency);

    // The representative (best) member index per cluster.
    let mut rep_by_cluster: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for (i, assignment) in assignments.iter().enumerate() {
        let cid = assignment.cluster_id.as_str();
        match rep_by_cluster.get(cid).copied() {
            Some(cur) if !is_better_representative(&found_jobs[i], &found_jobs[cur]) => {}
            _ => {
                rep_by_cluster.insert(cid, i);
            }
        }
    }

    // A cluster passes iff its representative passes the per-member gate. Owned
    // ids: `assignments` is consumed by the zip below (its verdicts travel out
    // with the retained rows), so this set must not borrow from it.
    let passing: HashSet<String> = rep_by_cluster
        .iter()
        .filter(|&(_, &idx)| passes_min_score(&found_jobs[idx], threshold))
        .map(|(&cid, _)| cid.to_string())
        .collect();

    found_jobs
        .into_iter()
        .zip(assignments)
        .filter(|(_, assignment)| passing.contains(&assignment.cluster_id))
        .unzip()
}

/// Recompute + persist cluster annotations for one autopilot record after a
/// dedup split (`dedup_mark_not_duplicate` with an `autopilotId`). Snapshots the
/// current verdicts + extras and delegates to the store's per-record recompute.
pub(crate) fn recluster_autopilot_record(app: &AppHandle, autopilot_id: &str) {
    let (tombstones, extra_agency) = snapshot_dedup_inputs(app);
    store(app)
        .lock()
        .recompute_record_clusters(autopilot_id, &tombstones, &extra_agency);
}

#[cfg(test)]
mod tests;
