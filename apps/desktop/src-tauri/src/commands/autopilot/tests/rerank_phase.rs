//! The phase-2 entry point (`semantic_rerank_phase`): the gate, the off-by-default guarantee, the wall
//! clock and the budget.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::super::rerank::*;
use super::support::*;
use crate::autopilot::ScoreSource;

// ── the phase-2 gate (the REAL one) ───────────────────────────────────────

/// The production gate itself. Semantic OFF is the default, so `false` here is
/// what keeps a scheduled run embedding-free; a résumé-less autopilot has no
/// phase-1 scores to re-rank at all.
#[test]
fn the_semantic_gate_needs_both_the_preference_and_a_resume() {
    assert!(should_semantic_rerank(true, "rust engineer, kubernetes"));
    assert!(
        !should_semantic_rerank(false, "rust engineer, kubernetes"),
        "the preference OFF (the default) is what keeps a scheduled run embedding-free"
    );
    assert!(!should_semantic_rerank(true, ""));
    assert!(
        !should_semantic_rerank(true, "  \n\t "),
        "a whitespace-only résumé is no résumé"
    );
}

/// Semantic OFF is the load-bearing regression: a scheduled run must stay the
/// pre-existing embedding-free pipeline. This drives the REAL production
/// function (`semantic_rerank_phase`, which owns the gate and the setup) — with
/// a counting env proving ZERO calls AND a setup closure proving the run does
/// not even resolve the scoring state or build the blob map.
#[tokio::test]
async fn semantic_off_never_resolves_the_rerank_env_and_makes_zero_scoring_calls() {
    let mut jobs = vec![
        ranked("https://example.com/a", 80.0),
        ranked("https://example.com/b", 60.0),
    ];
    let before = jobs.clone();
    let env = std::sync::Arc::new(FakeRerankEnv::scoring_all(&jobs, 99.0));
    let blobs = blobs_for(&jobs);
    let setup_ran = std::sync::atomic::AtomicBool::new(false);

    let summary = semantic_rerank_phase(
        false, // the user's preference: semantic scoring OFF
        "rust engineer, kubernetes",
        &mut jobs,
        NO_CLUSTERS,
        &CancellationToken::new(),
        |_candidates| {
            setup_ran.store(true, std::sync::atomic::Ordering::SeqCst);
            Some((std::sync::Arc::clone(&env), blobs.clone()))
        },
    )
    .await;

    assert!(summary.is_none());
    assert!(
        !setup_ran.load(std::sync::atomic::Ordering::SeqCst),
        "with the preference off the run must not even resolve the scoring state \
         or build the blob map — the gate has to precede the setup"
    );
    assert_eq!(
        env.calls(),
        0,
        "a scheduled keyword-only run makes no embed calls"
    );
    assert_eq!(env.charges(), 0);
    assert_eq!(
        jobs.iter().map(|j| j.score).collect::<Vec<_>>(),
        before.iter().map(|j| j.score).collect::<Vec<_>>()
    );
    assert!(jobs.iter().all(|j| j.score_source == ScoreSource::Keyword));
}

/// …and the same real function DOES re-rank when the gate passes, so the test
/// above cannot be satisfied by a gate that is stuck closed.
#[tokio::test]
async fn semantic_on_runs_the_phase_through_the_same_entry_point() {
    let mut jobs = vec![ranked("https://example.com/a", 80.0)];
    let env = std::sync::Arc::new(FakeRerankEnv::new(vec![(autopilot_job_id(&jobs[0]), 42.0)]));
    let blobs = blobs_for(&jobs);

    let summary = semantic_rerank_phase(
        true,
        "rust engineer, kubernetes",
        &mut jobs,
        NO_CLUSTERS,
        &CancellationToken::new(),
        |_candidates| Some((std::sync::Arc::clone(&env), blobs.clone())),
    )
    .await;

    assert_eq!(summary.map(|s| s.rescored), Some(1));
    assert_eq!(env.calls(), 1);
    assert_eq!(jobs[0].score, Some(42.0));
    assert_eq!(jobs[0].score_source, ScoreSource::Combined);
}

/// A wall-clock ceiling, like the neighbouring AI-notes step: phase 2 runs
/// BEFORE `record_run`/`on_new_jobs` and only checks cancellation BETWEEN jobs,
/// so a hung provider must not delay the "new jobs" notification unboundedly.
/// The degrade is per job: whatever was scored before the deadline is KEPT, the
/// rest stay keyword-only, and the run continues.
///
/// …and the pass REPORTS what it did. A timed-out phase has already spent embeds
/// and promoted jobs, so returning no summary made `rank_done` read exactly like
/// a keyword-only run — the one shape where the log actively misdescribes the
/// work. The partial counts plus the `timed_out` flag (which the command turns
/// into its own `rerank_timeout` step) are what tell the two apart: the counts
/// alone cannot say whether the untouched tail was skipped by the ceiling, the
/// breaker, or the clock.
#[tokio::test(start_paused = true)]
async fn the_rerank_phase_gives_up_on_the_wall_clock_and_keeps_the_rest_keyword_only() {
    let mut jobs = vec![
        ranked("https://example.com/a", 80.0),
        ranked("https://example.com/b", 60.0),
    ];
    // Each score takes two thirds of the budget: the first finishes, the second
    // is still in flight when the deadline passes.
    let env = std::sync::Arc::new(
        FakeRerankEnv::new(vec![
            (autopilot_job_id(&jobs[0]), 95.0),
            (autopilot_job_id(&jobs[1]), 90.0),
        ])
        .with_delay(RERANK_STEP_TIMEOUT * 2 / 3),
    );
    let blobs = blobs_for(&jobs);

    let summary = semantic_rerank_phase(
        true,
        "rust engineer",
        &mut jobs,
        NO_CLUSTERS,
        &CancellationToken::new(),
        |_candidates| Some((std::sync::Arc::clone(&env), blobs.clone())),
    )
    .await;

    let summary = summary.expect(
        "a pass that spent embeds and promoted a job must report it — reporting \
         nothing describes the run as keyword-only",
    );
    assert_eq!(
        summary,
        RerankSummary {
            // The second job was in flight at the deadline: counted as
            // considered by the loop, never resolved either way.
            considered: 2,
            rescored: 1,
            degraded: 0,
            timed_out: true,
        },
        "the counts must be the PARTIAL ones as of the cutoff, and the cutoff \
         itself must be visible — that is what the `rerank_timeout` step reports"
    );
    assert_eq!(
        jobs[0].score,
        Some(95.0),
        "work done before the deadline is kept"
    );
    assert_eq!(jobs[0].score_source, ScoreSource::Combined);
    assert_eq!(
        jobs[1].score,
        Some(60.0),
        "the job cut off by the deadline degrades to keyword-only, it does not fail the run"
    );
    assert_eq!(jobs[1].score_source, ScoreSource::Keyword);
}

// ── budget: charge per ACTUAL provider round-trip ─────────────────────────

/// The seam's contract: a job the caches already answer is re-ranked WITHOUT a
/// daily charge. Charging per considered job (the old shape) billed a
/// steady-state repeat run for up to `SEMANTIC_RERANK_MAX` embeds it never
/// makes.
#[tokio::test]
async fn a_cached_job_is_re_ranked_without_charging_the_daily_budget() {
    let mut jobs = vec![
        ranked("https://example.com/a", 80.0),
        ranked("https://example.com/b", 60.0),
    ];
    let ids: Vec<String> = jobs.iter().map(autopilot_job_id).collect();
    let env = FakeRerankEnv::new(ids.iter().map(|id| (id.clone(), 91.0)).collect::<Vec<_>>())
        .with_cached(ids);

    let summary = rerank_jobs(&env, &mut jobs).await;

    assert_eq!(summary.rescored, 2);
    assert_eq!(env.calls(), 2);
    assert_eq!(
        env.charges(),
        0,
        "a repeat run that hits the ADR-017 caches must cost NOTHING against the \
         shared per-provider daily ceiling"
    );
}

/// The production budget object the kernel is handed. It charges the SHARED
/// per-provider ceiling once per call and, on a refusal, latches the flag
/// `LiveRerankEnv::score` reads to turn a keyword-only degrade into
/// `BudgetExhausted` — the only thing that stops the loop.
///
/// There is no charge PREDICATE left to test: the charge is made by the call
/// that reaches the provider, on the bytes it consumes (see
/// `commands::match_resume::test`'s cache/charge pins, which drive the real
/// kernel against a real store).
#[test]
fn the_rerank_budget_charges_the_shared_ceiling_and_latches_its_refusal() {
    use crate::documents::EmbedBudget;

    let limiter = Arc::new(crate::limits::Limiter::new());
    let budget = RerankBudget::new(Arc::clone(&limiter), "ollama".to_string());

    assert!(!budget.is_exhausted());
    budget
        .charge_one_embed()
        .expect("first embed is affordable");
    assert!(
        !budget.is_exhausted(),
        "an accepted charge must not stop the loop"
    );

    // Drain the shared daily ceiling through the SAME limiter the interactive
    // paths use — a parallel budget would defeat the point.
    for _ in 1..crate::limits::PROVIDER_DAILY_MAX {
        limiter
            .charge_provider_daily("ollama", crate::limits::PROVIDER_DAILY_MAX)
            .unwrap();
    }

    assert!(budget.charge_one_embed().is_err(), "the ceiling is reached");
    assert!(
        budget.is_exhausted(),
        "the refusal must be visible to the loop: a refused embed degrades the job \
         to keyword-only exactly like an offline provider, so the outcome alone \
         cannot tell the loop to stop"
    );
}
