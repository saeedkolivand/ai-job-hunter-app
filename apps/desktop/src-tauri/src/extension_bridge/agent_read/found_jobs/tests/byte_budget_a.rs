//! Byte-budget trimming tests, part A: typical/oversized page trimming and `trim_page_to_budget`.

use super::super::*;
use super::support::*;
use crate::autopilot::FoundJob;

#[test]
fn found_jobs_typical_compact_page_rarely_needs_trimming() {
    const MCP_RESULT_MAX_BYTES: usize = 256 * 1024;
    let jobs: Vec<FoundJob> = (0..MAX_FOUND_JOBS_LIMIT)
        .map(richest_realistic_job)
        .collect();
    let records = vec![autopilot_with_jobs("ap-1", jobs)];
    let out = resolve_found_jobs(
        &records,
        Some("ap-1"),
        &no_filters(),
        &no_applied(),
        0,
        MAX_FOUND_JOBS_LIMIT,
    )
    .unwrap();
    assert_eq!(
        out["jobs"].as_array().unwrap().len(),
        MAX_FOUND_JOBS_LIMIT,
        "an ordinary full COMPACT page must not need trimming"
    );
    let bytes = out.to_string().len();
    assert!(
        bytes < MCP_RESULT_MAX_BYTES,
        "an ordinary full page must stay under the MCP cap, was {bytes} bytes"
    );

    // B3-r1-F6 — the assertion above alone cannot fail on the regression
    // issue #1167 reports: BOTH a compact page and a full-description page
    // over this same fixture set sit comfortably under a 256 KiB ceiling, so
    // a description silently back on every default row would still pass it.
    // Compare against the SAME fixture set with `description` opted in
    // instead — a compact page must stay a small fraction of that size, a
    // property a shape regression actually breaks.
    let with_desc = FoundJobsFilters::from_payload(&json!({ "includeDescription": true })).unwrap();
    let full = resolve_found_jobs(
        &records,
        Some("ap-1"),
        &with_desc,
        &no_applied(),
        0,
        MAX_FOUND_JOBS_LIMIT,
    )
    .unwrap();
    let full_bytes = full.to_string().len();
    assert!(
        bytes * 3 < full_bytes,
        "a compact page must stay a small fraction of the same page with description opted \
         in, or the compact shape stopped actually being compact: compact {bytes} vs full \
         {full_bytes}"
    );
}

#[test]
fn found_jobs_trims_an_oversized_page_and_keeps_the_cursor_correct() {
    const MCP_RESULT_MAX_BYTES: usize = 256 * 1024;
    let total_jobs = MAX_FOUND_JOBS_LIMIT * 2;
    let jobs: Vec<FoundJob> = (0..total_jobs).map(worst_permitted_job).collect();
    let records = vec![autopilot_with_jobs("ap-1", jobs)];
    let with_desc = FoundJobsFilters::from_payload(&json!({ "includeDescription": true })).unwrap();

    let page1 = resolve_found_jobs(
        &records,
        Some("ap-1"),
        &with_desc,
        &no_applied(),
        0,
        MAX_FOUND_JOBS_LIMIT,
    )
    .unwrap();
    let kept = page1["jobs"].as_array().unwrap().len();
    assert!(
        kept < MAX_FOUND_JOBS_LIMIT,
        "worst-permitted content must actually trigger trimming, kept {kept} of \
         {MAX_FOUND_JOBS_LIMIT} requested"
    );
    assert!(kept > 0, "at least one row must always come back");
    let bytes = page1.to_string().len();
    assert!(
        bytes < MCP_RESULT_MAX_BYTES,
        "a trimmed page must stay under the MCP cap, was {bytes} bytes"
    );
    assert_eq!(
        page1["nextCursor"].as_str().unwrap(),
        format!("{}:{kept}", issuer(Some("ap-1"))),
        "nextCursor must reflect rows ACTUALLY kept, not the requested limit"
    );

    // The next page must start exactly at `kept` — no row skipped, none repeated.
    let page2 = resolve_found_jobs(
        &records,
        Some("ap-1"),
        &with_desc,
        &no_applied(),
        kept,
        MAX_FOUND_JOBS_LIMIT,
    )
    .unwrap();
    let first_url_page2 = page2["jobs"][0]["url"].as_str().unwrap();
    assert_eq!(
        first_url_page2,
        format!("https://boards.example.com/jobs/{kept}"),
        "the row immediately after the trimmed page must be next, not skipped or repeated"
    );
}

#[test]
fn trim_page_to_budget_keeps_everything_when_already_under_budget() {
    let small: Vec<Value> = (0..5).map(|i| json!({ "i": i })).collect();
    let trimmed = trim_page_to_budget(small.clone(), 0);
    assert_eq!(trimmed, small);
}

#[test]
fn trim_page_to_budget_drops_rows_from_the_end_until_it_fits() {
    let row = json!({ "s": "a".repeat(1000) });
    let row_len = serde_json::to_string(&row).unwrap().len();
    let candidates: Vec<Value> = (0..500).map(|_| row.clone()).collect();
    let trimmed = trim_page_to_budget(candidates, 0);
    assert!(
        !trimmed.is_empty() && trimmed.len() < 500,
        "must actually trim"
    );
    let bytes = serde_json::to_string(&trimmed).unwrap().len();
    assert!(
        bytes <= PAGE_BYTE_BUDGET,
        "trimmed output must fit the budget: {bytes}"
    );
    assert!(
        bytes + 1 + row_len > PAGE_BYTE_BUDGET,
        "the trim boundary must be exact — one more row should have overflowed the budget"
    );
}

#[test]
fn trim_page_to_budget_always_keeps_at_least_one_row() {
    let huge_row = json!({ "s": "a".repeat(PAGE_BYTE_BUDGET * 2) });
    let trimmed = trim_page_to_budget(vec![huge_row.clone(), huge_row], 0);
    assert_eq!(trimmed.len(), 1, "must keep exactly one row, never zero");
}

#[test]
fn trim_page_to_budget_a_larger_base_cost_leaves_less_room_for_rows() {
    let row = json!({ "s": "a".repeat(1000) });
    let candidates: Vec<Value> = (0..200).map(|_| row.clone()).collect();
    let kept_with_no_base = trim_page_to_budget(candidates.clone(), 0).len();
    let kept_with_big_base = trim_page_to_budget(candidates, 50_000).len();
    assert!(
        kept_with_big_base < kept_with_no_base,
        "a non-zero base_cost must leave strictly less room for rows: {kept_with_big_base} \
         vs {kept_with_no_base}"
    );
}
