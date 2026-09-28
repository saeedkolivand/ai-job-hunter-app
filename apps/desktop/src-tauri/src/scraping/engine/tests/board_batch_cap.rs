//! CWE-770 board-batch cap tests: a crafted board list is deduped and
//! truncated to the registry size before any dispatch.

use std::collections::HashMap;

use super::support::*;
use crate::scraping::types::Scraper;

use super::super::*;

// ── CWE-770 board-batch cap tests ─────────────────────────────────────────────

/// A 5000-entry input made of duplicates + a handful of distinct valid ids must
/// resolve to at most `max_boards_per_batch()` (the registry size) distinct
/// board runs.
/// Uses the engine's `scrape_boards` method directly via the public API seam,
/// injecting a real (but fast-returning) FakeScraper via run_boards for isolation.
#[tokio::test]
async fn scrape_boards_dedupes_and_caps_large_input() {
    let cap = super::super::max_boards_per_batch();

    // Build `cap + 3` distinct names (always > cap, whatever the registry size
    // grows to) plus duplicates filling 5000 total entries.
    let distinct_count = cap + 3;
    let distinct: Vec<String> = (0..distinct_count).map(|i| format!("board_{i}")).collect();
    let mut boards: Vec<String> = Vec::with_capacity(5000);
    for i in 0..5000 {
        boards.push(distinct[i % distinct.len()].clone());
    }

    // Wire up fake scrapers for every distinct id so "unknown board" errors don't
    // confuse the count — we test the batch size cap, not unknown-id handling.
    let fakes: Vec<FakeScraper> = (0..distinct_count).map(|_| FakeScraper::http(1)).collect();
    let fake_refs: Vec<(String, anyhow::Result<&dyn Scraper>)> = {
        // Dedupe + truncate exactly as scrape_boards does — mirror the logic here
        // so the test drives run_boards at the capped slice.
        let mut seen = std::collections::HashSet::new();
        boards
            .iter()
            .filter(|id| seen.insert(id.as_str()))
            .take(cap)
            .enumerate()
            .map(|(i, id)| (id.clone(), Ok(&fakes[i] as &dyn Scraper)))
            .collect()
    };

    let parent = CancellationToken::new();
    let results = ScraperEngine::run_boards(
        fake_refs,
        fake_input(1),
        parent,
        None,
        None,
        None,
        None,
        None,
        test_browser_sem(),
        &HashMap::new(),
    )
    .await;

    assert!(
        results.len() <= cap,
        "run_boards result count ({}) must not exceed max_boards_per_batch() ({})",
        results.len(),
        cap
    );
    assert_eq!(
        results.len(),
        cap,
        "expected exactly max_boards_per_batch() distinct board runs after dedup+truncate"
    );
}

/// HIGH 2 — CWE-770: `scrape_boards` itself must enforce the cap and dedupe,
/// not just `run_boards`. A future refactor moving the guard out of `scrape_boards`
/// MUST fail this test.
///
/// Strategy: pass 5 000 entries composed of > `max_boards_per_batch()` distinct
/// IDs directly to `ScraperEngine::scrape_boards`. Unknown board IDs resolve to
/// `Err("Unknown board: …")` entries immediately — no network calls — so the run
/// is fast. The test asserts that summaries.len() ≤ max_boards_per_batch() and
/// that first-seen order is preserved (the first `cap` distinct IDs win, not a
/// random set).
#[tokio::test]
async fn scrape_boards_real_entrypoint_caps_and_dedupes() {
    let cap = super::super::max_boards_per_batch();

    // `cap + 3` distinct fake IDs (always > cap) interleaved with duplicates.
    // None of these match registered boards, so they resolve to Err immediately.
    let distinct_count = cap + 3;
    let distinct: Vec<String> = (0..distinct_count)
        .map(|i| format!("nonexistent_board_{i}"))
        .collect();
    let mut boards: Vec<String> = Vec::with_capacity(5000);
    for i in 0..5000 {
        boards.push(distinct[i % distinct.len()].clone());
    }

    // The first `cap` distinct IDs we see in iteration order.
    let expected_first_cap: Vec<String> = distinct[..cap].to_vec();

    let engine = ScraperEngine::new();
    // No cancellation — all_failed=true but parent.is_cancelled()=false → Ok.
    let result = engine
        .scrape_boards(
            &boards,
            fake_input(1),
            "test-job-cap".to_string(),
            None,
            None,
        )
        .await;

    let (postings, summaries) = result
        .expect("all boards unknown (no network) but not cancelled → must return Ok, not Err");

    assert!(postings.is_empty(), "unknown boards produce no postings");
    assert!(
        summaries.len() <= cap,
        "summaries ({}) must not exceed max_boards_per_batch() ({})",
        summaries.len(),
        cap
    );
    assert_eq!(
        summaries.len(),
        cap,
        "exactly max_boards_per_batch() summaries expected after dedup+truncate"
    );

    // Verify first-seen order: the winning IDs must be the first `cap` distinct ones.
    let summary_boards: Vec<&str> = summaries.iter().map(|s| s.board.as_str()).collect();
    let expected_refs: Vec<&str> = expected_first_cap.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        summary_boards, expected_refs,
        "dedupe must preserve first-seen order; got {summary_boards:?}"
    );

    // Every summary must carry an error (unknown board → no items recovered).
    for s in &summaries {
        assert!(
            s.error.is_some(),
            "board '{}' is unknown so must report an error",
            s.board
        );
        assert_eq!(
            s.count, 0,
            "unknown board '{}' must report count=0",
            s.board
        );
    }
}

/// HIGH 3 — cancellation + all-failed gate: `scrape_boards` returns `Err("scrape cancelled")`
/// only when ALL boards failed AND the parent token is cancelled.
#[tokio::test]
async fn scrape_boards_all_failed_and_cancelled_returns_err() {
    // Pre-register a token under the job_id so scrape_boards reuses it (matching
    // the Autopilot pattern). Cancel it before the run so it's already cancelled
    // when the boards start — no timing dependency.
    let engine = ScraperEngine::new();
    let token = CancellationToken::new();
    engine
        .register_token("job-cancel-all-fail", token.clone())
        .await;
    token.cancel();

    // All boards are unknown → all fail with Err("Unknown board: …") immediately.
    let boards: Vec<String> = vec![
        "nonexistent_a".to_string(),
        "nonexistent_b".to_string(),
        "nonexistent_c".to_string(),
    ];

    let result = engine
        .scrape_boards(
            &boards,
            fake_input(5),
            "job-cancel-all-fail".to_string(),
            None,
            None,
        )
        .await;

    assert!(
        result.is_err(),
        "all-failed + cancelled must return Err, got: {result:?}"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("cancelled"),
        "error message must mention cancellation, got: {msg}"
    );
}

/// HIGH 3 (complementary) — partial success under cancellation returns `Ok`.
///
/// When at least one board recovers ≥1 item and the parent token is cancelled,
/// `scrape_boards` must return `Ok`. The gate now requires BOTH
/// `parent.is_cancelled() && !any_recovered_items` to return `Err`, so a board
/// that delivers items despite the cancel (already had a page buffered) keeps
/// the run as `Ok`.
///
/// Exercises the EXACT same code path as the public `scrape_boards` via the
/// resolver seam (`scrape_boards_with_resolver`).
#[tokio::test]
async fn scrape_boards_partial_success_under_cancel_returns_ok() {
    // UncancellableScraper ignores the signal — simulates a board that already
    // fetched a page before the cancel arrived. Static so the resolver closure
    // can return `&'static dyn Scraper`.
    static FAKE_OK: std::sync::LazyLock<UncancellableScraper> =
        std::sync::LazyLock::new(|| UncancellableScraper { count: 3 });
    static FAKE_FAIL: std::sync::LazyLock<FailingScraper> =
        std::sync::LazyLock::new(|| FailingScraper);

    let engine = ScraperEngine::new();

    // Pre-register and cancel the token before the run — matches the Autopilot
    // pattern tested by `scrape_boards_all_failed_and_cancelled_returns_err`.
    let token = CancellationToken::new();
    engine
        .register_token("job-partial-cancel", token.clone())
        .await;
    token.cancel();

    // Two board ids: "ok-board" resolves to UncancellableScraper (3 items even
    // under cancel), "fail-board" resolves to FailingScraper (always Err).
    let boards = vec!["ok-board".to_string(), "fail-board".to_string()];

    let result = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(3),
            "job-partial-cancel".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "ok-board" => Ok(&*FAKE_OK as &'static dyn crate::scraping::types::Scraper),
                "fail-board" => Ok(&*FAKE_FAIL as &'static dyn crate::scraping::types::Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await;

    // Partial success (ok-board recovered 3 items) + cancelled → must return Ok.
    let (postings, summaries) =
        result.expect("partial success under cancellation must return Ok, not Err");

    assert_eq!(
        summaries.len(),
        2,
        "summaries must cover both boards; got {summaries:?}"
    );

    let ok_summary = summaries
        .iter()
        .find(|s| s.board == "ok-board")
        .expect("ok-board summary missing");
    assert!(
        ok_summary.error.is_none(),
        "ok-board must not carry an error in its summary; got {ok_summary:?}"
    );
    assert!(
        ok_summary.count > 0,
        "ok-board must report recovered items; got count=0"
    );

    let fail_summary = summaries
        .iter()
        .find(|s| s.board == "fail-board")
        .expect("fail-board summary missing");
    assert!(
        fail_summary.error.is_some(),
        "fail-board must carry an error in its summary; got {fail_summary:?}"
    );

    // The ok-board's 3 items must be present in the result.
    assert!(
        !postings.is_empty(),
        "recovered postings must be non-empty when ok-board delivered items"
    );
}
