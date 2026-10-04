//! Fixtures shared by the `commands::autopilot` test topics: postings and found jobs, and a scriptable
//! stand-in for the re-rank provider seam.

use std::collections::{HashMap, HashSet};

use tokio_util::sync::CancellationToken;

use super::super::rerank::*;
use crate::autopilot::tests::support::found_job_full;
use crate::autopilot::FoundJob;
use crate::scraping::JobPosting;

pub(super) fn posting(title: &str, description: Option<&str>) -> JobPosting {
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

pub(super) fn found(score: Option<f64>) -> FoundJob {
    FoundJob {
        score,
        ..found_job_full("https://example.com/job", "t", "c", 0)
    }
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
pub(super) struct FakeRerankEnv {
    /// url-independent: keyed by the derived cache job id → the score to
    /// return. A missing entry models "no semantic score available"
    /// (embed failed / provider offline) → the degrade path.
    pub(super) scores: std::sync::Mutex<HashMap<String, f64>>,
    /// Job ids whose score comes from cache: no provider round-trip, so no
    /// daily charge.
    pub(super) cached: HashSet<String>,
    pub(super) calls: std::sync::atomic::AtomicUsize,
    pub(super) charges: std::sync::atomic::AtomicUsize,
    /// When `Some(n)`, the charge fails from the n-th round-trip onward —
    /// models hitting the shared per-provider daily ceiling mid-run.
    pub(super) charge_fails_after: Option<usize>,
    /// Per-call latency, for the wall-clock-timeout test. Applied AFTER the
    /// budget check, like a real provider call.
    pub(super) delay: Option<std::time::Duration>,
}

impl FakeRerankEnv {
    pub(super) fn new(scores: Vec<(String, f64)>) -> Self {
        Self {
            scores: std::sync::Mutex::new(scores.into_iter().collect()),
            cached: HashSet::new(),
            calls: std::sync::atomic::AtomicUsize::new(0),
            charges: std::sync::atomic::AtomicUsize::new(0),
            charge_fails_after: None,
            delay: None,
        }
    }
    /// A fake that scores every job in `jobs` at `score`.
    pub(super) fn scoring_all(jobs: &[FoundJob], score: f64) -> Self {
        Self::new(
            jobs.iter()
                .map(|j| (autopilot_job_id(j), score))
                .collect::<Vec<_>>(),
        )
    }
    /// Mark these job ids as already cached — scored, but with no round-trip.
    pub(super) fn with_cached(mut self, ids: impl IntoIterator<Item = String>) -> Self {
        self.cached = ids.into_iter().collect();
        self
    }
    pub(super) fn with_delay(mut self, delay: std::time::Duration) -> Self {
        self.delay = Some(delay);
        self
    }
    /// Scoring calls that actually ran (i.e. got past the budget check).
    pub(super) fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
    /// Charges against the shared per-provider daily ceiling.
    pub(super) fn charges(&self) -> usize {
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
pub(super) const NO_CLUSTERS: &[crate::scraping::cluster::ClusterAssignment] = &[];

/// Run the re-rank loop and hand back its summary.
///
/// `semantic_rerank` accumulates into a caller-owned summary (so a pass the wall
/// clock cuts off still reports its partial counts); these loop tests care about
/// the completed pass, so the accumulator is a local detail here.
pub(super) async fn rerank_all(
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

/// `rerank_all` over `jobs` with no clustering verdicts, the blobs `autopilot_run` would build and a
/// token nobody cancels — the common shape of the loop tests.
pub(super) async fn rerank_jobs(env: &dyn RerankEnv, jobs: &mut [FoundJob]) -> RerankSummary {
    let blobs = blobs_for(jobs);
    rerank_all(env, jobs, NO_CLUSTERS, &blobs, &CancellationToken::new()).await
}

/// Build a scored job at `url` with a phase-1 keyword score.
pub(super) fn ranked(url: &str, score: f64) -> FoundJob {
    FoundJob {
        url: url.into(),
        score: Some(score),
        ..found(None)
    }
}

/// The blob map `autopilot_run` builds: phase 1's exact scoring text per url.
pub(super) fn blobs_for(jobs: &[FoundJob]) -> HashMap<String, String> {
    jobs.iter()
        .map(|j| (j.url.clone(), format!("jd text for {}", j.url)))
        .collect()
}

/// The `match_scores` key an Autopilot re-rank writes for a résumé snapshot.
pub(super) fn snapshot_score_key<'a>(
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
