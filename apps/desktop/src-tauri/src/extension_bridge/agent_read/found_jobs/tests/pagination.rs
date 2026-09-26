//! Pagination tests: full coverage, deterministic replay, limit clamp, no repeats.

use super::super::*;
use super::support::*;
use crate::autopilot::FoundJob;

#[test]
fn found_jobs_on_an_empty_autopilot_returns_no_jobs_and_a_null_cursor() {
    let records = vec![autopilot_with_jobs("ap-1", vec![])];
    let out = resolve_found_jobs(&records, Some("ap-1"), &no_filters(), &no_applied(), 0, 20)
        .expect("found (empty)");
    assert_eq!(out["jobs"].as_array().unwrap().len(), 0);
    assert_eq!(out["nextCursor"], Value::Null);
    assert_eq!(out["total"], 0);
}

#[test]
fn found_jobs_pagination_covers_every_job_exactly_once_then_terminates() {
    let total_jobs = 25;
    let jobs: Vec<FoundJob> = (0..total_jobs).map(numbered_job).collect();
    let records = vec![autopilot_with_jobs("ap-1", jobs)];

    let page_size = 10;
    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        // The cursor goes back through the REAL parser (issue #1130), not a
        // hand-rolled `parse()` — that round trip is what proves an issued
        // cursor is actually accepted again, rather than only that the
        // digits inside it happen to be right.
        let offset = parse_found_jobs_cursor(&json!({ "cursor": cursor }), &issuer(Some("ap-1")))
            .expect("own cursor");
        let out = resolve_found_jobs(
            &records,
            Some("ap-1"),
            &no_filters(),
            &no_applied(),
            offset,
            page_size,
        )
        .expect("page resolves");
        for row in out["jobs"].as_array().unwrap() {
            seen.push(row["url"].as_str().unwrap().to_string());
        }
        match out["nextCursor"].as_str() {
            Some(next) => cursor = Some(next.to_string()),
            None => break,
        }
        assert!(seen.len() <= total_jobs, "must terminate at the true end");
    }

    assert_eq!(
        seen.len(),
        total_jobs,
        "every job must be seen exactly once"
    );
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), total_jobs, "no job must repeat across pages");
}

#[test]
fn found_jobs_same_cursor_returns_the_same_slice_deterministically() {
    let jobs: Vec<FoundJob> = (0..5).map(numbered_job).collect();
    let records = vec![autopilot_with_jobs("ap-1", jobs)];
    let a = resolve_found_jobs(&records, Some("ap-1"), &no_filters(), &no_applied(), 2, 2).unwrap();
    let b = resolve_found_jobs(&records, Some("ap-1"), &no_filters(), &no_applied(), 2, 2).unwrap();
    assert_eq!(
        a, b,
        "repeated calls with the same offset must be identical"
    );
}

#[test]
fn found_jobs_limit_is_honored_and_capped_server_side() {
    let payload = json!({ "limit": 5_000 });
    assert_eq!(clamp_found_jobs_limit(&payload), MAX_FOUND_JOBS_LIMIT);
    let default_payload = json!({});
    assert_eq!(
        clamp_found_jobs_limit(&default_payload),
        DEFAULT_FOUND_JOBS_LIMIT
    );
    // A zero/garbage limit must not widen to "unbounded" — it falls back
    // to the default, never to `usize::MAX` or an empty page forever.
    let zero_payload = json!({ "limit": 0 });
    assert_eq!(
        clamp_found_jobs_limit(&zero_payload),
        DEFAULT_FOUND_JOBS_LIMIT
    );
}

/// T5 — `project_found_job_row` is now INFALLIBLE (see its own doc), so the
/// old "a projection failure is dropped after being counted in `total`"
/// class is gone by construction: every candidate in the `[offset,
/// offset + limit)` window always yields exactly one row, so a full page
/// always advances the cursor (or terminates), never re-issues the same
/// offset.
#[test]
fn found_jobs_pagination_never_reissues_the_same_cursor_for_a_full_page() {
    let jobs: Vec<FoundJob> = (0..3).map(numbered_job).collect();
    let records = vec![autopilot_with_jobs("ap-1", jobs)];
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &no_filters(), &no_applied(), 0, 20).unwrap();
    assert_eq!(out["total"], 3);
    assert_eq!(out["jobs"].as_array().unwrap().len(), 3);
    assert!(
        out["nextCursor"].is_null(),
        "a fully-returned page must terminate the traversal, not reissue a cursor"
    );
}
