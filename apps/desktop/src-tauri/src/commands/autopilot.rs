use std::collections::HashSet;
use std::sync::{Arc, LazyLock};

use parking_lot::Mutex;

use crate::autopilot::{
    AutopilotFilter, AutopilotStatus, AutopilotStore, FoundJob, RunStatus, ScoreSource,
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
    let ap = store(&app).lock().update(
        &autopilot_id,
        serde_json::to_value(&req).unwrap_or_default(),
    );
    json!(ap)
}

#[tauri::command]
pub fn autopilot_remove(app: AppHandle, autopilot_id: String) -> Value {
    store(&app).lock().remove(&autopilot_id);
    json!(null)
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

    // Snapshot each posting, scored 0–100 against the resume when one is set, then
    // sorted highest-first. The score is the keyword-coverage match % — the SAME
    // embedding-free kernel as the Jobs page's ATS sub-score (NOT the Jobs
    // *combined* %), so the headless scheduler never makes an embedding/API call.
    // Autopilot is a discovery agent: a run only finds, ranks by keyword coverage,
    // and saves results — the user applies with the tailoring assistant.
    let resume = autopilot.resume_text.as_deref().unwrap_or("");
    let found_at = now_ms();
    let mut found_jobs: Vec<FoundJob> = postings
        .iter()
        .map(|p| build_found_job(p, resume, found_at))
        .collect();

    // Highest keyword-coverage match first; unscored postings sort to the end.
    found_jobs.sort_by(|a, b| {
        b.score
            .unwrap_or(-1.0)
            .partial_cmp(&a.score.unwrap_or(-1.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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
    found_jobs = cluster_aware_retain(found_jobs, threshold, &tombstones, &extra_agency);
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
    // scores at all for it, so there is nothing to re-rank.
    let semantic_on = app
        .try_state::<crate::job_preferences::JobPreferencesStore>()
        .is_some_and(|s| s.semantic_scoring());
    let rerank = if !semantic_on || resume.is_empty() {
        None
    } else {
        // `try_state` (not `state`) for both stores: a run must never fail
        // because of scoring, and `state` PANICS on an unmanaged type. A
        // startup failure that left either store unregistered degrades this run
        // to keyword-only instead of unwinding a scheduled tick.
        match app
            .try_state::<crate::documents::DocumentStore>()
            .zip(app.try_state::<Arc<crate::limits::Limiter>>())
        {
            Some((doc_store, limiter)) => {
                // Reuse phase 1's EXACT scoring blob per posting — `FoundJob` drops
                // `requirements`, so re-deriving it here would score different text
                // than the keyword phase did on the boards that populate that field.
                let blobs: std::collections::HashMap<String, String> = postings
                    .iter()
                    .filter_map(|p| {
                        crate::documents::keywords::posting_text_blob(
                            &p.title,
                            p.description.as_deref(),
                            p.requirements.as_deref(),
                        )
                        .map(|blob| (p.url.clone(), blob))
                    })
                    .collect();
                let env = LiveRerankEnv {
                    app: &app,
                    store: doc_store.inner(),
                    resume,
                    active: doc_store.embedding_config(),
                    limiter: limiter.inner().clone(),
                };
                let summary = semantic_rerank(&env, &mut found_jobs, &blobs, &cancel_token).await;
                // Re-sort: phase 2 replaced the head's scores, so the keyword
                // ordering no longer holds. Same comparator as phase 1 (unscored
                // sorts last).
                found_jobs.sort_by(|a, b| {
                    b.score
                        .unwrap_or(-1.0)
                        .partial_cmp(&a.score.unwrap_or(-1.0))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                Some(summary)
            }
            None => {
                log::warn!(
                    "[autopilot] semantic re-rank skipped: document/limiter state unavailable; ranking stays keyword-only"
                );
                None
            }
        }
    };

    let rerank_detail = match &rerank {
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

/// How many of a run's top keyword-ranked jobs get the semantic re-rank.
///
/// Evidence for the number: a run's raw harvest is bounded per board (the
/// aggregator caps at 2 pages × `ADZUNA_PAGE_SIZE` = ≤100 postings, matching the
/// manual search's own 100-item amount cap), and by the time this step runs the
/// list has already been through the keyword filter, the user's `minMatchScore`
/// gate and cross-board cluster dedup — so a typical kept set is a few dozen.
/// Re-ranking the top 20 therefore covers the head of the list a user actually
/// scans and acts on, while bounding the worst case at 20 posting embeds + 1
/// résumé embed per run.
///
// ponytail: the real ceiling is cost, and it is already tiny — 21 embeds is a
// rounding error next to the ONE completion `ASSISTANT_NOTES_MAX` (3) guards,
// and the ADR-017 caches (`posting_vectors` + `match_scores`) make a steady-state
// repeat run cost ZERO embeds. This is a hard bound against a pathological
// harvest, not a tuning knob — raise it only with a measurement.
const SEMANTIC_RERANK_MAX: usize = 20;

/// Cache identity for an Autopilot posting in the ADR-017 caches.
///
/// Keyed on `canonical_job_key` — the SAME identity `merge_found_jobs` uses —
/// rather than the raw URL, so a job re-surfacing under different tracking
/// params hits the row it already paid for instead of re-embedding. Prefixed so
/// it can never collide with a real `PostingsCache` posting id (mirrors
/// `extension_bridge::match_live::adhoc_job_id`).
fn autopilot_job_id(job: &FoundJob) -> String {
    let key =
        crate::scraping::boards::common::canonical_job_key(&job.url, &job.title, &job.company);
    format!("autopilot:{}", crate::documents::sha256_hex(&key))
}

/// Outcome counts of one semantic re-rank pass — the content-free traceability
/// summary (counts only, never posting text) the run logs and reports.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RerankSummary {
    /// Jobs the keyword prefilter handed to phase 2 (≤ [`SEMANTIC_RERANK_MAX`]).
    pub(crate) considered: usize,
    /// Jobs whose score is now the combined semantic+ATS number.
    pub(crate) rescored: usize,
    /// Jobs that fell back to their keyword score (embed/provider failure, no
    /// scorable text, the daily ceiling, or cancellation).
    pub(crate) degraded: usize,
}

/// The re-rank's I/O seam — mirrors `autopilot_helpers`'s `NoteEnv`: the two
/// external effects (one combined-kernel score, which may embed, and the shared
/// daily-budget charge) sit behind a trait so the loop's control flow (top-N
/// ceiling, cancellation, daily-ceiling short-circuit, per-job degrade) is
/// unit-testable with a fake — there is no way to fake a live embedding
/// provider in-process. Prod wiring is [`LiveRerankEnv`].
#[async_trait::async_trait]
trait RerankEnv: Send + Sync {
    /// Score one posting through the SHARED combined kernel.
    ///
    /// `None` means "no semantic score for this job" — an embed that failed, a
    /// provider that is offline, or an error object from the kernel. The caller
    /// must then keep the job's keyword score: a run NEVER fails because of
    /// scoring.
    async fn score(&self, job_id: &str, job_text: String) -> Option<f64>;
    /// Charge one embedding call against the SAME per-provider daily ceiling as
    /// interactive AI — no parallel budget architecture (the `NoteEnv::charge_daily`
    /// precedent). Acceptable because a run charges at most
    /// [`SEMANTIC_RERANK_MAX`] against it.
    fn charge_daily(&self) -> crate::error::AppResult<()>;
}

/// Extract a usable SEMANTIC score from a `score_one` result — the degrade
/// boundary, kept pure so it is directly testable against realistic kernel
/// output.
///
/// `None` unless the kernel itself reports `scoreSource: "combined"`. The
/// `combined` NUMBER alone is not sufficient evidence: `score_one` degrades to
/// `combined == ats` when no embedding vector is available, and that keyword
/// number is exactly what the job already carries — promoting it would relabel a
/// keyword score as semantic and lie to the user. An error object (`{"error":…}`)
/// and a `scoreSource`-less legacy cache row likewise degrade.
fn rerank_score_from(result: &Value) -> Option<f64> {
    if result.get("scoreSource").and_then(Value::as_str)
        != Some(crate::commands::match_resume::SCORE_SOURCE_COMBINED)
    {
        return None;
    }
    result.get("combined").and_then(Value::as_f64)
}

/// Production [`RerankEnv`]: the shared combined kernel + the shared limiter.
struct LiveRerankEnv<'a> {
    app: &'a AppHandle,
    store: &'a crate::documents::DocumentStore,
    resume: &'a str,
    active: crate::documents::EmbeddingConfig,
    limiter: Arc<crate::limits::Limiter>,
}

#[async_trait::async_trait]
impl RerankEnv for LiveRerankEnv<'_> {
    async fn score(&self, job_id: &str, job_text: String) -> Option<f64> {
        let result = crate::commands::match_resume::score_autopilot_semantic(
            self.app,
            self.store,
            self.resume,
            &self.active,
            job_id,
            job_text,
        )
        .await;
        rerank_score_from(&result)
    }

    fn charge_daily(&self) -> crate::error::AppResult<()> {
        // The EMBEDDING provider is the one being billed here (not the
        // generation provider the AI-notes step charges), so the counter is
        // keyed on it.
        self.limiter
            .charge_provider_daily(&self.active.provider, crate::limits::PROVIDER_DAILY_MAX)
    }
}

/// Phase 2 of the two-phase rank: re-score the top [`SEMANTIC_RERANK_MAX`] jobs
/// of the (already keyword-ranked, filtered and deduped) list through the shared
/// combined kernel, in place.
///
/// `blobs` maps a job url to the EXACT scoring blob phase 1 used, so the two
/// phases can never score different text for the same posting (`FoundJob` drops
/// `requirements`, so re-deriving the blob here would silently diverge on the
/// boards that populate it).
///
/// Degrade contract — a run never fails because of scoring:
/// - a job with no keyword score (no résumé / no scorable text) is skipped
///   entirely: there is nothing to re-rank and no reason to spend an embed;
/// - a job whose scoring returns `None` keeps its keyword score AND its
///   `Keyword` label, and the loop moves on to the next job;
/// - the daily ceiling and cancellation stop the loop, leaving every
///   not-yet-visited job on its keyword score.
///
/// Split from the command (mirroring `run_notes_loop`) so a fake `env` unit-tests
/// this control flow without a provider.
async fn semantic_rerank(
    env: &dyn RerankEnv,
    found_jobs: &mut [FoundJob],
    blobs: &std::collections::HashMap<String, String>,
    cancel: &CancellationToken,
) -> RerankSummary {
    let mut summary = RerankSummary::default();
    // Cache identities already re-ranked THIS run. A single run can surface the
    // same job under two URL variants (the cluster/merge pass that collapses
    // them runs later, in `record_run`), and both derive the SAME
    // `autopilot_job_id` — so the second would burn a top-N slot to recompute a
    // score the first already produced. Same guard, same reason, as
    // `run_notes_loop`'s `seen_this_run`.
    let mut seen_this_run: HashSet<String> = HashSet::new();
    for job in found_jobs.iter_mut() {
        if summary.considered >= SEMANTIC_RERANK_MAX {
            break; // top-N ceiling — the hard cost bound
        }
        // An unscored job had no résumé or no extractable text in phase 1;
        // neither is fixed by an embedding. Not counted as considered OR
        // degraded — it was never a re-rank candidate.
        if job.score.is_none() {
            continue;
        }
        let Some(job_text) = blobs.get(&job.url).cloned() else {
            continue; // same reasoning: no phase-1 blob means nothing to score
        };
        let cache_id = autopilot_job_id(job);
        if !seen_this_run.insert(cache_id.clone()) {
            // A different URL variant of this same job already ran this pass.
            // Checked BEFORE `charge_daily` so the duplicate also can't burn the
            // shared per-provider ceiling.
            continue;
        }
        summary.considered += 1;
        if cancel.is_cancelled() {
            summary.degraded += 1;
            break; // stopped by the user — the keyword score stands
        }
        if let Err(e) = env.charge_daily() {
            log::info!("[autopilot] semantic re-rank stopped at daily ceiling: {e}");
            summary.degraded += 1;
            break;
        }
        match env.score(&cache_id, job_text).await {
            Some(combined) => {
                job.score = Some(combined);
                job.score_source = ScoreSource::Combined;
                summary.rescored += 1;
            }
            // Per-job degrade: keep the keyword score and the `Keyword` label,
            // keep going. One offline embed must not cost the whole run.
            None => summary.degraded += 1,
        }
    }
    log::info!(
        "[autopilot] semantic re-rank: considered {} (max {SEMANTIC_RERANK_MAX}), rescored {}, degraded to keyword-only {}",
        summary.considered,
        summary.rescored,
        summary.degraded
    );
    summary
}

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
fn cluster_aware_retain(
    found_jobs: Vec<FoundJob>,
    threshold: f64,
    tombstones: &HashSet<(String, String)>,
    extra_agency: &[String],
) -> Vec<FoundJob> {
    if found_jobs.is_empty() {
        return found_jobs;
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

    // A cluster passes iff its representative passes the per-member gate.
    let passing: HashSet<&str> = rep_by_cluster
        .iter()
        .filter(|(_, &idx)| passes_min_score(&found_jobs[idx], threshold))
        .map(|(&cid, _)| cid)
        .collect();

    found_jobs
        .into_iter()
        .zip(assignments.iter())
        .filter_map(|(job, assignment)| {
            passing
                .contains(assignment.cluster_id.as_str())
                .then_some(job)
        })
        .collect()
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
