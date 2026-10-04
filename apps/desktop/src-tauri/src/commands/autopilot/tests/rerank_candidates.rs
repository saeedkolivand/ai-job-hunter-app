//! Which jobs the re-rank may visit, and the blobs built for them.

use tokio_util::sync::CancellationToken;

use super::super::rerank::*;
use super::support::*;
use crate::autopilot::FoundJob;

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

    let env = std::sync::Arc::new(FakeRerankEnv::scoring_all(&jobs, 99.0));
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
