//! What a re-rank costs: one embed per cluster, one per posting, cache hits, and the mixed-scale
//! ordering that follows it.

use std::collections::HashSet;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::super::keyword_rank::{cluster_aware_retain, AGGREGATOR_SNIPPET_SOURCE};
use super::super::rerank::*;
use super::support::*;
use crate::autopilot::{FoundJob, ScoreSource};

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
    let env = FakeRerankEnv::scoring_all(&jobs, 99.0);
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
    use crate::documents::{sha256_hex, DocumentStore};

    let temp_dir = tempfile::TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let resume = "rust engineer, kubernetes, postgres";
    let job_text = "We need a Rust engineer";
    let text_hash = sha256_hex(job_text);

    // Run 1 caches a combined score for the job as first seen.
    let run1 = ranked("https://example.com/job", 40.0);
    let resume_id = crate::commands::match_resume::autopilot_resume_id(resume);
    store
        .upsert_match_score(
            &snapshot_score_key(&resume_id, &autopilot_job_id(&run1), &text_hash),
            "{\"combined\":91}",
        )
        .unwrap();

    // Run 2 re-surfaces the SAME posting under tracking params. Keying on
    // `canonical_job_key` (not the raw url) is what makes this a HIT — the
    // second run pays nothing.
    let run2 = ranked("https://example.com/job?utm_source=newsletter", 40.0);
    assert!(
        store
            .get_match_score(&snapshot_score_key(
                &resume_id,
                &autopilot_job_id(&run2),
                &text_hash
            ))
            .is_some(),
        "a re-surfaced job must reuse the cached score instead of re-embedding"
    );

    // Self-invalidation: editing the autopilot's résumé is a different
    // content-addressed id, so the stale score can never be served.
    let edited = crate::commands::match_resume::autopilot_resume_id("totally different resume");
    assert_ne!(edited, resume_id);
    assert!(
        store
            .get_match_score(&snapshot_score_key(
                &edited,
                &autopilot_job_id(&run2),
                &text_hash
            ))
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

    let summary = rerank_jobs(&env, &mut jobs).await;

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
