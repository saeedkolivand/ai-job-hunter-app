//! Tests for `best-matches`' cursor paging and `query` filter (`best_matches.rs`).

use super::super::*;
use super::best_matches_projection::full_best_match_row_json;

/// Issue #1146 P11 — `best-matches` gained the same `cursor`/`nextCursor`
/// paging `found-jobs` already had. Walks every row via `resolve_best_matches`
/// directly (no `AppHandle` needed, same pure/impure split as `found-jobs`),
/// proving the traversal covers every row exactly once and terminates with a
/// `null` cursor rather than looping forever. The cursor goes back through
/// the REAL parser (round 2 fix, B3-r1-F4 — `nextCursor` is now
/// `<query fingerprint>:<offset>`, not a bare offset), not a hand-rolled
/// `parse()`, so this fails if the two halves of the format ever disagree.
#[test]
fn best_matches_cursor_walks_every_row_exactly_once_then_terminates_with_null() {
    let rows: Vec<Value> = (0..25)
        .map(|i| {
            let mut row = full_best_match_row_json();
            row["url"] = json!(format!("https://boards.example.com/jobs/{i}"));
            row
        })
        .collect();

    let page_size = 10;
    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    let issuer = best_matches::best_matches_cursor_issuer(None);
    loop {
        let offset = best_matches::parse_best_matches_cursor(&json!({ "cursor": cursor }), &issuer)
            .expect("own cursor");
        let out = best_matches::resolve_best_matches(&rows, offset, page_size, None);
        for row in out["matches"].as_array().unwrap() {
            seen.push(row["url"].as_str().unwrap().to_string());
        }
        match out["nextCursor"].as_str() {
            Some(next) => cursor = Some(next.to_string()),
            None => break,
        }
        assert!(seen.len() <= rows.len(), "must terminate at the true end");
    }

    assert_eq!(
        seen.len(),
        rows.len(),
        "every row must be seen exactly once"
    );
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), rows.len(), "no row must repeat across pages");
}

/// A cursor issued under one `query` replayed under a DIFFERENT one must
/// refuse rather than silently page the new query's list at the old query's
/// stale offset — the B3-r1-F4 hazard this fix closes.
#[test]
fn best_matches_cursor_issued_under_one_query_is_rejected_under_another() {
    let rows: Vec<Value> = (0..25)
        .map(|i| {
            let mut row = full_best_match_row_json();
            row["url"] = json!(format!("https://boards.example.com/jobs/{i}"));
            row
        })
        .collect();
    let issued = best_matches::resolve_best_matches(&rows, 0, 10, Some("engineer"))["nextCursor"]
        .as_str()
        .expect("more pages")
        .to_string();

    let err = best_matches::parse_best_matches_cursor(
        &json!({ "cursor": issued }),
        &best_matches::best_matches_cursor_issuer(Some("designer")),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        best_matches::BEST_MATCHES_WRONG_QUERY_CURSOR_MESSAGE
    );
}

/// The pre-round-2 wire shape (a bare numeric offset) is rejected, not
/// accepted for compatibility — same reasoning as
/// `found_jobs::found_jobs_rejects_a_bare_numeric_offset_cursor`.
#[test]
fn best_matches_rejects_a_bare_numeric_offset_cursor() {
    let err = best_matches::parse_best_matches_cursor(
        &json!({ "cursor": "10" }),
        &best_matches::best_matches_cursor_issuer(None),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        best_matches::BEST_MATCHES_MALFORMED_CURSOR_MESSAGE
    );
}

/// `best-matches`' `query` must go through the SAME hardened parse
/// `found-jobs` uses for its own `query`/`country` (round 2 fix, B3-r2-F1/
/// B3-r2-F2) — a wrong-typed or present-but-blank value refuses rather than
/// silently reading as "absent" and handing back the unfiltered ranked list
/// with a `total` the caller reads as filtered. Drives [`parse_best_matches_args`]
/// itself, not `found_jobs::trimmed_lowercase_filter` directly (round 3 fix,
/// B3-r3-F7 — the previous version of this test called the shared helper
/// directly, pinning nothing about `best_matches_resource`'s ACTUAL call
/// site; reverting that call site to the old `.and_then(Value::as_str)`
/// combinator left the whole suite green). `parse_best_matches_args` needs
/// no `AppHandle` — only [`best_matches_resource`] adds the
/// `autopilot_best_matches` call this can't reach.
#[test]
fn best_matches_query_filter_refuses_a_wrong_typed_or_blank_value() {
    for bad in [json!(true), json!(5), json!(""), json!("   ")] {
        let err = best_matches::parse_best_matches_args(&json!({ "query": bad })).unwrap_err();
        assert!(
            err.to_string().contains("query"),
            "refusal must name the key: {err}"
        );
    }
    let (query, offset) = best_matches::parse_best_matches_args(&json!({})).unwrap();
    assert_eq!(query, None, "an OMITTED query must still mean no filter");
    assert_eq!(offset, 0, "no cursor means start at the first page");
}

/// The `query` filter itself must actually narrow the row set — every test
/// above this one only exercises cursor issuance/refusal or the argument
/// PARSE, never whether `resolve_best_matches`' own `retain` actually drops
/// a non-matching row or matches by EITHER `title` or `company` (mirrors
/// `found_jobs::tests::found_jobs_query_filter_matches_title_or_company_case_insensitively`,
/// one resource over — this same predicate, hand-rolled here as
/// `resolve_best_matches`' own `.retain(...)` rather than reused from
/// `found_jobs`). Mutation check: deleting the `if let Some(q) = query {
/// matches.retain(...) }` block in `resolve_best_matches` makes this fail —
/// `total`/`returned` would read 3 instead of 1, and the `miss`/`by_title`
/// rows would leak into `matches`.
#[test]
fn best_matches_query_filter_matches_title_or_company_case_insensitively() {
    let mut by_title = full_best_match_row_json();
    by_title["title"] = json!("Senior Backend Engineer");
    by_title["company"] = json!("Acme");
    by_title["url"] = json!("https://boards.example.com/jobs/1");

    let mut by_company = full_best_match_row_json();
    by_company["title"] = json!("Frontend Developer");
    by_company["company"] = json!("Roboto Widgets");
    by_company["url"] = json!("https://boards.example.com/jobs/2");

    let mut miss = full_best_match_row_json();
    miss["title"] = json!("Sales Associate");
    miss["company"] = json!("Nope Inc");
    miss["url"] = json!("https://boards.example.com/jobs/3");

    // `resolve_best_matches` receives an already-lowercased `query` (the
    // real call site normalizes it via `parse_best_matches_args` →
    // `found_jobs::trimmed_lowercase_filter` before this fn ever runs), so
    // the fixture passes the lowercase form directly while the SOURCE row
    // keeps mixed case — proving the match itself, not the caller's
    // normalization, is what makes this case-insensitive.
    let rows = vec![by_title, by_company, miss];
    let out = best_matches::resolve_best_matches(&rows, 0, 20, Some("roboto"));
    assert_eq!(
        out["total"], 1,
        "the query must exclude the two non-matching rows, not just narrow the page"
    );
    assert_eq!(out["returned"], 1);
    assert_eq!(
        out["matches"][0]["url"], "https://boards.example.com/jobs/2",
        "the surviving row must be the COMPANY match, proving `query` checks company too, \
         not only title"
    );
}

/// A row at the REAL permitted worst case: `title`/`company`/`location`
/// each pinned to `crate::prompt_fence::JOB_CAP` (8,000 chars), in
/// multi-byte CJK text (stresses the char-vs-byte distinction — a
/// char-counted cap is NOT a byte cap). Mirrors
/// `found_jobs::tests::worst_permitted_job`'s own reasoning one resource
/// over — this is legitimate, non-adversarial content a board could
/// genuinely return, not an adversarial payload.
fn worst_permitted_best_match_row(n: usize) -> Value {
    let cjk_field = |cap: usize| "中".repeat(cap);
    let mut row = full_best_match_row_json();
    row["title"] = json!(cjk_field(crate::prompt_fence::JOB_CAP));
    row["company"] = json!(cjk_field(crate::prompt_fence::JOB_CAP));
    row["location"] = json!(cjk_field(crate::prompt_fence::JOB_CAP));
    row["url"] = json!(format!("https://boards.example.com/jobs/{n}"));
    row
}

/// Issue #1165 (HIGH) — a row-count `limit` alone cannot bound a page's byte
/// size: raising `MAX_BEST_MATCHES_LIMIT` to 100 without a byte-budget trim
/// let a max-limit page of worst-permitted rows reach ~7 MB, well past both
/// `agent_cli::mcp::MCP_RESULT_MAX_BYTES` (256 KiB) and, eventually,
/// `extension_bridge::mod::MAX_FRAME_BYTES`. Mirrors
/// `found_jobs::tests::found_jobs_trims_an_oversized_page_and_keeps_the_cursor_correct`
/// one resource over: this fails against the pre-fix `resolve_best_matches`,
/// which built `page` and returned it unconditionally.
#[test]
fn best_matches_trims_an_oversized_page_and_keeps_the_cursor_correct() {
    const MCP_RESULT_MAX_BYTES: usize = 256 * 1024;
    let total_rows = best_matches::MAX_BEST_MATCHES_LIMIT * 2;
    let rows: Vec<Value> = (0..total_rows)
        .map(worst_permitted_best_match_row)
        .collect();

    let page1 =
        best_matches::resolve_best_matches(&rows, 0, best_matches::MAX_BEST_MATCHES_LIMIT, None);
    let kept = page1["matches"].as_array().unwrap().len();
    assert!(
        kept < best_matches::MAX_BEST_MATCHES_LIMIT,
        "worst-permitted content must actually trigger trimming, kept {kept} of \
         {} requested",
        best_matches::MAX_BEST_MATCHES_LIMIT
    );
    assert!(kept > 0, "at least one row must always come back");
    let bytes = page1.to_string().len();
    assert!(
        bytes < MCP_RESULT_MAX_BYTES,
        "a trimmed page must stay under the MCP cap, was {bytes} bytes"
    );
    let issuer = best_matches::best_matches_cursor_issuer(None);
    assert_eq!(
        page1["nextCursor"].as_str().unwrap(),
        format!("{issuer}:{kept}"),
        "nextCursor must reflect rows ACTUALLY kept, not the requested limit"
    );

    // The next page must start exactly at `kept` — no row skipped, none repeated.
    let page2 =
        best_matches::resolve_best_matches(&rows, kept, best_matches::MAX_BEST_MATCHES_LIMIT, None);
    let first_url_page2 = page2["matches"][0]["url"].as_str().unwrap();
    assert_eq!(
        first_url_page2,
        format!("https://boards.example.com/jobs/{kept}"),
        "the row immediately after the trimmed page must be next, not skipped or repeated"
    );
}

// ── throttle ─────────────────────────────────────────────────────────────
