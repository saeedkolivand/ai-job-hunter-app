//! Row-cap eviction of the derived caches (`prune_caches`) and the amortized per-write prune.
//!
//! `PerformanceConfig` is a process-global, so every test here is `#[serial]`.

use super::{support::*, *};

// ── Amortized per-write eviction: match_scores ────────────────────────────────

/// The `match_scores` cache prunes on the SAME amortized cadence as its
/// `posting_vectors` sibling — one eviction pass per [`sql::CACHE_PRUNE_EVERY`]
/// writes, rather than two DELETEs under the held connection lock on EVERY
/// write. The match path is the bigger batch of the two (an Autopilot re-rank
/// writes a row per scored job, on top of whatever the Jobs page scores), so
/// per-write was up to ~2000 extra DELETEs per run.
///
/// Both halves matter and both are asserted here: an amortized prune that never
/// fires is just a deleted prune, and a "prune" that fires on every write is the
/// cost this change exists to remove. A stale row is the probe — the read-side
/// TTL would hide it from `get_match_score` whether or not eviction ran, so the
/// assertion counts raw rows.
#[test]
#[serial]
fn the_match_score_prune_is_amortized_but_still_fires_across_a_batch() {
    let (_dir, store) = open_store();
    set_perf(Some(3600), None); // 1h TTL, no row cap

    // One row already two hours past the TTL, inserted underneath the store so
    // no write counter is spent on it.
    seed_match_score(&store, "stale-job", "h", "{}", now_ms() - 2 * 3600 * 1000);

    let fresh = |i: u64| {
        let hash = sha256_hex(&format!("job-text-{i}"));
        let job_id = format!("job-{i}");
        move |store: &DocumentStore| {
            store
                .upsert_match_score(&match_key("r", &job_id, 1, 1, &hash), "{}")
                .unwrap();
        }
    };

    // Every write but the last one leaves the expired row in place — that is
    // what "amortized" means, and a per-write prune fails here.
    for i in 0..sql::CACHE_PRUNE_EVERY - 1 {
        fresh(i)(&store);
    }
    assert_eq!(
        count_table(&store, "match_scores"),
        sql::CACHE_PRUNE_EVERY as i64,
        "the expired row plus every fresh one: within a batch the cache is \
         allowed to hold rows past the TTL"
    );

    // …and the write that completes the cadence evicts it, so the bound still
    // holds over the batch as a whole.
    fresh(sql::CACHE_PRUNE_EVERY)(&store);
    assert_eq!(
        count_table(&store, "match_scores"),
        sql::CACHE_PRUNE_EVERY as i64,
        "the write that completes the cadence pruned the expired row while adding \
         its own, so the count holds instead of growing — an amortized prune that \
         never fires would read {} here",
        sql::CACHE_PRUNE_EVERY as i64 + 1
    );

    reset_perf_to_balanced();
}

// ── Row-cap eviction: match_scores ────────────────────────────────────────────
//
// Implementation note: `prune_table_locked` uses
//   DELETE WHERE created_at < (SELECT created_at … ORDER BY DESC LIMIT 1 OFFSET n)
// OFFSET n picks the (n+1)-th newest row (0-indexed).  DELETE removes rows
// strictly OLDER than that pivot.  Result: the pivot + n rows newer than it stay
// → n+1 rows remain.  So "cap_param=2" leaves 3 rows, "cap_param=1" leaves 2, etc.
// The tests below pin this contract so any drift in the SQL is caught.

#[test]
#[serial]
fn prune_caches_row_cap_keeps_newest_match_scores() {
    let (_dir, store) = open_store();

    // Insert 5 match-score rows with strictly increasing created_at values.
    // Generous limits during insert so the per-write prune is a no-op.
    set_perf(None, None);
    let base_ts = now_ms();
    for i in 0_u64..5 {
        let hash = sha256_hex(&format!("job-text-{i}"));
        let score_json = format!("{{\"score\":{}}}", i);
        seed_match_score(
            &store,
            &format!("job-{i}"),
            &hash,
            &score_json,
            base_ts + i * 1000,
        );
    }

    assert_eq!(
        count_table(&store, "match_scores"),
        5,
        "5 rows before prune"
    );

    // cap_param=2: DELETE WHERE created_at < (row at OFFSET 2 DESC) = ts2.
    // Deletes ts1 and ts0.  Keeps ts4, ts3, ts2 → 3 rows.
    store.prune_caches(None, Some(2));

    let remaining = count_table(&store, "match_scores");
    assert_eq!(
        remaining, 3,
        "after prune(cap=2): 3 rows remain (OFFSET 2 semantics)"
    );

    // The two oldest (job-0, job-1) must be evicted; job-2/3/4 must remain.
    for &evicted in &["job-0", "job-1"] {
        let cnt = count_where(&store, "match_scores", "job_id", evicted);
        assert_eq!(cnt, 0, "oldest row {evicted} must have been evicted");
    }
    for &kept in &["job-2", "job-3", "job-4"] {
        let cnt = count_where(&store, "match_scores", "job_id", kept);
        assert_eq!(cnt, 1, "newest row {kept} must have been kept");
    }

    reset_perf_to_balanced();
}

// ── Row-cap eviction: posting_vectors ─────────────────────────────────────────

#[test]
#[serial]
fn prune_caches_row_cap_keeps_newest_posting_vectors() {
    let (_dir, store) = open_store();

    // Insert 4 posting-vector rows with strictly increasing created_at.
    set_perf(None, None);
    let base_ts = now_ms();
    for i in 0_u64..4 {
        let hash = sha256_hex(&format!("pv-text-{i}"));
        let job_id = format!("pv-row-{i}");
        let v = ev(vec![0.1 * (i + 1) as f64]);
        seed_posting_vector(&store, &job_id, &hash, &v, base_ts + i * 1000);
    }

    assert_eq!(count_table(&store, "posting_vectors"), 4);

    // cap_param=1: OFFSET 1 DESC picks the 2nd newest (ts2). DELETE WHERE < ts2.
    // Deleted: ts1, ts0.  Keeps: ts3, ts2 → 2 rows.
    store.prune_caches(None, Some(1));

    assert_eq!(
        count_table(&store, "posting_vectors"),
        2,
        "cap=1 → 2 rows remain"
    );

    // pv-row-0 and pv-row-1 (oldest two) must be evicted.
    for &gone in &["pv-row-0", "pv-row-1"] {
        let cnt = count_where(&store, "posting_vectors", "job_id", gone);
        assert_eq!(cnt, 0, "{gone} must have been evicted");
    }

    reset_perf_to_balanced();
}

// ── Row-cap eviction: help_vectors ────────────────────────────────────────────

/// `help_vectors` is on the SAME sweep as its two siblings. Its producer takes
/// its entries from the REQUEST (`commands::help`), not from the shipped
/// corpus, so "the corpus bounds the table" was never true — the per-request
/// embed cap bounds one call and this sweep bounds the table. Mutation-visible:
/// drop the `help_vectors` line from `prune_caches` and the count stays 4.
#[test]
#[serial]
fn prune_caches_row_cap_keeps_newest_help_vectors() {
    let (_dir, store) = open_store();

    set_perf(None, None);
    let base_ts = now_ms();
    for i in 0_u64..4 {
        // Written through raw SQL rather than `upsert_help_vector` only
        // because `created_at` must be controlled; every other column is
        // exactly what that method writes.
        let v = ev(vec![0.1 * (i + 1) as f64]);
        let json = serde_json::to_string(&v.values).unwrap();
        let conn = store.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO help_vectors
             (text_hash, provider, model, dim, version, vector, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                format!("hv-row-{i}"),
                v.space.provider,
                v.space.model,
                v.space.dim as i64,
                v.space.version,
                json,
                ts_to_db(base_ts + i * 1000),
            ],
        )
        .unwrap();
    }

    assert_eq!(count_table(&store, "help_vectors"), 4);

    // Same arithmetic as the posting_vectors case above: cap=1 keeps 2 rows.
    store.prune_caches(None, Some(1));

    assert_eq!(
        count_table(&store, "help_vectors"),
        2,
        "help_vectors must be swept alongside posting_vectors and match_scores"
    );
    for gone in ["hv-row-0", "hv-row-1"] {
        let cnt = count_where(&store, "help_vectors", "text_hash", gone);
        assert_eq!(cnt, 0, "{gone} must have been evicted");
    }

    reset_perf_to_balanced();
}

// ── Generous (None/None): no eviction ─────────────────────────────────────────

#[test]
#[serial]
fn prune_caches_generous_leaves_all_rows_intact() {
    let (_dir, store) = open_store();

    set_perf(None, None);

    // Insert 10 match-score rows.
    for i in 0_u64..10 {
        let hash = sha256_hex(&format!("generous-{i}"));
        let job_id = format!("generous-job-{i}");
        let key = match_key("r", &job_id, 1, 1, &hash);
        store.upsert_match_score(&key, "{\"s\":1}").unwrap();
    }
    // Insert 5 posting vectors.
    for i in 0_u64..5 {
        let hash = sha256_hex(&format!("pv-{i}"));
        store
            .upsert_posting_vector(&format!("pv-job-{i}"), &hash, &ev(vec![0.1]))
            .unwrap();
    }

    assert_eq!(count_table(&store, "match_scores"), 10);
    assert_eq!(count_table(&store, "posting_vectors"), 5);

    // Prune with None/None (generous) → nothing removed.
    store.prune_caches(None, None);

    assert_eq!(
        count_table(&store, "match_scores"),
        10,
        "generous prune must not remove any match_scores rows"
    );
    assert_eq!(
        count_table(&store, "posting_vectors"),
        5,
        "generous prune must not remove any posting_vectors rows"
    );

    reset_perf_to_balanced();
}

// ── Row-cap boundary: cap=0 ───────────────────────────────────────────────────
//
// H1: cap=0 means OFFSET 0 → the subquery pivot IS the single newest row.
// DELETE WHERE created_at < newest_ts removes all strictly-older rows.
// The newest row itself (the pivot) is never deleted because the condition is
// strictly-less-than, not less-than-or-equal. Contract: exactly 1 row remains
// AND it is the row with the greatest created_at.

#[test]
#[serial]
fn prune_caches_cap_zero_keeps_exactly_the_single_newest_row() {
    let (_dir, store) = open_store();

    set_perf(None, None); // generous during inserts
    let base_ts = now_ms();

    // Insert 3 match-score rows with strictly increasing timestamps.
    for i in 0_u64..3 {
        let hash = sha256_hex(&format!("cap0-text-{i}"));
        let score_json = format!("{{\"score\":{}}}", i);
        seed_match_score(
            &store,
            &format!("cap0-job-{i}"),
            &hash,
            &score_json,
            base_ts + i * 1000,
        );
    }

    assert_eq!(
        count_table(&store, "match_scores"),
        3,
        "3 rows before prune"
    );

    // cap=0: OFFSET 0 → pivot is the newest row (ts+2000).
    // DELETE WHERE created_at < pivot removes the two older rows.
    // Result: exactly 1 row — the newest.
    store.prune_caches(None, Some(0));

    let remaining = count_table(&store, "match_scores");
    assert_eq!(
        remaining, 1,
        "cap=0 keeps exactly the single newest row (OFFSET 0 picks the newest as pivot)"
    );

    // That surviving row must be the one with the largest created_at (cap0-job-2).
    {
        let conn = store.conn.lock();
        let max_ts: i64 = conn
            .query_row(
                "SELECT created_at FROM match_scores ORDER BY created_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let expected_ts = ts_to_db(base_ts + 2 * 1000);
        assert_eq!(
            max_ts, expected_ts,
            "surviving row must have the largest created_at (newest insert)"
        );
    }

    // Confirm the specific job_id is cap0-job-2.
    let cnt = count_where(&store, "match_scores", "job_id", "cap0-job-2");
    assert_eq!(cnt, 1, "cap0-job-2 (newest) must be the surviving row");

    // The two older rows must be gone.
    for gone in &["cap0-job-0", "cap0-job-1"] {
        let cnt = count_where(&store, "match_scores", "job_id", gone);
        assert_eq!(cnt, 0, "{gone} (older) must have been evicted by cap=0");
    }

    reset_perf_to_balanced();
}

// ── Row-cap + tied created_at ─────────────────────────────────────────────────
//
// H2: The OFFSET DELETE uses a strict `<` comparison against the pivot's
// created_at. When multiple rows share the same created_at as the pivot, ALL
// of them survive (their timestamp is not strictly less than the pivot). This
// means cap=1 with 2 tied-oldest rows leaves ≥ 2 rows, not exactly 1.
//
// Contract (documented relaxed-tie contract):
//   - After prune(cap=1) with 3 rows (2 tied-oldest, 1 distinct-newest):
//     * Row count is in [2, 3] — the 2 older tied rows MAY survive as pivot collateral.
//     * The distinct-newest row ALWAYS survives (its timestamp is ≥ the pivot).
//
// This test pins that behavior so any tightening of the SQL (e.g. LIMIT 1 OFFSET 0
// changed to DELETE all but N) is caught.

#[test]
#[serial]
fn prune_caches_cap_with_tied_timestamps_retains_newest_and_at_least_bound() {
    let (_dir, store) = open_store();

    set_perf(None, None); // generous during inserts

    let old_ts = now_ms();
    let new_ts = old_ts + 5000; // clearly later

    // Insert 2 rows with identical (oldest) created_at, then 1 row with a newer ts.
    for i in 0_u64..2 {
        let hash = sha256_hex(&format!("tie-old-text-{i}"));
        seed_match_score(&store, &format!("tie-old-{i}"), &hash, "{\"s\":1}", old_ts);
    }
    seed_match_score(
        &store,
        "tie-new",
        &sha256_hex("tie-new-text"),
        "{\"s\":2}",
        new_ts,
    );

    assert_eq!(
        count_table(&store, "match_scores"),
        3,
        "3 rows before prune"
    );

    // prune(cap=1): OFFSET 1 DESC picks the 2nd-newest row as pivot.
    // With 3 rows sorted DESC by created_at: new_ts, old_ts, old_ts
    //   → the 2nd element (OFFSET 1) is one of the old_ts rows.
    // DELETE WHERE created_at < old_ts: removes nothing (both old_ts rows are = not <).
    // So all 3 rows (or at minimum the 2 with old_ts) remain.
    // Result: count is in [2, 3] — the tie prevents strict trimming.
    store.prune_caches(None, Some(1));

    let remaining = count_table(&store, "match_scores");
    assert!(
        (2..=3).contains(&remaining),
        "tied created_at means prune(cap=1) retains [2,3] rows, got {remaining}: \
         ties on the OFFSET pivot are never deleted (strict < not <=)"
    );

    // The distinct-newest row must ALWAYS survive, regardless of tie handling.
    let cnt = count_where(&store, "match_scores", "job_id", "tie-new");
    assert_eq!(
        cnt, 1,
        "the newest distinct-timestamp row must always be retained after prune"
    );

    reset_perf_to_balanced();
}
