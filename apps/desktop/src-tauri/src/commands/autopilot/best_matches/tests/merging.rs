//! One row per cluster across autopilots: sources, cluster members, tombstone splits and the earliest
//! `foundAt`.

use super::*;

#[test]
fn same_listing_across_two_autopilots_merges_into_one_row_with_two_sources() {
    let a = single(
        "a",
        "https://a.example.com/job",
        "Rust Developer",
        "Acme",
        80.0,
    );
    let b = single(
        "b",
        "https://b.example.com/job",
        "Rust Developer",
        "Acme",
        85.0,
    );
    let out = best(&[a, b]);
    assert_eq!(
        out.matches.len(),
        1,
        "same title+company from two autopilots is one cluster"
    );
    assert_eq!(out.matches[0].sources.len(), 2);
    assert_eq!(out.total, 1);
    assert_eq!(out.autopilot_count, 2);
}

#[test]
fn identical_url_from_two_autopilots_dedupes_cluster_members_to_one() {
    // The EXACT same posting url, found independently by two autopilots.
    // Before H3's pre-clustering dedupe this produced TWO identical
    // `ClusterMemberRef`s (one per input item) even though there is only
    // one real board copy — any `clusterMembers.length > 1` gate would
    // misread that as "found on 2 boards".
    let a = single(
        "a",
        "https://jobs.lever.co/acme/123",
        "Rust Developer",
        "Acme",
        80.0,
    );
    let b = single(
        "b",
        "https://jobs.lever.co/acme/123",
        "Rust Developer",
        "Acme",
        85.0,
    );
    let out = best(&[a, b]);
    assert_eq!(out.matches.len(), 1);
    assert_eq!(
        out.matches[0].cluster_members.len(),
        1,
        "the identical url is ONE cluster member, not one per contributing autopilot"
    );
    assert_eq!(
        out.matches[0].sources.len(),
        2,
        "both autopilots are still credited as sources"
    );
    assert_eq!(
        out.matches[0].score, 85.0,
        "the better-scored duplicate copy wins"
    );
}

#[test]
fn tombstone_veto_splits_a_cross_autopilot_near_duplicate_into_two_rows() {
    let key_a = crate::scraping::boards::common::canonical_job_key(
        "https://a.example.com/job1",
        "Senior Rust Engineer",
        "Acme",
    );
    let key_b = crate::scraping::boards::common::canonical_job_key(
        "https://b.example.com/job2",
        "Senior Rust Engineer",
        "Acme",
    );
    let a = single(
        "a",
        "https://a.example.com/job1",
        "Senior Rust Engineer",
        "Acme",
        90.0,
    );
    let b = single(
        "b",
        "https://b.example.com/job2",
        "Senior Rust Engineer",
        "Acme",
        85.0,
    );
    let tombstones: HashSet<(String, String)> =
        [tombstone_pair(&key_a, &key_b)].into_iter().collect();
    let out = compute_best_matches(&[a, b], &tombstones, &[], &no_dismissed());
    assert_eq!(
        out.matches.len(),
        2,
        "a tombstoned pair never joins, even across autopilots"
    );
    assert_eq!(out.total, 2);
    for row in &out.matches {
        assert_eq!(row.sources.len(), 1);
    }
}

#[test]
fn found_at_is_the_earliest_across_cluster_members() {
    let a = autopilot(
        "a",
        AutopilotStatus::Active,
        vec![job_found_at(
            job(
                "https://a.example.com/job",
                "Senior Rust Engineer",
                "Acme",
                Some(90.0),
                ScoreSource::Keyword,
            ),
            500,
        )],
    );
    let b = autopilot(
        "b",
        AutopilotStatus::Active,
        vec![job_found_at(
            job(
                "https://b.example.com/job",
                "Senior Rust Engineer",
                "Acme",
                Some(80.0),
                ScoreSource::Keyword,
            ),
            100,
        )],
    );
    let out = best(&[a, b]);
    assert_eq!(out.matches.len(), 1);
    assert_eq!(
        out.matches[0].found_at, 100,
        "row found_at is the EARLIEST across all sources, not the best-scored member's own"
    );
}
