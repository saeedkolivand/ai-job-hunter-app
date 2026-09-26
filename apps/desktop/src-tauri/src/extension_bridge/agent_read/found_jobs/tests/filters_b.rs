//! Server-side filter tests (issue #1167), part B: `applied` store-unavailable refusal, `query`,
//! filtered `total`, and `FoundJobsFilters::from_payload` argument parsing (B3-r1-F3).

use super::super::*;
use super::support::*;
use crate::autopilot::FoundJob;
use std::collections::HashSet;

/// B3-r3-F1 — `applied_job_urls(app)` returns an EMPTY set both when the
/// user has applied to nothing AND when `ApplicationStore` failed to open
/// (a non-fatal boot path), so the `applied` filter must be refused, not
/// silently answered, when the store is unavailable — otherwise `applied:
/// true` would read as "you have applied to nothing" (`total: 0`) and
/// `applied: false` would silently return postings already applied to.
#[test]
fn applied_filter_refuses_rather_than_answering_wrong_when_the_store_is_unavailable() {
    let filters = FoundJobsFilters::from_payload(&json!({ "applied": true })).unwrap();
    let err = check_applied_filter_available(false, &filters).unwrap_err();
    assert_eq!(err.to_string(), APPLIED_FILTER_UNAVAILABLE_MESSAGE);

    let filters = FoundJobsFilters::from_payload(&json!({ "applied": false })).unwrap();
    let err = check_applied_filter_available(false, &filters).unwrap_err();
    assert_eq!(err.to_string(), APPLIED_FILTER_UNAVAILABLE_MESSAGE);
}

/// The store being unavailable must never block a call that never asked for
/// the `applied` filter — this is a targeted refusal, not a blanket outage.
#[test]
fn applied_filter_availability_is_a_no_op_when_the_filter_is_not_requested() {
    assert!(check_applied_filter_available(false, &no_filters()).is_ok());
    let filters = FoundJobsFilters::from_payload(&json!({ "applied": true })).unwrap();
    assert!(check_applied_filter_available(true, &filters).is_ok());
}

#[test]
fn found_jobs_query_filter_matches_title_or_company_case_insensitively() {
    let target = FoundJob {
        title: "Senior Backend Engineer".to_string(),
        company: "Acme Corp".to_string(),
        ..numbered_job(1)
    };
    let by_company = FoundJob {
        title: "Frontend Developer".to_string(),
        company: "Roboto Widgets".to_string(),
        ..numbered_job(2)
    };
    let miss = FoundJob {
        title: "Sales Associate".to_string(),
        company: "Nope Inc".to_string(),
        ..numbered_job(3)
    };
    let records = vec![autopilot_with_jobs("ap-1", vec![target, by_company, miss])];
    let filters = FoundJobsFilters::from_payload(&json!({ "query": "roboto" })).unwrap();
    let out = resolve_found_jobs(&records, Some("ap-1"), &filters, &no_applied(), 0, 20).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["jobs"][0]["url"], "https://boards.example.com/jobs/2");
}

#[test]
fn found_jobs_total_reflects_filtered_count_not_the_whole_store_unaffected_by_paging() {
    let jobs: Vec<FoundJob> = (0..30).map(numbered_job).collect();
    let records = vec![autopilot_with_jobs("ap-1", jobs)];
    let filters = FoundJobsFilters::from_payload(&json!({ "minScore": 0 })).unwrap();
    let page1 = resolve_found_jobs(&records, Some("ap-1"), &filters, &no_applied(), 0, 5).unwrap();
    let page2 = resolve_found_jobs(&records, Some("ap-1"), &filters, &no_applied(), 20, 5).unwrap();
    assert_eq!(page1["total"], 30);
    assert_eq!(
        page2["total"], 30,
        "total must not shrink because of a later offset"
    );
}

/// Round 2 fix (B3-r2-F6): unlike a `record_run` merge (which only ever
/// PREPENDS, so a stale offset can at worst re-return a row), `applied` is
/// re-derived fresh on every call and can REMOVE a row from the middle of
/// the candidate list between two pages of the SAME traversal — shifting
/// every later index down by one and making a stale absolute offset skip
/// exactly one row that still passes every filter and was never returned.
/// Demonstrates the exact mechanism the doc on [`resolve_found_jobs`] now
/// names: job 0 is returned on page 1, then becomes applied (excluded by
/// this call's `applied: false` filter) before page 2 is fetched at the
/// stale offset — job 2 is silently skipped, never appearing in either page.
#[test]
fn found_jobs_applied_narrowing_between_pages_skips_a_row_never_merely_repeats_one() {
    let jobs: Vec<FoundJob> = (0..4).map(numbered_job).collect();
    let records = vec![autopilot_with_jobs("ap-1", jobs)];
    let filters = FoundJobsFilters::from_payload(&json!({ "applied": false })).unwrap();

    let page1 = resolve_found_jobs(&records, Some("ap-1"), &filters, &no_applied(), 0, 2).unwrap();
    assert_eq!(page1["total"], 4);
    let returned_page1: Vec<String> = page1["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["url"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        returned_page1,
        vec![
            "https://boards.example.com/jobs/0",
            "https://boards.example.com/jobs/1",
        ]
    );
    let next_cursor = page1["nextCursor"].as_str().unwrap().to_string();
    let stale_offset: usize = next_cursor.rsplit_once(':').unwrap().1.parse().unwrap();
    assert_eq!(stale_offset, 2);

    // Job 0 (already returned, BEFORE the stale offset) becomes applied
    // between the two calls — the mid-traversal narrowing this test pins.
    let mut applied_urls = HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(
        &records[0].found_jobs[0].url,
    ));

    let page2 = resolve_found_jobs(
        &records,
        Some("ap-1"),
        &filters,
        &applied_urls,
        stale_offset,
        2,
    )
    .unwrap();
    let returned_page2: Vec<String> = page2["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["url"].as_str().unwrap().to_string())
        .collect();

    // Job 2 still passes every filter (it was never applied) and was never
    // returned on page 1 — yet it is absent from page 2 too, because the
    // stale offset now points one row too far into the shrunk list.
    assert!(
        !returned_page1
            .iter()
            .chain(returned_page2.iter())
            .any(|url| url == "https://boards.example.com/jobs/2"),
        "job 2 must have been silently skipped by the stale offset, pinning the doc's caveat: \
         page1={returned_page1:?} page2={returned_page2:?}"
    );
}

// ── B3-r1-F3: a present filter that fails to materialise must refuse,
// never silently drop and return the UNFILTERED page ──────────────────

/// `json!(non_finite_f64)` collapses to JSON `null` — RFC 8259 has no
/// `Infinity`/`NaN` token, so `serde_json::Value::Number` cannot represent
/// one BY CONSTRUCTION; there is no well-formed JSON text this fn could ever
/// read as a present-but-non-finite `minScore`. That is exactly why the
/// load-bearing half of the B3-r1-F3 fix sits at the CLI's OWN parse
/// (`agent_cli::parse_found_jobs`'s `--min-score` — see
/// `agent_cli::tests::rejects_found_jobs_a_non_finite_min_score`), before
/// `1e400`/`inf`/`nan` are ever handed to `json!` and turned into this same
/// indistinguishable `null`. This test pins the OTHER, intentional half:
/// `from_payload` must keep treating an explicit `null` the same as
/// "absent" — the established convention every other filter/cursor on this
/// resource already follows — so a caller that legitimately sends
/// `{"minScore": null}` to mean "no filter" is never refused.
#[test]
fn found_jobs_filters_from_payload_treats_a_null_min_score_as_absent() {
    assert!(
        json!(f64::INFINITY).is_null(),
        "pins the serde_json invariant the doc above relies on"
    );
    let filters = FoundJobsFilters::from_payload(&json!({ "minScore": null })).unwrap();
    assert_eq!(filters.min_score, None);
}

#[test]
fn found_jobs_filters_from_payload_rejects_a_wrong_typed_present_filter() {
    for (payload, key) in [
        (json!({ "minScore": "70" }), "minScore"),
        (json!({ "remote": "true" }), "remote"),
        (json!({ "applied": "false" }), "applied"),
        (json!({ "country": 5 }), "country"),
        (json!({ "query": true }), "query"),
        (
            json!({ "includeDescription": "true" }),
            "includeDescription",
        ),
    ] {
        let err = FoundJobsFilters::from_payload(&payload).unwrap_err();
        assert!(
            err.to_string().contains(key),
            "refusal for {payload} must name {key}: {err}"
        );
    }
}

/// Round 2 fix (B3-r2-F2): a PRESENT-but-blank/whitespace-only `query`/
/// `country` now refuses, the same as a wrong-typed one — it used to read as
/// "not set" and silently widen the call to the entire corpus with a
/// `total` the caller reads as filtered. The canonical repro is a shell
/// caller forwarding an unset variable straight through
/// (`agent found-jobs --query "$ROLE"` with `ROLE` empty). An OMITTED key
/// still means "no filter" — every other test in this file that calls
/// `no_filters()` (an empty payload) exercises that direction.
#[test]
fn found_jobs_filters_from_payload_rejects_a_blank_string_filter() {
    for (payload, key) in [
        (json!({ "country": "  " }), "country"),
        (json!({ "query": "" }), "query"),
    ] {
        let err = FoundJobsFilters::from_payload(&payload).unwrap_err();
        assert!(
            err.to_string().contains(key),
            "refusal for {payload} must name {key}: {err}"
        );
    }
}

// ── worst-case payload / trimming (issue #1167's compact-row shape) ───
