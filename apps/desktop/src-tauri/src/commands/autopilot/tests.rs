//! Unit tests for `commands::autopilot` — the keyword filters, the cluster-aware
//! retain, the run guard, and the phase-2 semantic re-rank.
//!
//! Split into a sibling file (the `commands/geocoding.rs` + `commands/geocoding/`
//! precedent) purely to keep the parent module under R8's LOC cap; nothing about
//! the tests themselves changed in the move.

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

// The `country_code` save-time derivation tests moved to
// `commands::geocoding` with the helpers themselves (they are now shared
// with the manual scrape path).

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
        score_source: ScoreSource::Keyword,
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
    let (kept, _) = cluster_aware_retain(vec![strong, weak], 50.0, &HashSet::new(), &[]);
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
    let (kept, _) = cluster_aware_retain(vec![weak], 50.0, &HashSet::new(), &[]);
    assert!(kept.is_empty(), "a below-bar singleton cluster is dropped");
}

#[test]
fn cluster_aware_retain_keeps_fully_unscored_cluster() {
    let unscored = FoundJob {
        url: "https://d.example.com/job".into(),
        ..found(None)
    };
    let (kept, _) = cluster_aware_retain(vec![unscored], 50.0, &HashSet::new(), &[]);
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
    let (kept, _) = cluster_aware_retain(vec![scored_below, unscored], 50.0, &HashSet::new(), &[]);
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

// ── Phase 2: semantic re-rank (ADR-020 addendum) ──────────────────────────

/// Scriptable [`RerankEnv`] fake. Counts every scoring call and every daily
/// charge, so a test can pin "the scheduled path made ZERO scoring calls"
/// rather than only asserting on the resulting scores (which a broken
/// implementation could reproduce by accident).
///
/// It models the production seam faithfully on the one axis that matters for
/// budget: a job the ADR-017 caches already answer is scored WITHOUT a charge
/// (`LiveRerankEnv` decides that with the kernel's own
/// `documents::posting_vector_is_fresh` — see
/// `a_cached_posting_vector_means_no_provider_round_trip`, which pins the real
/// predicate against a real store).
struct FakeRerankEnv {
    /// url-independent: keyed by the derived cache job id → the score to
    /// return. A missing entry models "no semantic score available"
    /// (embed failed / provider offline) → the degrade path.
    scores: std::sync::Mutex<HashMap<String, f64>>,
    /// Job ids whose score comes from cache: no provider round-trip, so no
    /// daily charge.
    cached: HashSet<String>,
    calls: std::sync::atomic::AtomicUsize,
    charges: std::sync::atomic::AtomicUsize,
    /// When `Some(n)`, the charge fails from the n-th round-trip onward —
    /// models hitting the shared per-provider daily ceiling mid-run.
    charge_fails_after: Option<usize>,
    /// Per-call latency, for the wall-clock-timeout test. Applied AFTER the
    /// budget check, like a real provider call.
    delay: Option<std::time::Duration>,
}

impl FakeRerankEnv {
    fn new(scores: Vec<(String, f64)>) -> Self {
        Self {
            scores: std::sync::Mutex::new(scores.into_iter().collect()),
            cached: HashSet::new(),
            calls: std::sync::atomic::AtomicUsize::new(0),
            charges: std::sync::atomic::AtomicUsize::new(0),
            charge_fails_after: None,
            delay: None,
        }
    }
    /// Mark these job ids as already cached — scored, but with no round-trip.
    fn with_cached(mut self, ids: impl IntoIterator<Item = String>) -> Self {
        self.cached = ids.into_iter().collect();
        self
    }
    fn with_delay(mut self, delay: std::time::Duration) -> Self {
        self.delay = Some(delay);
        self
    }
    /// Scoring calls that actually ran (i.e. got past the budget check).
    fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
    /// Charges against the shared per-provider daily ceiling.
    fn charges(&self) -> usize {
        self.charges.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl RerankEnv for FakeRerankEnv {
    async fn score(&self, job_id: &str, _job_text: String) -> RerankOutcome {
        if !self.cached.contains(job_id) {
            let n = self
                .charges
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            if self.charge_fails_after.is_some_and(|limit| n > limit) {
                return RerankOutcome::BudgetExhausted;
            }
        }
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        // The lock is never held across the await above.
        let scored = self.scores.lock().unwrap().get(job_id).copied();
        match scored {
            Some(s) => RerankOutcome::Scored(s),
            None => RerankOutcome::Degraded,
        }
    }
}

/// Lets a test keep a handle on the fake while `semantic_rerank_phase`'s
/// `setup` closure hands one over by value.
#[async_trait::async_trait]
impl RerankEnv for std::sync::Arc<FakeRerankEnv> {
    async fn score(&self, job_id: &str, job_text: String) -> RerankOutcome {
        <FakeRerankEnv as RerankEnv>::score(self, job_id, job_text).await
    }
}

/// No clustering verdicts — the per-job identity fallback. Used by the loop
/// tests that are not about clustering.
const NO_CLUSTERS: &[crate::scraping::cluster::ClusterAssignment] = &[];

/// Run the re-rank loop and hand back its summary.
///
/// `semantic_rerank` accumulates into a caller-owned summary (so a pass the wall
/// clock cuts off still reports its partial counts); these loop tests care about
/// the completed pass, so the accumulator is a local detail here.
async fn rerank_all(
    env: &dyn RerankEnv,
    found_jobs: &mut [FoundJob],
    clusters: &[crate::scraping::cluster::ClusterAssignment],
    blobs: &HashMap<String, String>,
    cancel: &CancellationToken,
) -> RerankSummary {
    let mut summary = RerankSummary::default();
    semantic_rerank(env, found_jobs, clusters, blobs, cancel, &mut summary).await;
    summary
}

/// Build a scored job at `url` with a phase-1 keyword score.
fn ranked(url: &str, score: f64) -> FoundJob {
    FoundJob {
        url: url.into(),
        score: Some(score),
        ..found(None)
    }
}

/// The blob map `autopilot_run` builds: phase 1's exact scoring text per url.
fn blobs_for(jobs: &[FoundJob]) -> HashMap<String, String> {
    jobs.iter()
        .map(|j| (j.url.clone(), format!("jd text for {}", j.url)))
        .collect()
}

#[tokio::test]
async fn semantic_rerank_reorders_the_head_through_the_combined_kernel() {
    // Phase 1 (keyword) order: A(80) > B(60) > C(40).
    let mut jobs = vec![
        ranked("https://example.com/a", 80.0),
        ranked("https://example.com/b", 60.0),
        ranked("https://example.com/c", 40.0),
    ];
    // Phase 2 (combined) disagrees: C is the strongest semantic match.
    let env = FakeRerankEnv::new(vec![
        (autopilot_job_id(&jobs[0]), 55.0),
        (autopilot_job_id(&jobs[1]), 70.0),
        (autopilot_job_id(&jobs[2]), 95.0),
    ]);
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;
    // The command re-sorts after the re-rank; mirror that here so the test
    // pins ORDER, not just the numbers.
    jobs.sort_by(|a, b| b.score.unwrap().partial_cmp(&a.score.unwrap()).unwrap());

    assert_eq!(
        summary,
        RerankSummary {
            considered: 3,
            rescored: 3,
            degraded: 0,
            timed_out: false
        }
    );
    assert_eq!(
        jobs.iter().map(|j| j.url.as_str()).collect::<Vec<_>>(),
        vec![
            "https://example.com/c",
            "https://example.com/b",
            "https://example.com/a"
        ],
        "the semantic re-rank must be able to OVERTURN the keyword order — \
         pinning the exact inversion, not merely that scores changed"
    );
    assert_eq!(
        jobs.iter().map(|j| j.score.unwrap()).collect::<Vec<_>>(),
        vec![95.0, 70.0, 55.0]
    );
    assert!(
        jobs.iter().all(|j| j.score_source == ScoreSource::Combined),
        "a re-ranked job must be labelled Combined so the UI does not call a \
         semantic number 'keyword coverage'"
    );
}

#[tokio::test]
async fn semantic_rerank_degrades_that_job_only_and_the_run_completes() {
    let mut jobs = vec![
        ranked("https://example.com/a", 80.0),
        ranked("https://example.com/b", 60.0),
        ranked("https://example.com/c", 40.0),
    ];
    // B has no entry → its embed "failed". A and C still score.
    let env = FakeRerankEnv::new(vec![
        (autopilot_job_id(&jobs[0]), 90.0),
        (autopilot_job_id(&jobs[2]), 70.0),
    ]);
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(
        summary,
        RerankSummary {
            considered: 3,
            rescored: 2,
            degraded: 1,
            timed_out: false
        }
    );
    // The failure did NOT abort the loop: the job AFTER the failure still ran.
    assert_eq!(
        env.calls(),
        3,
        "one job's embed failure must not stop the run"
    );
    // The degraded job keeps its phase-1 keyword score AND its keyword label.
    assert_eq!(jobs[1].score, Some(60.0));
    assert_eq!(jobs[1].score_source, ScoreSource::Keyword);
    // Its neighbours are re-ranked normally.
    assert_eq!(jobs[0].score, Some(90.0));
    assert_eq!(jobs[0].score_source, ScoreSource::Combined);
    assert_eq!(jobs[2].score, Some(70.0));
    assert_eq!(jobs[2].score_source, ScoreSource::Combined);
}

#[tokio::test]
async fn semantic_rerank_leaves_the_provisional_flag_untouched() {
    // `score_provisional` describes WHERE the scored text came from (a
    // truncated aggregator snippet), which a re-rank does not change: the
    // semantic score is computed over that same truncated blob.
    let mut jobs = vec![FoundJob {
        score_provisional: true,
        ..ranked("https://example.com/a", 30.0)
    }];
    let env = FakeRerankEnv::new(vec![(autopilot_job_id(&jobs[0]), 88.0)]);
    let blobs = blobs_for(&jobs);

    rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(jobs[0].score, Some(88.0));
    assert!(
        jobs[0].score_provisional,
        "a snippet-derived score stays provisional after a semantic re-rank"
    );
}

#[tokio::test]
async fn semantic_rerank_never_scores_an_unscored_job() {
    // No résumé / no extractable text in phase 1 → no score. An embedding
    // cannot fix either, so the job must not cost a call OR a daily charge.
    let mut jobs = vec![found(None), ranked("https://example.com/b", 50.0)];
    jobs[0].url = "https://example.com/a".into();
    let env = FakeRerankEnv::new(vec![(autopilot_job_id(&jobs[1]), 77.0)]);
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(summary.considered, 1, "only the scored job is a candidate");
    assert_eq!(env.calls(), 1);
    assert_eq!(env.charges(), 1);
    assert_eq!(jobs[0].score, None);
    assert_eq!(jobs[0].score_source, ScoreSource::Keyword);
}

#[tokio::test]
async fn semantic_rerank_stops_at_the_top_n_ceiling() {
    // One more candidate than the ceiling allows: the tail keeps its keyword
    // score, untouched and uncharged.
    let mut jobs: Vec<FoundJob> = (0..SEMANTIC_RERANK_MAX + 5)
        .map(|i| ranked(&format!("https://example.com/{i}"), 50.0))
        .collect();
    let env = FakeRerankEnv::new(
        jobs.iter()
            .map(|j| (autopilot_job_id(j), 99.0))
            .collect::<Vec<_>>(),
    );
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(summary.considered, SEMANTIC_RERANK_MAX);
    assert_eq!(summary.rescored, SEMANTIC_RERANK_MAX);
    assert_eq!(
        env.calls(),
        SEMANTIC_RERANK_MAX,
        "the ceiling bounds real calls, not just the reported count"
    );
    assert_eq!(
        jobs[SEMANTIC_RERANK_MAX].score_source,
        ScoreSource::Keyword,
        "beyond the ceiling a job keeps its keyword score and label"
    );
}

#[tokio::test]
async fn semantic_rerank_charges_the_daily_ceiling_and_stops_when_it_is_hit() {
    let mut jobs = vec![
        ranked("https://example.com/a", 80.0),
        ranked("https://example.com/b", 60.0),
        ranked("https://example.com/c", 40.0),
    ];
    let mut env = FakeRerankEnv::new(
        jobs.iter()
            .map(|j| (autopilot_job_id(j), 99.0))
            .collect::<Vec<_>>(),
    );
    env.charge_fails_after = Some(2); // the 3rd charge is refused
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(
        env.charges(),
        3,
        "every embed charges the shared per-provider daily counter BEFORE it runs"
    );
    assert_eq!(
        env.calls(),
        2,
        "a refused charge must prevent the call, not merely be logged after it"
    );
    assert_eq!(summary.rescored, 2);
    assert_eq!(summary.degraded, 1);
    assert_eq!(
        jobs[2].score,
        Some(40.0),
        "the run still completes; the unscored tail keeps its keyword score"
    );
    assert_eq!(jobs[2].score_source, ScoreSource::Keyword);
}

/// An offline embedding provider degrades EVERY job, so the per-job degrade
/// contract alone spends a full phase — up to `SEMANTIC_RERANK_MAX` provider
/// timeouts — on every scheduled run to produce nothing. After
/// `RERANK_DEGRADE_BREAKER` consecutive degrades the provider is plainly down
/// and the pass stops; the rest of the list keeps its keyword scores, which is
/// the ordinary degrade.
#[tokio::test]
async fn a_run_of_consecutive_degrades_stops_the_phase() {
    let mut jobs: Vec<FoundJob> = (0..RERANK_DEGRADE_BREAKER + 4)
        .map(|i| ranked(&format!("https://example.com/{i}"), 50.0))
        .collect();
    // Only the LAST job is scriptable — the provider is "down" for every job
    // before it, so a pass without a breaker would walk the whole list.
    let env = FakeRerankEnv::new(vec![(
        autopilot_job_id(jobs.last().expect("non-empty")),
        99.0,
    )]);
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(
        env.calls(),
        RERANK_DEGRADE_BREAKER,
        "an offline provider must cost a bounded number of attempts, not one per job"
    );
    assert_eq!(summary.degraded, RERANK_DEGRADE_BREAKER);
    assert_eq!(summary.rescored, 0);
    assert!(
        jobs.iter().all(|j| j.score_source == ScoreSource::Keyword),
        "a stopped pass leaves every job on its keyword score — the run still completes"
    );
    assert_eq!(
        jobs.last().and_then(|j| j.score),
        Some(50.0),
        "the unvisited tail is untouched, exactly as for the daily ceiling"
    );
}

/// …and the counter is CONSECUTIVE, not cumulative: an unscorable posting
/// between healthy ones must not close the breaker on a working provider.
#[tokio::test]
async fn isolated_degrades_never_stop_a_healthy_pass() {
    // Alternating degrade / success, with more degrades in total than the
    // breaker allows — but never two in a row.
    let mut jobs: Vec<FoundJob> = (0..2 * RERANK_DEGRADE_BREAKER + 2)
        .map(|i| ranked(&format!("https://example.com/{i}"), 50.0))
        .collect();
    let env = FakeRerankEnv::new(
        jobs.iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, j)| (autopilot_job_id(j), 88.0))
            .collect::<Vec<_>>(),
    );
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(
        env.calls(),
        jobs.len(),
        "every job is still visited: a success resets the consecutive counter"
    );
    assert!(
        summary.degraded > RERANK_DEGRADE_BREAKER,
        "test premise: more total degrades than the breaker, none of them consecutive"
    );
    assert_eq!(summary.rescored, jobs.len() / 2);
}

#[tokio::test]
async fn semantic_rerank_stops_on_cancellation_without_spending() {
    let mut jobs = vec![ranked("https://example.com/a", 80.0)];
    let env = FakeRerankEnv::new(vec![(autopilot_job_id(&jobs[0]), 99.0)]);
    let blobs = blobs_for(&jobs);
    let cancel = CancellationToken::new();
    cancel.cancel();

    let summary = rerank_all(&env, &mut jobs, NO_CLUSTERS, &blobs, &cancel).await;

    assert_eq!(env.calls(), 0);
    assert_eq!(
        env.charges(),
        0,
        "a cancelled run must not charge the budget"
    );
    assert_eq!(summary.rescored, 0);
    assert_eq!(jobs[0].score, Some(80.0));
    assert_eq!(jobs[0].score_source, ScoreSource::Keyword);
}

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
    let env = std::sync::Arc::new(FakeRerankEnv::new(
        jobs.iter()
            .map(|j| (autopilot_job_id(j), 99.0))
            .collect::<Vec<_>>(),
    ));
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
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

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

/// The blob map is built for the re-rank CANDIDATES, not for the whole harvest
/// — and the set handed to `setup` must be a superset of what the loop visits,
/// or a candidate silently loses its blob and is skipped.
#[test]
fn the_candidate_url_set_covers_the_scorable_head_and_nothing_else() {
    let jobs = vec![
        ranked("https://example.com/a", 80.0),
        FoundJob {
            url: "https://example.com/unscored".into(),
            score: None,
            ..found(None)
        },
        ranked("https://example.com/b", 60.0),
    ];

    let candidates = rerank_candidate_urls(&jobs, NO_CLUSTERS);

    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains("https://example.com/a"));
    assert!(candidates.contains("https://example.com/b"));
    assert!(
        !candidates.contains("https://example.com/unscored"),
        "an unscored job is never re-ranked, so its blob is never needed"
    );
}

/// …and it must NOT be capped by POSITION. The loop's own `considered` counter
/// is the cost bound, and it only counts jobs that got past the blob and
/// URL-variant filters — both of which it applies AFTER the candidate set was
/// built. A positional cap therefore under-covers by exactly the number of rows
/// the loop skips, and a skipped row is silently dropped (no blob → `continue`),
/// so the phase quietly re-ranks fewer jobs than the ceiling allows.
#[test]
fn the_candidate_url_set_is_not_capped_by_position() {
    let jobs: Vec<FoundJob> = (0..SEMANTIC_RERANK_MAX + 7)
        .map(|i| ranked(&format!("https://example.com/{i}"), 90.0))
        .collect();

    assert_eq!(
        rerank_candidate_urls(&jobs, NO_CLUSTERS).len(),
        jobs.len(),
        "every scored canonical is reachable by the loop, so every one of them \
         needs its blob — the ceiling is enforced by the loop, not by this set"
    );
}

/// The defect a positional cap causes, end to end through the REAL phase with
/// the REAL candidate set feeding the blob map exactly as `autopilot_run` does:
/// two URL variants of one posting are collapsed by `seen_this_run` WITHOUT
/// counting toward `considered`, so the 20th distinct job sits past position 20
/// — where a capped set no longer has its blob. The run then pays for 19 of the
/// 20 slots the user is entitled to, silently.
#[tokio::test]
async fn a_collapsed_url_variant_does_not_cost_a_later_job_its_rerank_slot() {
    // Two variants of ONE posting (they share a cache id), then exactly
    // SEMANTIC_RERANK_MAX distinct postings behind them.
    let mut jobs = vec![
        ranked("https://example.com/job", 99.0),
        ranked("https://example.com/job?utm_source=alerts", 99.0),
    ];
    jobs.extend(
        (0..SEMANTIC_RERANK_MAX).map(|i| ranked(&format!("https://example.com/{i}"), 90.0)),
    );
    assert_eq!(
        autopilot_job_id(&jobs[0]),
        autopilot_job_id(&jobs[1]),
        "test premise: the first two rows must collapse to one cache identity"
    );

    let env = std::sync::Arc::new(FakeRerankEnv::new(
        jobs.iter()
            .map(|j| (autopilot_job_id(j), 99.0))
            .collect::<Vec<_>>(),
    ));
    let all_blobs = blobs_for(&jobs);

    let summary = semantic_rerank_phase(
        true,
        "rust engineer, kubernetes",
        &mut jobs,
        NO_CLUSTERS,
        &CancellationToken::new(),
        |candidates| {
            // Production shape: the map is keyed on the candidate set, so a
            // candidate missing from it loses its blob and is skipped.
            let blobs = all_blobs
                .iter()
                .filter(|(url, _)| candidates.contains(url.as_str()))
                .map(|(url, blob)| (url.clone(), blob.clone()))
                .collect();
            Some((std::sync::Arc::clone(&env), blobs))
        },
    )
    .await;

    assert_eq!(
        summary.map(|s| s.rescored),
        Some(SEMANTIC_RERANK_MAX),
        "the full top-N budget must still be spent on real jobs — a collapsed \
         duplicate costs a list position, never a re-rank slot"
    );
}

/// A cross-board duplicate's hidden member is not a candidate: the loop only
/// re-ranks the cluster canonical, so paying to keep its blob in memory buys
/// nothing.
#[test]
fn a_hidden_cluster_member_is_not_a_candidate() {
    use crate::scraping::cluster::ClusterAssignment;

    let jobs = vec![
        ranked("https://a.example.com/job", 80.0),
        ranked("https://b.example.com/job", 95.0),
    ];
    let clusters = vec![
        ClusterAssignment {
            cluster_id: "c1".into(),
            canonical: true,
            members: Vec::new(),
            is_agency: false,
        },
        ClusterAssignment {
            cluster_id: "c1".into(),
            canonical: false,
            members: Vec::new(),
            is_agency: false,
        },
    ];

    let candidates = rerank_candidate_urls(&jobs, &clusters);

    assert_eq!(candidates.len(), 1);
    assert!(candidates.contains("https://a.example.com/job"));
}

// ── the résumé snapshot vector dies with its autopilot ────────────────────

/// `autopilot-resume:<sha>` is résumé-derived user content in a cache bounded
/// only by a TTL and a row cap. Deleting the autopilot must delete it too,
/// rather than leaving it readable for up to the TTL (7 days by default).
#[test]
fn deleting_an_autopilot_drops_its_resume_snapshot_vector() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let docs = crate::documents::DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let resume = "rust engineer, kubernetes, distributed systems";
    let id = crate::commands::match_resume::autopilot_resume_id(resume);
    docs.upsert_posting_vector(
        &id,
        &crate::documents::sha256_hex(resume),
        &snapshot_vector(&docs),
    )
    .unwrap();
    assert!(docs.get_posting_vector(&id).is_some(), "seeded");

    drop_orphaned_resume_cache(&docs, Some(resume), &[]);

    assert!(
        docs.get_posting_vector(&id).is_none(),
        "the résumé-derived row must not outlive the record it was derived from"
    );
}

/// …but the id is the CONTENT, so a second autopilot with the same résumé is
/// still a live producer of that row: deleting it would just re-embed.
#[test]
fn a_resume_shared_with_another_autopilot_keeps_its_vector() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let docs = crate::documents::DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let resume = "rust engineer, kubernetes, distributed systems";
    let id = crate::commands::match_resume::autopilot_resume_id(resume);
    docs.upsert_posting_vector(
        &id,
        &crate::documents::sha256_hex(resume),
        &snapshot_vector(&docs),
    )
    .unwrap();

    let survivor = autopilot_with_resume(Some(resume));
    drop_orphaned_resume_cache(&docs, Some(resume), std::slice::from_ref(&survivor));
    assert!(docs.get_posting_vector(&id).is_some());

    // A remaining autopilot with a DIFFERENT résumé is not a producer of it.
    let other = autopilot_with_resume(Some("python data engineer"));
    drop_orphaned_resume_cache(&docs, Some(resume), std::slice::from_ref(&other));
    assert!(docs.get_posting_vector(&id).is_none());
}

/// The vector is only half the résumé's cache footprint: every `match_scores`
/// row keyed on `autopilot-resume:<sha>` holds résumé-DERIVED content too — the
/// gaps, the recommendations and the explanation all describe that résumé — and
/// nothing else can ever reach those rows once the record is gone.
#[test]
fn dropping_a_resume_snapshot_also_drops_the_scores_it_produced() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let docs = crate::documents::DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let resume = "rust engineer, kubernetes, distributed systems";
    let id = crate::commands::match_resume::autopilot_resume_id(resume);
    docs.upsert_posting_vector(
        &id,
        &crate::documents::sha256_hex(resume),
        &snapshot_vector(&docs),
    )
    .unwrap();
    // Two scored jobs for this résumé, plus one for an unrelated one.
    let job_hash = crate::documents::sha256_hex("We need a Rust engineer");
    let other_resume = crate::commands::match_resume::autopilot_resume_id("python data engineer");
    for resume_id in [&id, &id, &other_resume] {
        docs.upsert_match_score(
            &snapshot_score_key(resume_id, "autopilot:job-1", &job_hash),
            "{\"combined\":91,\"gaps\":[\"kubernetes\"]}",
        )
        .unwrap();
    }
    assert!(
        docs.get_match_score(&snapshot_score_key(&id, "autopilot:job-1", &job_hash))
            .is_some(),
        "seeded"
    );

    drop_orphaned_resume_cache(&docs, Some(resume), &[]);

    assert!(
        docs.get_match_score(&snapshot_score_key(&id, "autopilot:job-1", &job_hash))
            .is_none(),
        "the résumé's cached scores must die with it, not linger for the TTL"
    );
    assert!(
        docs.get_match_score(&snapshot_score_key(
            &other_resume,
            "autopilot:job-1",
            &job_hash
        ))
        .is_some(),
        "…and the delete must be scoped to THAT résumé — another autopilot's \
         scores are untouched"
    );
}

/// The UPDATE shape, which shipped with exactly the defect the DELETE path had
/// just closed: the cache id is `sha256(resume_text)`, so replacing the text
/// orphans the old rows just as thoroughly as deleting the record.
///
/// No before/after diff is needed to see it — the post-mutation snapshot still
/// contains this record carrying its NEW text, so the old text has no producer
/// left; an update that did NOT touch the résumé leaves the record as its own
/// producer and keeps the rows.
#[test]
fn editing_an_autopilots_resume_orphans_the_previous_snapshot() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let docs = crate::documents::DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let before = "rust engineer, kubernetes, distributed systems";
    let id = crate::commands::match_resume::autopilot_resume_id(before);
    let job_hash = crate::documents::sha256_hex("We need a Rust engineer");
    let seed = || {
        docs.upsert_posting_vector(
            &id,
            &crate::documents::sha256_hex(before),
            &snapshot_vector(&docs),
        )
        .unwrap();
        docs.upsert_match_score(
            &snapshot_score_key(&id, "autopilot:job-1", &job_hash),
            "{\"combined\":91}",
        )
        .unwrap();
    };

    // An update that did NOT change the résumé: the record still carries it.
    seed();
    let unchanged = autopilot_with_resume(Some(before));
    drop_orphaned_resume_cache(&docs, Some(before), std::slice::from_ref(&unchanged));
    assert!(
        docs.get_posting_vector(&id).is_some(),
        "an update that leaves resume_text alone must not evict its own cache"
    );

    // An update that REPLACED the résumé: nothing produces the old rows now.
    let edited = autopilot_with_resume(Some("staff platform engineer, terraform"));
    drop_orphaned_resume_cache(&docs, Some(before), std::slice::from_ref(&edited));
    assert!(
        docs.get_posting_vector(&id).is_none(),
        "the replaced résumé's snapshot vector is unreachable — it must not survive the TTL"
    );
    assert!(
        docs.get_match_score(&snapshot_score_key(&id, "autopilot:job-1", &job_hash))
            .is_none(),
        "…and neither may the scores it produced"
    );
}

/// The two tests above pin what `drop_orphaned_resume_cache` DOES; this pins
/// that the commands still call it — the wiring, which is the half that shipped
/// broken (the UPDATE path landed without the cleanup the DELETE path had just
/// gained). Driving `autopilot_update`/`autopilot_remove` for real needs an
/// `AppHandle` and this crate has no `tauri::test` mock-app harness, so the
/// cheapest honest guard is a source pin (the `pipeline::json` +
/// `extension_bridge::answer_rewrite` precedent; `include_str!` also makes rustc
/// track the file, so this can never read a stale copy).
///
/// The invariant: every mutation of an autopilot RECORD goes through
/// `mutate_record`, which owns both halves. A new command that reaches the store
/// directly — the exact shape of the original defect — fails here.
///
/// Rustfmt assumption: the enclosing opener of a call is the nearest preceding
/// line at a strictly smaller indent. True for every form rustfmt emits here; a
/// hand-wrapped receiver chain would need this pin updated with it.
#[test]
fn every_record_mutation_goes_through_mutate_record() {
    const SRC: &str = include_str!("../autopilot.rs");

    // Assembled at runtime, never written out as one literal: an inline needle
    // would appear in THIS file, and this file is a `#[path]` module OF the one
    // being scanned — a future inlining would make the test satisfy its own scan.
    let writers: Vec<String> = ["update", "remove"]
        .iter()
        .map(|method| format!(".{}().{method}(", "lock"))
        .collect();
    let owner = format!("mutate_{}(", "record");
    let indent = |l: &str| l.len() - l.trim_start().len();
    let is_code = |l: &str| !l.trim().is_empty() && !l.trim_start().starts_with("//");

    let lines: Vec<&str> = SRC.lines().collect();
    let mut pinned = Vec::new();
    for (i, &line) in lines.iter().enumerate() {
        if !is_code(line) || !writers.iter().any(|w| line.contains(w.as_str())) {
            continue;
        }
        // The in-flight guard is a `HashSet` of run ids, not the record store.
        if line.contains("RUNS_IN_FLIGHT") {
            continue;
        }
        let opener = lines[..i]
            .iter()
            .rev()
            .copied()
            .filter(|l| is_code(l))
            .find(|l| indent(l) < indent(line))
            .unwrap_or_default();
        assert!(
            opener.contains(&owner),
            "src/commands/autopilot.rs:{}\n  {}\nmutates an autopilot record outside \
             `mutate_record`, whose enclosing block is instead:\n  {}\n\n\
             Every record mutation must run through `mutate_record`: the cache id is \
             sha256(resume_text), so a DELETE and a resume REPLACE orphan the identical \
             `autopilot-resume:<sha>` vector + `match_scores` rows. Route the new call \
             through it rather than remembering the cleanup at one more site.",
            i + 1,
            line.trim(),
            opener.trim()
        );
        pinned.push(line.trim());
    }
    assert_eq!(
        pinned.len(),
        2,
        "expected exactly the update + remove call sites to be pinned; got {pinned:?}. \
         A drop to 0 means the scan stopped matching the real writers (a renamed store \
         method, or a reformatted call) and is silently guarding nothing."
    );
}

/// The `match_scores` key an Autopilot re-rank writes for a résumé snapshot.
fn snapshot_score_key<'a>(
    resume_id: &'a str,
    job_id: &'a str,
    job_text_hash: &'a str,
) -> crate::documents::MatchScoreKey<'a> {
    crate::documents::MatchScoreKey {
        resume_id,
        job_id,
        provider: "ollama",
        model: "nomic-embed-text",
        semantic_enabled: 1,
        formula_version: 2,
        vector_version: 1,
        job_text_hash,
    }
}

/// A vector in the active embedding space, so `get_posting_vector` reads it back.
fn snapshot_vector(
    docs: &crate::documents::DocumentStore,
) -> crate::commands::ai_provider::EmbeddingVector {
    let active = docs.embedding_config();
    crate::commands::ai_provider::EmbeddingVector {
        values: vec![0.1, 0.2, 0.3],
        space: crate::commands::ai_provider::EmbeddingSpace {
            provider: active.provider,
            model: active.model,
            dim: 3,
            version: crate::commands::ai_provider::EMBEDDING_VECTOR_VERSION,
        },
    }
}

fn autopilot_with_resume(resume_text: Option<&str>) -> Autopilot {
    let mut ap: Autopilot = serde_json::from_value(serde_json::json!({
        "_id": "ap-1",
        "name": "n",
        "status": "active",
        "target": { "boards": [], "query": "rust", "pages": 1 },
        "filter": { "minMatchScore": 0.0 },
        "schedule": "manual",
        "totalFound": 0,
        "totalApplied": 0,
        "createdAt": 0,
        "updatedAt": 0,
    }))
    .expect("autopilot fixture");
    ap.resume_text = resume_text.map(String::from);
    ap
}

// ── mixed-scale ordering ──────────────────────────────────────────────────

/// After phase 2 the list carries two scales. They must not share one sort
/// axis: `generate_assistant_notes` takes its ≤3 AI-note recipients straight
/// off this order, so a never-re-ranked keyword 62 outranking a re-ranked
/// combined 58 spends a provider completion on a job the re-rank demoted.
#[test]
fn re_ranked_jobs_form_the_head_and_the_keyword_tail_follows() {
    let combined = |url: &str, score: f64| FoundJob {
        score_source: ScoreSource::Combined,
        ..ranked(url, score)
    };
    let mut jobs = [
        ranked("https://example.com/keyword-62", 62.0),
        combined("https://example.com/combined-58", 58.0),
        ranked("https://example.com/keyword-40", 40.0),
        combined("https://example.com/combined-91", 91.0),
        FoundJob {
            url: "https://example.com/unscored".into(),
            ..found(None)
        },
    ];
    jobs.sort_by(by_rank);

    assert_eq!(
        jobs.iter().map(|j| j.url.as_str()).collect::<Vec<_>>(),
        vec![
            "https://example.com/combined-91",
            "https://example.com/combined-58",
            "https://example.com/keyword-62",
            "https://example.com/keyword-40",
            "https://example.com/unscored",
        ],
        "re-ranked jobs (combined scale) first, ordered among themselves; then the \
         keyword tail by coverage; unscored last"
    );
}

// ── one embed per CLUSTER, spent on the displayed canonical ───────────────

/// A cross-board duplicate pair has two different `canonical_job_key`s, so it
/// used to take two top-N slots and two embeds — and the paid-for score could
/// land on the copy the UI hides. Keyed on the clustering verdict, the pair
/// costs ONE embed and the DISPLAYED canonical is the one that carries it.
#[tokio::test]
async fn a_cross_board_duplicate_pair_costs_one_embed_on_the_displayed_canonical() {
    // Same job on two boards. The aggregator copy has the better keyword score
    // (so it comes FIRST in the list) but no description, which is exactly what
    // makes the other copy the cluster canonical — the row the UI displays.
    let aggregator_copy = FoundJob {
        url: "https://agg.example.com/job".into(),
        board: Some(AGGREGATOR_SNIPPET_SOURCE.to_string()),
        score: Some(90.0),
        ..found(None)
    };
    let full_text_copy = FoundJob {
        url: "https://boards.example.com/job".into(),
        board: Some("greenhouse".into()),
        description: Some("We need a Rust engineer".into()),
        score: Some(50.0),
        ..found(None)
    };
    let other_job = FoundJob {
        url: "https://example.com/other".into(),
        title: "Data Scientist".into(),
        company: "Zeta".into(),
        score: Some(70.0),
        ..found(None)
    };

    // The REAL pairing the command uses: retention returns each surviving row's
    // clustering verdict alongside it.
    let (mut jobs, clusters) = cluster_aware_retain(
        vec![aggregator_copy, full_text_copy, other_job],
        0.0,
        &HashSet::new(),
        &[],
    );
    assert_eq!(jobs.len(), 3);
    assert_eq!(
        clusters[0].cluster_id, clusters[1].cluster_id,
        "test premise: the two board copies must be ONE cluster"
    );
    assert!(
        !clusters[0].canonical && clusters[1].canonical,
        "test premise: the hidden (aggregator, description-less) copy is listed first"
    );

    // Every job is scriptable, so the assertions below are about WHICH ones the
    // pass chooses to score, not about which ones it could.
    let env = FakeRerankEnv::new(
        jobs.iter()
            .map(|j| (autopilot_job_id(j), 99.0))
            .collect::<Vec<_>>(),
    );
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        &clusters,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(
        env.calls(),
        2,
        "one embed per CLUSTER: the duplicate pair must not buy two"
    );
    assert_eq!(summary.considered, 2);
    assert_eq!(
        jobs[1].score_source,
        ScoreSource::Combined,
        "the DISPLAYED canonical is the member that carries the combined score"
    );
    assert_eq!(jobs[1].score, Some(99.0));
    assert_eq!(
        jobs[0].score,
        Some(90.0),
        "the hidden member keeps its keyword score — the embed was not spent on it"
    );
    assert_eq!(jobs[0].score_source, ScoreSource::Keyword);
}

/// The degrade boundary, against REALISTIC `score_one` output shapes. A
/// keyword-only or failed result must never be promoted to a semantic
/// re-rank just because it carries a `combined` number.
#[test]
fn only_a_kernel_reported_combined_source_counts_as_a_semantic_rescore() {
    // A real semantic result: an embedding pair backed `combined`.
    let ok = json!({
        "resumeId": "autopilot:abc", "jobId": "autopilot:def",
        "ats": 40.0, "semantic": 90.0, "combined": 70.0,
        "gaps": [], "recommendations": [], "explanation": "…", "guidance": "…",
        "scoreSource": "combined",
    });
    assert_eq!(rerank_score_from(&ok), Some(70.0));

    // `score_one`'s own degrade: no vector → `combined == ats`, and it says
    // so. Promoting this would relabel a keyword number as semantic.
    let degraded = json!({
        "ats": 40.0, "semantic": 0.0, "combined": 40.0, "scoreSource": "keyword",
    });
    assert_eq!(rerank_score_from(&degraded), None);

    // A `semantic: 0.0` reading is NOT the degrade signal — a real cosine
    // can legitimately clamp to zero, and that score is still semantic.
    let genuine_zero = json!({
        "ats": 40.0, "semantic": 0.0, "combined": 16.0, "scoreSource": "combined",
    });
    assert_eq!(rerank_score_from(&genuine_zero), Some(16.0));

    // Error object (job text missing) → degrade, never a score.
    assert_eq!(
        rerank_score_from(&json!({ "error": "job not found in cache: x" })),
        None
    );
    // A cache row written before `scoreSource` existed → degrade, not a
    // silently-unlabelled promotion.
    assert_eq!(
        rerank_score_from(&json!({ "ats": 40.0, "combined": 70.0 })),
        None
    );
}

/// Cache reuse (ADR-017): a repeat run must be near-free. The mechanism is
/// the cache KEY — this asserts against the real `match_scores` store, with
/// ids derived by the real `autopilot_job_id`/`autopilot_resume_id`.
#[test]
fn a_repeat_run_hits_the_cached_score_even_under_a_tracking_param_url() {
    use crate::documents::{sha256_hex, DocumentStore, MatchScoreKey};

    let temp_dir = tempfile::TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let resume = "rust engineer, kubernetes, postgres";
    let job_text = "We need a Rust engineer";
    let text_hash = sha256_hex(job_text);
    fn key<'a>(resume_id: &'a str, job_id: &'a str, job_text_hash: &'a str) -> MatchScoreKey<'a> {
        MatchScoreKey {
            resume_id,
            job_id,
            provider: "ollama",
            model: "nomic-embed-text",
            semantic_enabled: 1,
            formula_version: 2,
            vector_version: 1,
            job_text_hash,
        }
    }

    // Run 1 caches a combined score for the job as first seen.
    let run1 = ranked("https://example.com/job", 40.0);
    let resume_id = crate::commands::match_resume::autopilot_resume_id(resume);
    store
        .upsert_match_score(
            &key(&resume_id, &autopilot_job_id(&run1), &text_hash),
            "{\"combined\":91}",
        )
        .unwrap();

    // Run 2 re-surfaces the SAME posting under tracking params. Keying on
    // `canonical_job_key` (not the raw url) is what makes this a HIT — the
    // second run pays nothing.
    let run2 = ranked("https://example.com/job?utm_source=newsletter", 40.0);
    assert!(
        store
            .get_match_score(&key(&resume_id, &autopilot_job_id(&run2), &text_hash))
            .is_some(),
        "a re-surfaced job must reuse the cached score instead of re-embedding"
    );

    // Self-invalidation: editing the autopilot's résumé is a different
    // content-addressed id, so the stale score can never be served.
    let edited = crate::commands::match_resume::autopilot_resume_id("totally different resume");
    assert_ne!(edited, resume_id);
    assert!(
        store
            .get_match_score(&key(&edited, &autopilot_job_id(&run2), &text_hash))
            .is_none(),
        "an edited résumé must MISS, never reuse the previous résumé's score"
    );
}

#[tokio::test]
async fn semantic_rerank_pays_once_for_a_job_surfaced_under_two_url_variants() {
    // Both rows are the SAME posting to `canonical_job_key` (tracking params are
    // normalized away) — the cluster/merge pass that collapses them runs later,
    // in `record_run`, so phase 2 sees both. Paying twice would burn a top-N
    // slot (and a daily charge) on a score the first call already produced.
    let mut jobs = vec![
        ranked("https://example.com/job", 80.0),
        ranked("https://example.com/job?utm_source=alerts", 80.0),
        ranked("https://example.com/other", 70.0),
    ];
    assert_eq!(
        autopilot_job_id(&jobs[0]),
        autopilot_job_id(&jobs[1]),
        "test premise: the two URL variants must share one cache identity"
    );
    let env = FakeRerankEnv::new(vec![
        (autopilot_job_id(&jobs[0]), 95.0),
        (autopilot_job_id(&jobs[2]), 60.0),
    ]);
    let blobs = blobs_for(&jobs);

    let summary = rerank_all(
        &env,
        &mut jobs,
        NO_CLUSTERS,
        &blobs,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(
        env.calls(),
        2,
        "the duplicate variant must not be scored again"
    );
    assert_eq!(
        env.charges(),
        2,
        "…and the charge count is the SCORED count, not the row count: three rows, \
         one collapsed as a URL variant, two charges. The skip precedes the charge, \
         so the duplicate never reaches the shared per-provider ceiling"
    );
    assert_eq!(summary.considered, 2);
    // The first variant IS re-ranked; the duplicate keeps its keyword score
    // (the merge in `record_run` collapses the two rows anyway).
    assert_eq!(jobs[0].score, Some(95.0));
    assert_eq!(jobs[0].score_source, ScoreSource::Combined);
    assert_eq!(jobs[1].score, Some(80.0));
    assert_eq!(jobs[1].score_source, ScoreSource::Keyword);
}
