use std::collections::HashSet;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use parking_lot::Mutex;

use crate::autopilot::{AutopilotFilter, AutopilotStatus, AutopilotStore, FoundJob, RunStatus};
use crate::autopilot_helpers::autopilot_scrape;
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

/// Whether `location` is non-empty after trimming — the only precondition for
/// attempting a geocode lookup. Pure (no network) so it's unit-tested directly.
fn should_derive_country_code(location: Option<&str>) -> bool {
    location.map(str::trim).is_some_and(|s| !s.is_empty())
}

/// Pick a country code out of the geocode service's ranked suggestions: the
/// first hit that actually CARRIES a VALID one wins (not just the first hit —
/// an earlier suggestion with an absent/null/malformed `countryCode` must not
/// block a usable later one), lower-cased to match
/// `BoardSearchInput::country_code`'s convention. Each candidate is validated
/// against the SAME 2-ASCII-letter shape `AutopilotTargetSchema.countryCode`
/// enforces (`^[A-Za-z]{2}$`) — a geocoded value written server-side bypasses
/// the IPC schema, so a `"USA"` / `"1a"`-style value is skipped, not accepted.
/// Pure — no network — so this is unit-tested directly; the HTTP round trip
/// inside `commands::geocoding::suggest` is not (no fixture for it).
fn country_code_from_suggestions(suggestions: &[Value]) -> Option<String> {
    suggestions
        .iter()
        .find_map(|s| {
            s.get("countryCode")
                .and_then(|v| v.as_str())
                .filter(|cc| cc.len() == 2 && cc.bytes().all(|b| b.is_ascii_alphabetic()))
        })
        .map(str::to_lowercase)
}

/// Cap the save-path geocode lookup tighter than `geocoding::suggest`'s own 5s
/// reqwest timeout: that 5s is SHARED with the interactive `geocode_suggest`
/// picker (where the user is waiting on the result and a longer wait is
/// acceptable), so it can't be lowered there. On the save path this lookup is a
/// best-effort backfill — `autopilot_create`/`autopilot_update` should not block
/// the user's save for up to 5s on a slow geocode. On timeout we fall through to
/// no suggestion (`None`), and the aggregator's guessed-market guard still
/// covers the residual case.
const SAVE_GEOCODE_TIMEOUT: Duration = Duration::from_secs(2);

/// Best-effort: when a target has a real `location` but no `country_code` (the
/// autopilot aggregator zero-jobs bug — a prefilled/typed location saved without
/// a geocode pick), look one up via the SAME geocode service the manual picker
/// uses (`commands::geocoding::suggest`) and backfill it before persisting.
/// Never blocks or fails the save: a network error / no match / timeout just
/// leaves `country_code` absent, exactly as it would without this fix — the
/// aggregator's own guessed-market guard (`scraping::boards::aggregator`)
/// covers that residual case too.
async fn derive_country_code(location: Option<&str>) -> Option<String> {
    if !should_derive_country_code(location) {
        return None;
    }
    // Safe: `should_derive_country_code` just proved this is `Some`.
    let location = location.unwrap_or_default().trim();
    // Cap the save-path lookup at SAVE_GEOCODE_TIMEOUT (see const above): a
    // timeout yields an empty Vec -> None (best-effort), never a slow save.
    let suggestions = tokio::time::timeout(
        SAVE_GEOCODE_TIMEOUT,
        crate::commands::geocoding::suggest(location),
    )
    .await
    .unwrap_or_default();
    country_code_from_suggestions(&suggestions)
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

    emit_step(
        &app,
        &job_id,
        "rank_done",
        &format!(
            "Keyword-matched {scored_count}/{total_found}; kept {kept} at or above {threshold:.0}% coverage (dropped {dropped})"
        ),
    );

    // Phase 4 (opt-in, headless, READ-ONLY): after the keyword rank, attach a
    // short AI-reasoned note to the top NEW matches. Bounded (≤ ASSISTANT_NOTES_MAX
    // provider calls, per-provider daily ceiling, cancellable mid-call, AND an
    // overall wall-clock timeout — see `generate_assistant_notes`) and best-effort —
    // a provider/config error just means no notes, never a failed run. `prior_urls`
    // (this record's pre-run found jobs) lets the step skip re-surfaced jobs, whose
    // notes the store merge preserves for free, so a steady-state run makes zero
    // provider calls. No-op unless `autopilot.assistant` is set. Runs BEFORE
    // `record_run`/`on_new_jobs` below, so the wall-clock timeout is what keeps a
    // hung provider from delaying the user-facing "new jobs" notification.
    let prior_urls: std::collections::HashSet<&str> = autopilot
        .found_jobs
        .iter()
        .map(|j| j.url.as_str())
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
        &prior_urls,
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
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn posting(title: &str, description: Option<&str>) -> JobPosting {
        JobPosting {
            id: "id".into(),
            external_id: None,
            title: title.into(),
            company: "co".into(),
            location: None,
            url: "https://example.com/job".into(),
            source: "test".into(),
            description: description.map(String::from),
            requirements: None,
            posted_at: None,
            captured_at: 0,
            extra: HashMap::new(),
        }
    }

    fn filter(keywords: Option<&[&str]>, exclude: Option<&[&str]>) -> AutopilotFilter {
        AutopilotFilter {
            min_match_score: 0.0,
            keywords: keywords.map(|v| v.iter().map(|s| s.to_string()).collect()),
            exclude_keywords: exclude.map(|v| v.iter().map(|s| s.to_string()).collect()),
        }
    }

    // ── country_code save-time derivation (autopilot aggregator zero-jobs fix) ──

    #[test]
    fn should_derive_country_code_requires_a_real_location() {
        assert!(should_derive_country_code(Some("London")));
        // Whitespace-only or absent location → nothing to geocode.
        assert!(!should_derive_country_code(Some("   ")));
        assert!(!should_derive_country_code(Some("")));
        assert!(!should_derive_country_code(None));
    }

    #[test]
    fn country_code_from_suggestions_takes_first_hit_lowercased() {
        let suggestions = vec![
            json!({ "display": "London, United Kingdom", "countryCode": "GB" }),
            json!({ "display": "London, Canada", "countryCode": "CA" }),
        ];
        assert_eq!(
            country_code_from_suggestions(&suggestions),
            Some("gb".to_string()),
            "must take the first (best-ranked) suggestion and lower-case it to \
             match BoardSearchInput::country_code's convention"
        );
    }

    #[test]
    fn country_code_from_suggestions_empty_or_missing_field_yields_none() {
        assert_eq!(country_code_from_suggestions(&[]), None);
        // A suggestion missing `countryCode` entirely (e.g. an ambiguous hit).
        let no_country = vec![json!({ "display": "Atlantis" })];
        assert_eq!(country_code_from_suggestions(&no_country), None);
    }

    #[test]
    fn country_code_from_suggestions_skips_a_leading_hit_with_no_country_code() {
        // The first (best-ranked) hit has no countryCode (absent AND explicit
        // null) — must not block a usable later suggestion.
        let absent_then_present = vec![
            json!({ "display": "Ambiguous place" }),
            json!({ "display": "Munich, Germany", "countryCode": "DE" }),
        ];
        assert_eq!(
            country_code_from_suggestions(&absent_then_present),
            Some("de".to_string()),
            "an absent countryCode on the first hit must not block a later, \
             usable suggestion"
        );

        let null_then_present = vec![
            json!({ "display": "Ambiguous place", "countryCode": null }),
            json!({ "display": "Munich, Germany", "countryCode": "DE" }),
        ];
        assert_eq!(
            country_code_from_suggestions(&null_then_present),
            Some("de".to_string()),
            "an explicit null countryCode on the first hit must not block a \
             later, usable suggestion"
        );
    }

    #[test]
    fn country_code_from_suggestions_skips_malformed_country_codes() {
        // A geocoded value written server-side bypasses the IPC schema's
        // `^[A-Za-z]{2}$` guard, so a leading 3-letter ("USA") or non-alpha
        // ("1a") countryCode must be SKIPPED in favour of a later valid hit —
        // not accepted (it would fail BoardSearchInput's 2-letter contract).
        let malformed_then_present = vec![
            json!({ "display": "United States", "countryCode": "USA" }),
            json!({ "display": "Nowhere", "countryCode": "1a" }),
            json!({ "display": "Munich, Germany", "countryCode": "DE" }),
        ];
        assert_eq!(
            country_code_from_suggestions(&malformed_then_present),
            Some("de".to_string()),
            "malformed leading countryCodes (3-letter, non-alpha) must be \
             skipped in favour of a valid 2-letter hit"
        );

        // All candidates malformed → None (nothing usable to backfill).
        let all_malformed = vec![
            json!({ "display": "United States", "countryCode": "USA" }),
            json!({ "display": "Nowhere", "countryCode": "1a" }),
            json!({ "display": "Solo", "countryCode": "G" }),
        ];
        assert_eq!(
            country_code_from_suggestions(&all_malformed),
            None,
            "an all-malformed list yields no country code"
        );
    }

    #[tokio::test]
    async fn derive_country_code_skips_geocode_when_location_absent() {
        // No location at all → must resolve to None WITHOUT attempting a
        // network call (a real HTTP hit here would make this test flaky/slow).
        assert_eq!(derive_country_code(None).await, None);
        assert_eq!(derive_country_code(Some("")).await, None);
        assert_eq!(derive_country_code(Some("   ")).await, None);
    }

    #[test]
    fn no_filters_keep_everything() {
        let p = posting("Rust Engineer", Some("We use Rust and Go"));
        assert!(matches_keyword_filters(&p, &filter(None, None)));
        // Empty lists are also a no-op.
        assert!(matches_keyword_filters(&p, &filter(Some(&[]), Some(&[]))));
    }

    #[test]
    fn must_include_requires_all_keywords() {
        let p = posting("Rust Engineer", Some("We use Rust and Kubernetes"));
        assert!(matches_keyword_filters(
            &p,
            &filter(Some(&["rust", "kubernetes"]), None)
        ));
        // Missing one required keyword → dropped.
        assert!(!matches_keyword_filters(
            &p,
            &filter(Some(&["rust", "elixir"]), None)
        ));
    }

    #[test]
    fn exclude_drops_on_any_match() {
        let p = posting("Senior PHP Developer", Some("Legacy PHP codebase"));
        assert!(!matches_keyword_filters(&p, &filter(None, Some(&["php"]))));
        assert!(matches_keyword_filters(
            &p,
            &filter(None, Some(&["python"]))
        ));
    }

    #[test]
    fn matching_is_case_insensitive_over_title_and_description() {
        let p = posting("Backend Role", Some("Postgres and REDIS"));
        // "Backend" only in title, "redis" only in description, different cases.
        assert!(matches_keyword_filters(
            &p,
            &filter(Some(&["Backend", "redis"]), None)
        ));
    }

    // Autopilot now ranks with the shared keyword-coverage kernel
    // (`documents::keywords::coverage_score`) — the same embedding-free ATS
    // sub-score the Jobs page uses — instead of the deleted Jaccard
    // `simple_similarity`. A résumé covering all the JD's keywords scores high; an
    // unrelated résumé scores 0; partial overlap lands strictly in between.
    #[test]
    fn ranking_uses_shared_keyword_coverage_kernel() {
        use crate::documents::keywords::coverage_score;

        // resume = description (all JD keywords covered) → full coverage.
        assert_eq!(
            coverage_score("rust kubernetes docker", "rust kubernetes docker"),
            100.0
        );
        // No overlapping keywords → 0.
        assert_eq!(coverage_score("rust", "java"), 0.0);
        // Résumé covers only part of the JD's keywords → strictly between.
        let partial = coverage_score("rust kubernetes", "rust kubernetes docker terraform");
        assert!(
            partial > 0.0 && partial < 100.0,
            "partial coverage must be strictly between 0 and 100; got {partial}"
        );
    }

    fn found(score: Option<f64>) -> FoundJob {
        FoundJob {
            title: "t".into(),
            company: "c".into(),
            url: "https://example.com/job".into(),
            location: None,
            board: None,
            description: None,
            salary_min: None,
            salary_max: None,
            salary_currency: None,
            score,
            score_provisional: false,
            found_at: 0,
            is_new: false,
            applied: false,
            trust: None,
            assistant_notes: None,
            cluster_id: None,
            cluster_canonical: true,
            cluster_members: Vec::new(),
            is_agency: false,
        }
    }

    #[test]
    fn min_score_gate_keeps_at_or_above_threshold() {
        assert!(passes_min_score(&found(Some(80.0)), 50.0));
        assert!(passes_min_score(&found(Some(50.0)), 50.0)); // boundary is inclusive
        assert!(!passes_min_score(&found(Some(49.9)), 50.0));
    }

    #[test]
    fn min_score_gate_keeps_unscored_jobs() {
        // No resume / no description → no score → never filtered out by the gate.
        assert!(passes_min_score(&found(None), 50.0));
        assert!(passes_min_score(&found(None), 100.0));
    }

    // ── cluster-aware retention (ADR-029 §g) ───────────────────────────────────

    #[test]
    fn cluster_aware_retain_keeps_below_bar_member_of_passing_cluster() {
        // Two board copies of the SAME job (same title+company, different urls)
        // form ONE cluster. The strong copy (80) clears the 50 bar, so the whole
        // cluster — including the below-bar (40) copy — is retained.
        let strong = FoundJob {
            url: "https://a.example.com/job".into(),
            score: Some(80.0),
            ..found(None)
        };
        let weak = FoundJob {
            url: "https://b.example.com/job".into(),
            score: Some(40.0),
            ..found(None)
        };
        let kept = cluster_aware_retain(vec![strong, weak], 50.0, &HashSet::new(), &[]);
        assert_eq!(
            kept.len(),
            2,
            "a below-bar member of a passing cluster must be kept"
        );
    }

    #[test]
    fn cluster_aware_retain_drops_a_failing_cluster() {
        // A lone scored job below the bar → its cluster fails → dropped.
        let weak = FoundJob {
            url: "https://c.example.com/job".into(),
            score: Some(40.0),
            ..found(None)
        };
        let kept = cluster_aware_retain(vec![weak], 50.0, &HashSet::new(), &[]);
        assert!(kept.is_empty(), "a below-bar singleton cluster is dropped");
    }

    #[test]
    fn cluster_aware_retain_keeps_fully_unscored_cluster() {
        let unscored = FoundJob {
            url: "https://d.example.com/job".into(),
            ..found(None)
        };
        let kept = cluster_aware_retain(vec![unscored], 50.0, &HashSet::new(), &[]);
        assert_eq!(
            kept.len(),
            1,
            "a fully-unscored cluster keeps the keep-unscored behavior"
        );
    }

    #[test]
    fn mixed_cluster_with_below_bar_scored_representative_is_dropped_even_with_unscored_member() {
        // Same job on two boards → ONE cluster. One copy scores 40 (below the 50
        // bar); the other is unscored. Per ADR-029 §g the cluster representative
        // is its best-SCORED member (40 < 50), so the WHOLE cluster is dropped —
        // the unscored member does NOT rescue it. Keep-unscored only applies to a
        // cluster with NO scored member at all.
        let scored_below = FoundJob {
            url: "https://a.example.com/job".into(),
            score: Some(40.0),
            ..found(None)
        };
        let unscored = FoundJob {
            url: "https://b.example.com/job".into(),
            ..found(None)
        };
        let kept = cluster_aware_retain(vec![scored_below, unscored], 50.0, &HashSet::new(), &[]);
        assert!(
            kept.is_empty(),
            "a below-bar scored representative drops the whole cluster, unscored member included"
        );
    }

    #[test]
    fn take_pending_focus_returns_buffered_id_then_clears() {
        let buf = crate::tray::PendingFocus(Mutex::new(Some("autopilot-123".to_string())));
        assert_eq!(take_pending_focus(&buf), Some("autopilot-123".to_string()));
        // Atomic take cleared the slot — a second pull (e.g. a later focus) is empty,
        // so a cold-start deep-link focus is delivered exactly once and can't re-fire.
        assert_eq!(take_pending_focus(&buf), None);
    }

    #[test]
    fn take_pending_focus_returns_none_when_empty() {
        let buf = crate::tray::PendingFocus(Mutex::new(None));
        assert_eq!(take_pending_focus(&buf), None);
    }

    // ── concurrent-run guard (item 2) ──────────────────────────────────────
    // Distinct ids per test isolate the process-global RUNS_IN_FLIGHT set from
    // the parallel test runner, so no #[serial] is needed.

    #[test]
    fn run_guard_blocks_a_second_concurrent_acquire() {
        let id = "guard-test-concurrent";
        let first = RunGuard::try_acquire(id).expect("first acquire succeeds");
        assert!(
            RunGuard::try_acquire(id).is_none(),
            "a second acquire for the same in-flight id is blocked (no double-run)"
        );
        drop(first);
        assert!(
            RunGuard::try_acquire(id).is_some(),
            "after the first guard drops, the id can be acquired again"
        );
    }

    #[test]
    fn run_guard_distinct_ids_do_not_block_each_other() {
        let _a = RunGuard::try_acquire("guard-test-a").expect("id a acquires");
        assert!(
            RunGuard::try_acquire("guard-test-b").is_some(),
            "different autopilot ids run concurrently — the guard is per-id"
        );
    }

    // ── snippet-score provisional flag (item 4) ────────────────────────────

    #[test]
    fn build_found_job_flags_aggregator_snippet_scores_as_provisional() {
        // An aggregator (Adzuna) posting is ranked over a truncated snippet, so
        // its score is provisional.
        let mut agg = posting("Rust Engineer", Some("We use Rust and Go"));
        agg.source = AGGREGATOR_SNIPPET_SOURCE.into();
        let job = build_found_job(&agg, "rust go", 0);
        assert!(job.score.is_some(), "a résumé + description yields a score");
        assert!(
            job.score_provisional,
            "an aggregator snippet score must be flagged provisional"
        );

        // A direct full-text board's score is authoritative — not provisional.
        let mut greenhouse = posting("Rust Engineer", Some("We use Rust and Go"));
        greenhouse.source = "greenhouse".into();
        let job = build_found_job(&greenhouse, "rust go", 0);
        assert!(job.score.is_some());
        assert!(
            !job.score_provisional,
            "a full-text board score must not be flagged provisional"
        );

        // No résumé → no score → nothing to qualify, even for an aggregator job.
        let mut agg_unscored = posting("Rust Engineer", Some("We use Rust"));
        agg_unscored.source = AGGREGATOR_SNIPPET_SOURCE.into();
        let job = build_found_job(&agg_unscored, "", 0);
        assert!(job.score.is_none());
        assert!(
            !job.score_provisional,
            "an unscored job is never provisional"
        );
    }
}
