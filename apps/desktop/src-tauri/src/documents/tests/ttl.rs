//! TTL behaviour of the derived caches: `prune_caches` drops rows older than the cutoff, and the
//! read-side cutoff hides an expired row before any prune has run.
//!
//! `PerformanceConfig` is a process-global, so every test here is `#[serial]`.

use super::{support::*, *};

// ── TTL eviction: prune_caches removes rows older than the cutoff ─────────────

#[test]
#[serial]
fn prune_caches_ttl_removes_old_match_scores() {
    let (_dir, store) = open_store();

    // Insert a row with created_at = now - 2 hours (7200 seconds ago).
    let old_ts = now_ms().saturating_sub(7200 * 1000);
    let new_ts = now_ms();

    set_perf(None, None); // generous during inserts

    for (job_id, ts) in [("old-job", old_ts), ("new-job", new_ts)] {
        seed_match_score(&store, job_id, &sha256_hex(job_id), "{\"s\":1}", ts);
    }

    assert_eq!(count_table(&store, "match_scores"), 2);

    // TTL = 3600 s (1 hour): the old-job row (2h old) is past the cutoff; new-job is not.
    store.prune_caches(Some(3600), None);

    assert_eq!(count_table(&store, "match_scores"), 1);
    let cnt = count_where(&store, "match_scores", "job_id", "new-job");
    assert_eq!(cnt, 1, "new-job must survive TTL prune");

    reset_perf_to_balanced();
}

// ── TTL eviction: prune_caches removes old posting_vectors ────────────────────
//
// M4: Mirror of `prune_caches_ttl_removes_old_match_scores` for posting_vectors.
// The helper `prune_table_locked` is shared; both call sites must be pinned.

#[test]
#[serial]
fn prune_caches_ttl_removes_old_posting_vectors() {
    let (_dir, store) = open_store();

    // Insert one old row (2 hours ago) and one fresh row (now).
    let old_ts = now_ms().saturating_sub(7200 * 1000);
    let new_ts = now_ms();

    set_perf(None, None); // generous during inserts

    for (job_id, ts) in [("pv-old-job", old_ts), ("pv-new-job", new_ts)] {
        seed_posting_vector(&store, job_id, &sha256_hex(job_id), &ev(vec![0.1, 0.2]), ts);
    }

    assert_eq!(count_table(&store, "posting_vectors"), 2);

    // TTL = 3600 s (1 hour): the old row (2h old) is past the cutoff; new row is not.
    store.prune_caches(Some(3600), None);

    assert_eq!(
        count_table(&store, "posting_vectors"),
        1,
        "TTL prune must remove the 2-hour-old posting_vectors row"
    );

    let cnt = count_where(&store, "posting_vectors", "job_id", "pv-new-job");
    assert_eq!(cnt, 1, "pv-new-job must survive the TTL prune");

    reset_perf_to_balanced();
}

// ── Read-side TTL: get_match_score returns None for an expired row ─────────────
//
// The read path uses `ttl_cutoff_ms()` which reads the live global — we set a
// very small TTL so the row's age (inserted at now_ms() - a few ms) exceeds it.
// We achieve "expiry" by setting a negative TTL seconds value (the prune SQL
// saturates, but the read cutoff formula allows negative: now - (neg * 1000) >
// created_at when neg is large enough that cutoff > created_at). Use a large
// negative TTL to force the cutoff into the future.
#[test]
#[serial]
fn get_match_score_returns_none_for_expired_row_via_live_ttl() {
    let (_dir, store) = open_store();

    // Insert a fresh row.
    let hash = sha256_hex("expire-me");
    let key = match_key("r", "j", 1, 1, &hash);
    // Insert with generous limits to avoid per-write eviction interfering.
    set_perf(None, None);
    store.upsert_match_score(&key, "{\"s\":1}").unwrap();

    // Confirm the row is a hit under generous TTL.
    assert!(
        store.get_match_score(&key).is_some(),
        "row must be present under no-TTL config"
    );

    // Set a TTL so large (negative) that the cutoff is in the future: every row
    // is "expired". ttl_cutoff_ms() = now_ms() - ttl_secs * 1000. With ttl_secs
    // = i64::MIN / 1000 the subtraction overflows and clamps to i64::MAX via
    // saturating_sub in the production code — that would make cutoff = MAX → all
    // rows expire. However: `prune_table_locked` uses saturating_sub, but
    // `ttl_cutoff_ms` uses saturating_sub too. Let's use a large negative value
    // that keeps the arithmetic well-behaved: -i64::MAX (not i64::MIN to avoid
    // any edge on platforms). A TTL of -1_000_000 means
    // cutoff = now_ms_as_i64 - (-1_000_000 * 1000) = now + 1_000_000_000 ms
    // which is far in the future → every existing row is "before" that → miss.
    set_perf(Some(-1_000_000), None);

    assert!(
        store.get_match_score(&key).is_none(),
        "row must be a read-side TTL miss when the cutoff is in the future"
    );

    reset_perf_to_balanced();
}

// ── Read-side TTL: get_posting_vector returns None for an expired row ──────────

#[test]
#[serial]
fn get_posting_vector_returns_none_for_expired_row_via_live_ttl() {
    let (_dir, store) = open_store();

    set_perf(None, None); // generous during insert
    let hash = sha256_hex("posting-expire");
    store
        .upsert_posting_vector("job-x", &hash, &ev(vec![0.1, 0.2]))
        .unwrap();

    assert!(
        store.get_posting_vector("job-x").is_some(),
        "posting vector must be present under no-TTL"
    );

    // Expire via negative TTL (same technique as match_score test above).
    set_perf(Some(-1_000_000), None);

    assert!(
        store.get_posting_vector("job-x").is_none(),
        "posting vector must be a read-side TTL miss when cutoff is in the future"
    );

    reset_perf_to_balanced();
}
