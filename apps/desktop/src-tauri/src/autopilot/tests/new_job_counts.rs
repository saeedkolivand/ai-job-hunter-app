//! `record_run`'s "N new jobs" count: deduped, cluster-aware, and stable across split cycles.

use super::super::*;
use super::support::*;

/// The cluster id of the job whose url contains `needle`.
fn cluster_of(jobs: &[FoundJob], needle: &str) -> Option<String> {
    jobs.iter()
        .find(|j| j.url.contains(needle))
        .and_then(|j| j.cluster_id.clone())
}

#[test]
fn record_run_new_count_reflects_deduped_batch() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP", "aggregator", 0.0, "manual");
    let id = ap.id;

    // One logical job surfaced by two sources (URL variants that canonicalize to
    // the same key) + one genuinely distinct job at a DIFFERENT company. The two
    // variants merge; the two distinct jobs sit in different clusters (distinct
    // companies → different blocks), so the cluster count is 2.
    let dup_a = found_job_full(
        "https://jobs.example.com/eng-1?utm_source=x",
        "Engineer",
        "AcmeOne",
        1,
    );
    let dup_b = found_job_full(
        "https://jobs.example.com/eng-1#frag",
        "Engineer",
        "AcmeOne",
        2,
    );
    let other = found_job_full("https://jobs.example.com/eng-2", "Engineer", "AcmeTwo", 3);

    let new_count = record(&store, &id, 3, vec![dup_a, dup_b, other]);

    assert_eq!(
        new_count, 2,
        "the 'N new jobs' count must reflect the DEDUPED batch as CLUSTERS (2 distinct), not the raw 3"
    );
}

#[test]
fn record_run_reports_only_newly_surfaced_jobs() {
    let (_temp, store) = temp_store();
    let ap = create_ap(&store, "AP", "linkedin", 50.0, "manual");
    let id = ap.id;

    // Distinct companies keep each url in its own cluster, so the cluster count
    // tracks first-seen urls exactly (no accidental same-title merges).
    let j = |url: &str, company: &str, at: u64| found_job_full(url, "Engineer", company, at);

    // First run — both URLs are brand new → drives a "2 new jobs" notification.
    assert_eq!(
        record(
            &store,
            &id,
            2,
            vec![j("u1", "Alpha", 1), j("u2", "Beta", 2)]
        ),
        2
    );
    // Re-run with the two seen URLs + one unseen → only the unseen counts.
    assert_eq!(
        record(
            &store,
            &id,
            3,
            vec![j("u1", "Alpha", 9), j("u2", "Beta", 9), j("u3", "Gamma", 9)]
        ),
        1
    );
    // Nothing unseen → no notification.
    assert_eq!(record(&store, &id, 3, vec![j("u1", "Alpha", 9)]), 0);
    // Unknown autopilot → 0 (no panic).
    assert_eq!(record(&store, "missing", 5, vec![j("x", "Alpha", 1)]), 0);
}

// ── Cross-board cluster counts + split survival (ADR-029 §f/§h) ───────────────

/// One direct-board FoundJob with an explicit board id (source).
fn board_job(url: &str, board: &str, at: u64) -> FoundJob {
    FoundJob {
        board: Some(board.into()),
        ..found_job_full(url, "Rust Developer", "Acme", at)
    }
}

fn manual_ap(store: &AutopilotStore) -> String {
    create_ap(store, "AP", "linkedin", 0.0, "manual").id
}

#[test]
fn record_run_cluster_count_two_board_new_is_one() {
    let (_temp, store) = temp_store();
    let id = manual_ap(&store);

    // The SAME job on two boards (same title+company, distinct urls/sources) →
    // one cluster, all members new → the notification count is 1, not 2.
    let a = board_job("https://a.example.com/1", "greenhouse", 1);
    let b = board_job("https://b.example.com/2", "aggregator", 2);
    let new_count = record(&store, &id, 2, vec![a, b]);
    assert_eq!(
        new_count, 1,
        "one job on two boards counts as ONE new cluster"
    );
}

#[test]
fn record_run_cluster_count_known_job_resurfacing_is_zero() {
    let (_temp, store) = temp_store();
    let id = manual_ap(&store);

    // Run 1: the job is first seen on a direct board → 1 new.
    let direct = board_job("https://a.example.com/1", "greenhouse", 1);
    assert_eq!(record(&store, &id, 1, vec![direct]), 1);

    // Run 2: the SAME job resurfaces via the aggregator (new url) — it clusters
    // with the known direct row, so the cluster is not all-new → 0.
    let agg = board_job("https://b.example.com/2", "aggregator", 2);
    assert_eq!(
        record(&store, &id, 2, vec![agg]),
        0,
        "a known job resurfacing on another board contributes 0 new"
    );
}

#[test]
fn split_survives_two_record_run_cycles() {
    let (_temp, store) = temp_store();
    let id = manual_ap(&store);

    let a = board_job("https://a.example.com/1", "greenhouse", 1);
    let b = board_job("https://b.example.com/2", "lever", 2);

    // Without a tombstone the two cluster together.
    record(&store, &id, 2, vec![a.clone(), b.clone()]);
    let jobs = store.get(&id).unwrap().found_jobs;
    let cid_a = cluster_of(&jobs, "a.example");
    let cid_b = cluster_of(&jobs, "b.example");
    assert_eq!(cid_a, cid_b, "no tombstone → same cluster");

    // Tombstone their canonical keys, then re-run TWICE — the split must hold.
    let key_a = crate::scraping::boards::common::canonical_job_key(
        "https://a.example.com/1",
        "Rust Developer",
        "Acme",
    );
    let key_b = crate::scraping::boards::common::canonical_job_key(
        "https://b.example.com/2",
        "Rust Developer",
        "Acme",
    );
    let mut tombstones = std::collections::HashSet::new();
    tombstones.insert(crate::dedup::DedupStore::pair(&key_a, &key_b));

    for cycle in 0..2 {
        store.record_run(
            &id,
            2,
            0,
            vec![a.clone(), b.clone()],
            Vec::new(),
            &tombstones,
            &[],
        );
        let jobs = store.get(&id).unwrap().found_jobs;
        let ca = cluster_of(&jobs, "a.example");
        let cb = cluster_of(&jobs, "b.example");
        assert_ne!(ca, cb, "tombstone split must survive cycle {cycle}");
    }
}
