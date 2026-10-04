//! The phase-2 re-rank loop (`semantic_rerank`): ordering, degrade, ceiling, breaker, cancellation.

use tokio_util::sync::CancellationToken;

use super::super::rerank::*;
use super::support::*;
use crate::autopilot::{FoundJob, ScoreSource};

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

    let summary = rerank_jobs(&env, &mut jobs).await;
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

    let summary = rerank_jobs(&env, &mut jobs).await;

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

    rerank_jobs(&env, &mut jobs).await;

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

    let summary = rerank_jobs(&env, &mut jobs).await;

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
    let env = FakeRerankEnv::scoring_all(&jobs, 99.0);

    let summary = rerank_jobs(&env, &mut jobs).await;

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
    let mut env = FakeRerankEnv::scoring_all(&jobs, 99.0);
    env.charge_fails_after = Some(2); // the 3rd charge is refused

    let summary = rerank_jobs(&env, &mut jobs).await;

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

    let summary = rerank_jobs(&env, &mut jobs).await;

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

    let summary = rerank_jobs(&env, &mut jobs).await;

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
