//! TRUST PR A/B: a failed board surfaces `BoardScrapeSummary.error` without
//! killing sibling boards, and a mid-run pagination failure surfaces
//! `truncated` instead of silently losing the partial harvest.

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

// ── TRUST PR A: honest-failure containment ────────────────────────────────────

/// TRUST PR A — a board that fails all its fetches surfaces its failure without
/// killing the batch, and a genuine empty result stays distinguishable from a
/// failure.
///
/// This pins the sink end of the "representable fetch failures" contract. After
/// PR A a board that hits a non-2xx / schema-drift / all-slug-fail returns `Err`
/// (carrying the HTTP status) instead of a silent `Ok([])`. The engine must:
///   - contain that `Err` into the board's `BoardScrapeSummary.error` (carrying
///     the status), not bubble it up — `scrape_boards` itself stays `Ok`;
///   - keep a sibling board's jobs, with its own `error: None`;
///   - leave a genuinely empty board as `count=0, error=None` (a real zero is
///     NOT a failure — that distinction is the whole point of the PR).
///
/// It complements `scraping/http/test.rs`'s source-end coverage
/// (`test_fetch_json_non_2xx_carries_status` → `Err(Provider("HTTP 403"))`,
/// `test_fetch_json_invalid` → `Err(Parse)`): those prove the fetch layer
/// produces the `Err`; this proves the engine surfaces it honestly. Board
/// `search()` methods hardcode their production hosts (no base-URL injection
/// seam), so their per-board `?`/all-fail propagation can only be exercised
/// through this engine seam without a live network — see the log note.
#[tokio::test]
async fn scrape_boards_failed_board_surfaces_error_and_keeps_siblings() {
    // Mimics a board that propagated a fetch failure — germantechjobs' non-200
    // `Err(anyhow!("HTTP {status}"))`, or an ATS all-slug-fail
    // `Err("all lever company fetches failed: HTTP 403")`. The "403" substring
    // is what must survive `e.to_string()` into the summary so a blocked/rotted
    // board is distinguishable from a genuine zero.
    struct HttpBlockedScraper;

    #[async_trait::async_trait]
    impl Scraper for HttpBlockedScraper {
        fn id(&self) -> &'static str {
            "blocked"
        }
        fn display_name(&self) -> &'static str {
            "Blocked"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            _ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            Err(anyhow::anyhow!("HTTP 403"))
        }
    }

    static BLOCKED: std::sync::LazyLock<HttpBlockedScraper> =
        std::sync::LazyLock::new(|| HttpBlockedScraper);
    // Sibling that succeeds with 3 items — must survive its failing peer.
    static GOOD: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(3));
    // A genuine empty result — Ok([]), which must NOT be reported as an error.
    static EMPTY: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(0));

    let engine = ScraperEngine::new();
    // No cancellation: the failing board must be contained into its summary, not
    // bubble up as a batch-level `Err` (that gate requires a cancelled token).
    let boards = vec![
        "blocked-board".to_string(),
        "good-board".to_string(),
        "empty-board".to_string(),
    ];

    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(10),
            "job-trust-a-containment".to_string(),
            None,
            None,
            // All three fakes are Guest / !requires_company / !needs_keys, so no
            // skip fires regardless of data_dir — `.` matches the partial-cancel test.
            std::path::Path::new("."),
            |id| match id {
                "blocked-board" => Ok(&*BLOCKED as &'static dyn Scraper),
                "good-board" => Ok(&*GOOD as &'static dyn Scraper),
                "empty-board" => Ok(&*EMPTY as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("one failing board must not fail the whole batch — scrape_boards must be Ok");

    assert_eq!(
        summaries.len(),
        3,
        "one summary per board; got {summaries:?}"
    );

    // Failing board: error set + carries the HTTP status, count 0, not skipped.
    let blocked = summaries
        .iter()
        .find(|s| s.board == "blocked-board")
        .expect("blocked-board summary missing");
    let err = blocked
        .error
        .as_deref()
        .expect("a failed board must carry an error, not a silent Ok([]) (count=0, error=None)");
    assert!(
        err.contains("403"),
        "the HTTP status must survive into BoardScrapeSummary.error so a blocked board is \
         distinguishable from a genuine zero; got {err:?}"
    );
    assert_eq!(blocked.count, 0, "failed board reports count=0");
    assert!(
        blocked.skipped.is_none(),
        "a fetch failure is an error, not a skip"
    );

    // Genuine empty board: the trust distinction — a real zero is Ok, not an error.
    let empty = summaries
        .iter()
        .find(|s| s.board == "empty-board")
        .expect("empty-board summary missing");
    assert!(
        empty.error.is_none(),
        "a genuine empty result must NOT be reported as an error; got {empty:?}"
    );
    assert!(empty.skipped.is_none(), "an empty result is not a skip");
    assert_eq!(empty.count, 0, "empty board reports count=0 with no error");

    // Sibling success board: succeeds, no error, its items are kept.
    let good = summaries
        .iter()
        .find(|s| s.board == "good-board")
        .expect("good-board summary missing");
    assert!(
        good.error.is_none(),
        "the succeeding board must not inherit its failing peer's error; got {good:?}"
    );
    assert!(good.skipped.is_none(), "good board must not be skipped");
    assert_eq!(good.count, 3, "good board returns its 3 items");

    // Only the good board's 3 postings are aggregated — the failed and empty
    // boards each contribute none.
    assert_eq!(
        postings.len(),
        3,
        "only the good board's items are aggregated; got {postings:?}"
    );
    assert!(
        postings.iter().all(|p| p.source == "fake"),
        "all recovered postings must come from the good (fake) board"
    );
}

/// TRUST PR B — a paginated board that keeps a partial harvest after a mid-run
/// page failure must surface that truncation on its `BoardScrapeSummary`, so a
/// 2-of-5-pages harvest is distinguishable from a complete one. A board that
/// completed its pages must report `truncated: None`.
///
/// The board reports the reason through `ctx.report_truncation`; the engine tags
/// it with the board name and attributes it to that board's summary. The
/// paginated boards (themuse/arbeitnow/arbeitsagentur) hardcode their production
/// hosts, so — like the PR A containment test above — the report path is
/// exercised through this engine seam rather than a live network.
#[tokio::test]
async fn scrape_boards_partial_pagination_surfaces_truncated_and_keeps_count() {
    // Streams `count` items, then reports a mid-run page failure and returns
    // Ok(partial) — the exact shape of a paginated board when
    // `should_propagate_page_error(collected) == false` breaks the loop.
    struct TruncatingScraper {
        count: usize,
    }

    #[async_trait::async_trait]
    impl Scraper for TruncatingScraper {
        fn id(&self) -> &'static str {
            "truncating"
        }
        fn display_name(&self) -> &'static str {
            "Truncating"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            let mut out = Vec::new();
            for i in 0..self.count {
                let job = JobPosting {
                    id: format!("truncating:{i}"),
                    external_id: Some(i.to_string()),
                    title: format!("Job {i}"),
                    company: "Trunc Co".to_string(),
                    location: None,
                    url: format!("https://trunc.example.com/{i}"),
                    source: "truncating".to_string(),
                    description: None,
                    requirements: None,
                    posted_at: None,
                    captured_at: 0,
                    extra: std::collections::HashMap::new(),
                };
                if let Some(ref on_item) = ctx.on_item {
                    on_item(job.clone());
                }
                out.push(job);
            }
            // A later page failed after items were already collected → keep the
            // partial harvest and report why it stopped short.
            ctx.report_truncation("page 2 of 5 failed: HTTP 429".to_string());
            Ok(out)
        }
    }

    static TRUNC: std::sync::LazyLock<TruncatingScraper> =
        std::sync::LazyLock::new(|| TruncatingScraper { count: 3 });
    // A sibling that completes its pages — must report truncated: None.
    static COMPLETE: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(4));

    let engine = ScraperEngine::new();
    let boards = vec!["trunc-board".to_string(), "complete-board".to_string()];

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(10),
            "job-trust-b-truncation".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "trunc-board" => Ok(&*TRUNC as &'static dyn Scraper),
                "complete-board" => Ok(&*COMPLETE as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("a kept partial harvest is a success, not a batch failure");

    // Truncated board: reason surfaced (carrying the HTTP status), items still
    // counted, and it is neither an error nor a skip.
    let trunc = summaries
        .iter()
        .find(|s| s.board == "trunc-board")
        .expect("trunc-board summary missing");
    let reason = trunc
        .truncated
        .as_deref()
        .expect("a partial-pagination harvest must carry a truncation reason");
    assert!(
        reason.contains("429"),
        "the page-failure reason must survive into BoardScrapeSummary.truncated so a \
         partial harvest is distinguishable from a complete one; got {reason:?}"
    );
    assert_eq!(
        trunc.count, 3,
        "the partial harvest's already-collected items are still counted"
    );
    assert!(
        trunc.error.is_none(),
        "a kept partial harvest is not an error"
    );
    assert!(
        trunc.skipped.is_none(),
        "a kept partial harvest is not a skip"
    );

    // Complete board: a run that finished its pages must NOT be marked truncated,
    // otherwise a complete run is indistinguishable from a partial one.
    let complete = summaries
        .iter()
        .find(|s| s.board == "complete-board")
        .expect("complete-board summary missing");
    assert!(
        complete.truncated.is_none(),
        "a board that completed its pages must not be marked truncated; got {complete:?}"
    );
    assert_eq!(complete.count, 4, "complete board returns all its items");
}
