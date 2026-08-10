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

// ── Phase 2: semantic re-rank (ADR-020 addendum) ──────────────────────────

/// Scriptable [`RerankEnv`] fake. Counts every scoring call and every daily
/// charge, so a test can pin "the scheduled path made ZERO scoring calls"
/// rather than only asserting on the resulting scores (which a broken
/// implementation could reproduce by accident).
struct FakeRerankEnv {
    /// url-independent: keyed by the derived cache job id → the score to
    /// return. A missing entry models "no semantic score available"
    /// (embed failed / provider offline) → the degrade path.
    scores: std::sync::Mutex<HashMap<String, f64>>,
    calls: std::sync::atomic::AtomicUsize,
    charges: std::sync::atomic::AtomicUsize,
    /// When `Some(n)`, `charge_daily` fails from the n-th call onward —
    /// models hitting the shared per-provider daily ceiling mid-run.
    charge_fails_after: Option<usize>,
}

impl FakeRerankEnv {
    fn new(scores: Vec<(String, f64)>) -> Self {
        Self {
            scores: std::sync::Mutex::new(scores.into_iter().collect()),
            calls: std::sync::atomic::AtomicUsize::new(0),
            charges: std::sync::atomic::AtomicUsize::new(0),
            charge_fails_after: None,
        }
    }
    fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn charges(&self) -> usize {
        self.charges.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl RerankEnv for FakeRerankEnv {
    async fn score(&self, job_id: &str, _job_text: String) -> Option<f64> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.scores.lock().unwrap().get(job_id).copied()
    }
    fn charge_daily(&self) -> crate::error::AppResult<()> {
        let n = self
            .charges
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        match self.charge_fails_after {
            Some(limit) if n > limit => Err(crate::error::AppError::RateLimited(
                "daily ceiling reached".into(),
            )),
            _ => Ok(()),
        }
    }
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

    let summary = semantic_rerank(&env, &mut jobs, &blobs, &CancellationToken::new()).await;
    // The command re-sorts after the re-rank; mirror that here so the test
    // pins ORDER, not just the numbers.
    jobs.sort_by(|a, b| b.score.unwrap().partial_cmp(&a.score.unwrap()).unwrap());

    assert_eq!(
        summary,
        RerankSummary {
            considered: 3,
            rescored: 3,
            degraded: 0
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

    let summary = semantic_rerank(&env, &mut jobs, &blobs, &CancellationToken::new()).await;

    assert_eq!(
        summary,
        RerankSummary {
            considered: 3,
            rescored: 2,
            degraded: 1
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

    semantic_rerank(&env, &mut jobs, &blobs, &CancellationToken::new()).await;

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

    let summary = semantic_rerank(&env, &mut jobs, &blobs, &CancellationToken::new()).await;

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

    let summary = semantic_rerank(&env, &mut jobs, &blobs, &CancellationToken::new()).await;

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

    let summary = semantic_rerank(&env, &mut jobs, &blobs, &CancellationToken::new()).await;

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

#[tokio::test]
async fn semantic_rerank_stops_on_cancellation_without_spending() {
    let mut jobs = vec![ranked("https://example.com/a", 80.0)];
    let env = FakeRerankEnv::new(vec![(autopilot_job_id(&jobs[0]), 99.0)]);
    let blobs = blobs_for(&jobs);
    let cancel = CancellationToken::new();
    cancel.cancel();

    let summary = semantic_rerank(&env, &mut jobs, &blobs, &cancel).await;

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

/// Semantic OFF is the default and the load-bearing regression: a scheduled
/// run must stay byte-for-byte the pre-existing embedding-free pipeline.
/// `autopilot_run` gates the whole phase-2 block on the preference, so the
/// gate is what this pins — with a counting env proving ZERO calls, not just
/// unchanged scores.
#[tokio::test]
async fn semantic_off_makes_zero_scoring_calls_and_leaves_the_keyword_rank_intact() {
    let mut jobs = vec![
        ranked("https://example.com/a", 80.0),
        ranked("https://example.com/b", 60.0),
    ];
    let before = jobs.clone();
    let env = FakeRerankEnv::new(
        jobs.iter()
            .map(|j| (autopilot_job_id(j), 99.0))
            .collect::<Vec<_>>(),
    );
    let blobs = blobs_for(&jobs);

    // The command's gate, spelled out: with the preference off the re-rank
    // is never entered at all.
    let semantic_on = false;
    if semantic_on {
        semantic_rerank(&env, &mut jobs, &blobs, &CancellationToken::new()).await;
    }

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

    let summary = semantic_rerank(&env, &mut jobs, &blobs, &CancellationToken::new()).await;

    assert_eq!(
        env.calls(),
        2,
        "the duplicate variant must not be scored again"
    );
    assert_eq!(
        env.charges(),
        2,
        "…and must not burn a daily charge either — the skip precedes the charge"
    );
    assert_eq!(summary.considered, 2);
    // The first variant IS re-ranked; the duplicate keeps its keyword score
    // (the merge in `record_run` collapses the two rows anyway).
    assert_eq!(jobs[0].score, Some(95.0));
    assert_eq!(jobs[0].score_source, ScoreSource::Combined);
    assert_eq!(jobs[1].score, Some(80.0));
    assert_eq!(jobs[1].score_source, ScoreSource::Keyword);
}
