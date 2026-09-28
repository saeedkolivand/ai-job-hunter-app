//! Required-auth board skip short-circuit: no cookies / stale session /
//! fresh session, each verified to short-circuit (or not) before any
//! fetch — and skip health-store attribution.

use super::panickers::{NeedsKeysPanicker, RequiredPanicker};
use super::support::*;
use crate::scraping::types::Scraper;

use super::super::*;

// ── Required-board skip short-circuit ────────────────────────────────────────

/// A `Required` fake scraper with no cookies must be skipped without calling
/// `search` (the summary has `skipped=Some("needs-login")`, `count=0`, no
/// error), while a `Guest` fake runs normally. The RequiredPanicker's `search`
/// panics, so if the short-circuit is absent the test fails immediately.
#[tokio::test]
async fn required_board_without_session_is_skipped() {
    static REQ: std::sync::LazyLock<RequiredPanicker> =
        std::sync::LazyLock::new(|| RequiredPanicker::new("required-panicker"));
    // Guest board: 1 item so we can assert it WAS actually run (count=1), not silently skipped.
    static FAKE_GUEST: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(1));

    let engine = ScraperEngine::new();
    // Use an isolated tempdir — no cookies.json exists there for "required-board",
    // so load_cookies returns [] and the skip fires. If the skip is absent the
    // panicker's search() runs and the test panics.
    let tmp = tempfile::tempdir().expect("tempdir");
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["required-board".to_string(), "guest-board".to_string()],
            fake_input(5),
            "job-skip-test".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "required-board" => Ok(&*REQ as &'static dyn Scraper),
                "guest-board" => Ok(&*FAKE_GUEST as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unknown: {other}")),
            },
        )
        .await
        .expect("skip run must return Ok");

    let req_summary = summaries
        .iter()
        .find(|s| s.board == "required-board")
        .expect("required-board summary missing");
    assert_eq!(
        req_summary.skipped.as_deref(),
        Some("needs-login"),
        "Required board with no session must be skipped with 'needs-login'"
    );
    assert_eq!(req_summary.count, 0, "skipped board must report count=0");
    assert!(
        req_summary.error.is_none(),
        "skipped board must not carry an error"
    );

    let guest_summary = summaries
        .iter()
        .find(|s| s.board == "guest-board")
        .expect("guest-board summary missing");
    assert!(
        guest_summary.skipped.is_none(),
        "Guest board must not be skipped"
    );
    assert!(
        guest_summary.error.is_none(),
        "Guest board must run without error"
    );
    assert_eq!(
        guest_summary.count, 1,
        "Guest board (FakeScraper::http(1)) must have been run and report count=1"
    );
}

/// A board that declares `needs_keys() == true` (a key-backed board with no API
/// keys configured) must be skipped with `skipped=Some("needs-keys")`, count=0,
/// no error — its `search` must never run. A sibling guest board runs normally.
#[tokio::test]
async fn needs_keys_board_without_keys_is_skipped() {
    static NK: std::sync::LazyLock<NeedsKeysPanicker> =
        std::sync::LazyLock::new(|| NeedsKeysPanicker::new("needs-keys-panicker"));
    static FAKE_GUEST: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(1));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["nk-board".to_string(), "guest-board".to_string()],
            fake_input(5),
            "job-needs-keys".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "nk-board" => Ok(&*NK as &'static dyn Scraper),
                "guest-board" => Ok(&*FAKE_GUEST as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unknown: {other}")),
            },
        )
        .await
        .expect("skip run must return Ok");

    let nk = summaries
        .iter()
        .find(|s| s.board == "nk-board")
        .expect("nk-board summary missing");
    assert_eq!(
        nk.skipped.as_deref(),
        Some("needs-keys"),
        "an unconfigured key-backed board must be skipped with 'needs-keys'"
    );
    assert_eq!(nk.count, 0, "skipped board must report count=0");
    assert!(nk.error.is_none(), "skipped board must not carry an error");

    let guest = summaries
        .iter()
        .find(|s| s.board == "guest-board")
        .expect("guest-board summary missing");
    assert!(guest.skipped.is_none(), "guest board must not be skipped");
    assert_eq!(guest.count, 1, "guest board must run and report count=1");
}

/// Input order is preserved across a mixed run/skip/run scenario:
/// [required-no-session (skip), guest (run), required-no-session-2 (skip)]
/// Summaries must come back in that same order, not skips-last.
#[tokio::test]
async fn scrape_boards_summaries_preserve_input_order_with_skips() {
    static REQ1: std::sync::LazyLock<RequiredPanicker> =
        std::sync::LazyLock::new(|| RequiredPanicker::new("always-required"));
    static REQ2: std::sync::LazyLock<RequiredPanicker> =
        std::sync::LazyLock::new(|| RequiredPanicker::new("always-required"));
    static GUEST: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(2));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &[
                "req-1".to_string(),
                "guest-mid".to_string(),
                "req-2".to_string(),
            ],
            fake_input(10),
            "job-order-test".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "req-1" => Ok(&*REQ1 as &'static dyn Scraper),
                "guest-mid" => Ok(&*GUEST as &'static dyn Scraper),
                "req-2" => Ok(&*REQ2 as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unknown: {other}")),
            },
        )
        .await
        .expect("mixed run must return Ok");

    assert_eq!(summaries.len(), 3, "one summary per requested board");
    let order: Vec<&str> = summaries.iter().map(|s| s.board.as_str()).collect();
    assert_eq!(
        order,
        vec!["req-1", "guest-mid", "req-2"],
        "summaries must be in input order, not run-results-then-skips; got {order:?}"
    );
    assert_eq!(
        summaries[0].skipped.as_deref(),
        Some("needs-login"),
        "req-1 must be skipped"
    );
    assert_eq!(summaries[1].skipped, None, "guest-mid must not be skipped");
    assert_eq!(summaries[1].count, 2, "guest-mid must report 2 items run");
    assert_eq!(
        summaries[2].skipped.as_deref(),
        Some("needs-login"),
        "req-2 must be skipped"
    );
}

/// A Required board whose skip predicate fires on a stale session must be
/// skipped with `skipped=Some("needs-login")` — same outcome as no-cookies.
///
/// Uses a tempdir so this test never touches the real `data_dir()` and is
/// safe to run concurrently with any other test.
#[tokio::test]
async fn required_board_stale_session_is_skipped() {
    use crate::scraping::board_login::{auth_status_path, write_cookies, StoredCookie};

    let board_id = "stale-engine-test-board";

    static STALE_SCRAPER: std::sync::LazyLock<RequiredPanicker> =
        std::sync::LazyLock::new(|| RequiredPanicker::new("req-stale"));

    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path();

    // Write a fresh cookie (non-empty → skip fires on staleness, not absence).
    let cookie = StoredCookie {
        name: "sess".into(),
        value: "tok".into(),
        domain: "example.com".into(),
        path: "/".into(),
        expires: None,
        http_only: false,
        secure: false,
    };
    write_cookies(data_dir, board_id, &[cookie]).expect("write_cookies");
    // connected_at = 0 → age ≈ now-ms → always > SESSION_MAX_AGE_MS (7 days).
    let apath = auth_status_path(data_dir, board_id);
    std::fs::write(&apath, r#"{"connected":true,"connected_at":0}"#)
        .expect("overwrite auth-status with epoch-0");

    let engine = ScraperEngine::new();
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &[board_id.to_string()],
            fake_input(1),
            "job-stale-skip".to_string(),
            None,
            None,
            data_dir,
            |_id| Ok(&*STALE_SCRAPER as &'static dyn Scraper),
        )
        .await
        .expect("stale-session skip must return Ok");

    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].skipped.as_deref(),
        Some("needs-login"),
        "stale-session Required board must be skipped with 'needs-login'"
    );
    assert_eq!(summaries[0].count, 0);
    assert!(summaries[0].error.is_none());
}
