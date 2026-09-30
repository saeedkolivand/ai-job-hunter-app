use super::super::*;
use super::response_mapping::{jobs_from, make_ctx, make_input};

// ---------------------------------------------------------------------------
// search() — client-side query/location filters (network-free: results come
// from an already-fetched page, so these exercise the filter logic in
// isolation by constructing the haystack the same way `search()` does).
// ---------------------------------------------------------------------------

fn sample_postings() -> Vec<JobPosting> {
    let json = r#"{
        "results": [
            {"name": "Senior Backend Engineer", "refs": {"landing_page": "https://www.themuse.com/jobs/acme/backend"}, "company": {"name": "Acme Corp"}, "locations": [{"name": "Berlin, Germany"}]},
            {"name": "Product Designer", "refs": {"landing_page": "https://www.themuse.com/jobs/globex/designer"}, "company": {"name": "Globex"}, "locations": [{"name": "Remote"}]}
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    parse_themuse_response(jobs, 0)
}

/// Thin filter-a-slice wrapper around the real `matches_filters` — not a
/// reimplementation (that was the bug: a byte-duplicated mirror here could
/// diverge from `search()`'s actual filter and hide a regression). Every
/// test below now exercises the real extracted fn through this.
fn apply_filters(postings: &[JobPosting], query: &str) -> Vec<JobPosting> {
    postings
        .iter()
        .filter(|posting| matches_filters(posting, query))
        .cloned()
        .collect()
}

#[test]
fn query_filter_matches_title_case_insensitive() {
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "BACKEND");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Senior Backend Engineer");
}

#[test]
fn query_filter_matches_company_case_insensitive() {
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "globex");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].company, "Globex");
}

#[test]
fn query_filter_empty_returns_all() {
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "");
    assert_eq!(filtered.len(), 2);
}

#[test]
fn query_filter_matching_nothing_returns_empty() {
    let postings = sample_postings();
    let filtered = apply_filters(&postings, "nonexistent-role");
    assert!(filtered.is_empty());
}

#[test]
fn board_filter_does_not_drop_on_location() {
    // The Muse is `supports_location() == false`, so board-local filtering is
    // keyword-only: location is the engine's central `location_filter`. A query
    // that matches must keep the row whatever its location, and an empty query
    // keeps every row — the board must never zero itself on a picked city (the
    // original "New York, United States" vs "New York, NY" regression).
    let postings = sample_postings();
    // "Senior Backend Engineer" in "Berlin, Germany" is kept by the keyword
    // alone; the board applies no "Paris"/"Remote" location narrowing here.
    let filtered = apply_filters(&postings, "engineer");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Senior Backend Engineer");
    // No keyword → both rows pass, regardless of their differing locations.
    assert_eq!(apply_filters(&postings, "").len(), 2);
}

// ---------------------------------------------------------------------------
// total_pages / page budget clamp — pure arithmetic, no network needed.
// ---------------------------------------------------------------------------

#[test]
fn page_count_clamps_to_at_least_one() {
    // `resp.page_count.max(1)` — a zero/missing page_count must not zero out
    // the loop bound (which would make `page >= total_pages` true immediately
    // on page 1+ but still allow page 0 through). Values come from real
    // fixture deserialization, not literals, so this exercises the actual
    // `#[serde(default)]` behaviour, not just arithmetic.
    let missing: TmResponse = serde_json::from_str(r#"{"results": []}"#).unwrap();
    assert_eq!(missing.page_count, 0);
    assert_eq!(missing.page_count.max(1), 1);

    let zero: TmResponse = serde_json::from_str(r#"{"results": [], "page_count": 0}"#).unwrap();
    assert_eq!(zero.page_count.max(1), 1);

    let three: TmResponse = serde_json::from_str(r#"{"results": [], "page_count": 3}"#).unwrap();
    assert_eq!(three.page_count.max(1), 3);
}

#[test]
fn requested_pages_clamps_into_one_to_max_pages_range() {
    let want_zero = BoardSearchInput {
        pages: 0,
        provider_amount: None,
        ..make_input("", None)
    };
    assert_eq!(
        want_zero.pages.clamp(1, MAX_PAGES),
        1,
        "0 requested pages must clamp up to 1"
    );

    let want_one = BoardSearchInput {
        pages: 1,
        provider_amount: None,
        ..make_input("", None)
    };
    assert_eq!(want_one.pages.clamp(1, MAX_PAGES), 1);

    let want_oversized = BoardSearchInput {
        pages: 100,
        provider_amount: None,
        ..make_input("", None)
    };
    assert_eq!(
        want_oversized.pages.clamp(1, MAX_PAGES),
        MAX_PAGES,
        "an oversized request must clamp down to MAX_PAGES"
    );
}

// ---------------------------------------------------------------------------
// progress_denominator — on_progress must track real page work, not the
// request budget (regression: previously used `max_pages` unconditionally,
// so a feed with fewer real pages than the budget never reached 1.0).
// ---------------------------------------------------------------------------

#[test]
fn progress_denominator_uses_actual_page_count_when_smaller_than_budget() {
    // Real feed has 2 pages but the request budget allows up to 5 — progress
    // must be measured against the real 2, so `page=1` (the last page) yields
    // 2/2 = 1.0, not 2/5.
    assert_eq!(progress_denominator(2, 5), 2);
}

#[test]
fn progress_denominator_clamps_to_the_page_budget() {
    // A feed reporting more pages than the request is allowed to fetch must
    // not inflate the denominator past the actual work performed.
    assert_eq!(progress_denominator(10, 5), 5);
}

#[test]
fn progress_denominator_matches_budget_when_equal() {
    assert_eq!(progress_denominator(5, 5), 5);
}

// ---------------------------------------------------------------------------
// search() — network-free edge cases
// ---------------------------------------------------------------------------

/// A pre-cancelled signal must make the loop break immediately without
/// attempting a fetch, returning Ok with an empty Vec.
#[tokio::test]
async fn cancelled_before_fetch_returns_ok_empty() {
    let scraper = TheMuseScraper;
    let ctx = make_ctx();
    ctx.signal.cancel();
    let result = scraper.search(make_input("", None), ctx).await;
    assert!(
        result.is_ok(),
        "cancelled run must return Ok, not Err: {:?}",
        result.err()
    );
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = TheMuseScraper;
    let input = make_input("engineer", None);
    let ctx = make_ctx();
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        scraper.search(input, ctx),
    )
    .await
    .expect("live search timed out");
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    println!("themuse: {} results", postings.len());
    println!("first: {:?}", first.title);
}
