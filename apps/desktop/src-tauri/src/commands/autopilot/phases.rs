//! The optional and best-effort steps of a run, lifted out of `autopilot_run` so the pipeline reads
//! top to bottom: the step events, the phase-2 semantic re-rank, the AI notes and the LinkedIn
//! description enrichment. Each one degrades silently — none can fail a run.

use std::sync::Arc;

use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use super::rerank::{
    semantic_rerank_phase, LiveRerankEnv, RerankBudget, RerankSummary, RERANK_STEP_TIMEOUT,
    SEMANTIC_RERANK_MAX,
};
use super::store;
use crate::autopilot::{Autopilot, FoundJob};
use crate::events::{emit_event, AUTOPILOT_STEP};
use crate::observability::sanitize_reason;
use crate::scraping::JobPosting;

/// Emits the `autopilot:step` progress events of one run.
pub(super) struct StepEmitter {
    pub(super) app: AppHandle,
    pub(super) autopilot_id: String,
    pub(super) job_id: String,
}

impl StepEmitter {
    pub(super) fn emit(&self, step: &str, detail: &str) {
        emit_event(
            &self.app,
            AUTOPILOT_STEP,
            json!({ "jobId": self.job_id, "autopilotId": self.autopilot_id, "step": step, "detail": detail }),
        );
    }
}

/// Phase 2: re-rank the head of `found_jobs` through the combined semantic kernel, when (and only
/// when) the user has semantic scoring on. `None` means the phase did not run.
pub(super) async fn semantic_rerank_step(
    app: &AppHandle,
    steps: &StepEmitter,
    resume: &str,
    postings: &[JobPosting],
    found_jobs: &mut [FoundJob],
    clusters: &[crate::scraping::cluster::ClusterAssignment],
    cancel_token: &CancellationToken,
) -> Option<RerankSummary> {
    // ── Phase 2 (opt-in, ADR-020 addendum): semantic re-rank ──────────────────
    // Everything before this call is phase 1 — the free, embedding-free keyword prefilter,
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
    semantic_rerank_phase(
        semantic_on,
        resume,
        found_jobs,
        clusters,
        cancel_token,
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
            steps.emit(
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
                    app,
                    store: doc_store.inner(),
                    resume,
                    budget: RerankBudget::new(limiter.inner().clone(), active.provider.clone()),
                    active,
                },
                blobs,
            ))
        },
    )
    .await
}

/// Report what the re-rank did: emits the `rerank_timeout` step for a pass the wall clock cut off,
/// and returns the suffix the `rank_done` step appends (empty when the phase did not run).
pub(super) fn report_rerank(steps: &StepEmitter, rerank: Option<&RerankSummary>) -> String {
    // A timed-out pass reports its PARTIAL counts (it spent embeds and promoted
    // jobs — saying nothing would describe the run as keyword-only) plus a step
    // of its own, because "re-ranked 4 of 20" alone cannot say whether the other
    // 16 were skipped by the ceiling, the breaker, or the clock.
    if let Some(s) = rerank.filter(|s| s.timed_out) {
        steps.emit(
            "rerank_timeout",
            &format!(
                "Semantic re-rank ran out of time after {}s; {} of {} re-ranked, the rest stay keyword-only",
                RERANK_STEP_TIMEOUT.as_secs(),
                s.rescored,
                s.considered
            ),
        );
    }
    match rerank {
        Some(s) if s.timed_out => format!(
            "; semantic re-rank {}/{} before the time limit (kept keyword for the rest)",
            s.rescored, s.considered
        ),
        Some(s) => format!(
            "; semantic re-rank {}/{} (kept keyword for {})",
            s.rescored, s.considered, s.degraded
        ),
        None => String::new(),
    }
}

/// Phase 4: attach AI notes to the top new matches. Returns how many were generated.
pub(super) async fn assistant_notes_step(
    app: &AppHandle,
    autopilot: &Autopilot,
    found_jobs: &mut [FoundJob],
    cancel_token: &CancellationToken,
) -> usize {
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
        crate::pipeline::Completer::from_active(app)
            .inspect_err(|e| {
                log::info!(
                    "[autopilot] AI notes skipped: no usable provider ({})",
                    sanitize_reason(&e.to_string())
                )
            })
            .ok()
    } else {
        None
    };
    let limiter = app.state::<Arc<crate::limits::Limiter>>().inner().clone();

    crate::autopilot_helpers::generate_assistant_notes(
        completer.as_ref(),
        limiter,
        autopilot,
        found_jobs,
        &prior_keys,
        cancel_token,
    )
    .await
}

/// Hand the just-recorded LinkedIn rows that still lack a description to the background enrichment.
pub(super) fn spawn_linkedin_enrichment(app: &AppHandle, autopilot_id: &str) {
    // LinkedIn-only post-discovery description enrichment (issue #1114):
    // LinkedIn search results never carry a description (see
    // `linkedin::api_client`'s known gap, also documented on `no_jd_text`
    // above), so `build_found_job` above scored these title-only. Re-read the
    // just-persisted record (record_run consumed `found_jobs`, and the merge
    // it performs is what decides the FINAL per-job board/description this
    // run actually kept) and hand any LinkedIn rows still blank to a
    // best-effort background pass. Spawned via `tauri::async_runtime::spawn`
    // (fire-and-forget, never awaited here) right after `record_run` — a
    // slow/failing LinkedIn fetch can never delay this command's own return
    // or the "new jobs" notification below, regardless of which one the
    // detached task happens to still be running alongside — see
    // `linkedin_enrich`'s own doc for the failure-isolation and rate-limiting
    // details.
    if let Some(ap) = store(app).lock().get(autopilot_id) {
        let targets = crate::autopilot_helpers::linkedin_enrich::select_linkedin_enrichment_targets(
            &ap.found_jobs,
        );
        if !targets.is_empty() {
            let app_for_enrich = app.clone();
            tauri::async_runtime::spawn(async move {
                super::linkedin_enrich::enrich_linkedin_descriptions(app_for_enrich, targets).await;
            });
        }
    }
}
