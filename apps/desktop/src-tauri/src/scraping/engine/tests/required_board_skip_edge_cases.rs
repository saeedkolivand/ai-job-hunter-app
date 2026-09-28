//! Required-auth board skip short-circuit, continued: valid-session run,
//! per-board skip independence, and the resolver-vs-`boards::get` id
//! source for the skip check.

use super::panickers::RequiredPanicker;
use super::support::*;
use crate::scraping::types::Scraper;

use super::super::*;

/// A Required board with fresh cookies + fresh auth-status must NOT be skipped —
/// search is called and the summary appears in run results with its items.
///
/// Uses a tempdir so this test never touches the real `data_dir()`.
#[tokio::test]
async fn required_board_with_valid_session_runs() {
    use crate::scraping::board_login::{write_auth_status, write_cookies, StoredCookie};

    let board_id = "fresh-engine-test-board";

    static FRESH_SCRAPER: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(3));

    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path();

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
    // connected_at = now → age ≈ 0 → not stale.
    write_auth_status(data_dir, board_id, true);

    let engine = ScraperEngine::new();
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &[board_id.to_string()],
            fake_input(5),
            "job-fresh-session".to_string(),
            None,
            None,
            data_dir,
            |_id| Ok(&*FRESH_SCRAPER as &'static dyn Scraper),
        )
        .await
        .expect("fresh-session Required board must return Ok");

    assert_eq!(summaries.len(), 1);
    assert!(
        summaries[0].skipped.is_none(),
        "Required board with fresh session must NOT be skipped; got {:?}",
        summaries[0].skipped
    );
    assert!(summaries[0].error.is_none(), "must not error");
    assert_eq!(
        summaries[0].count, 3,
        "FakeScraper::http(3) must return 3 items"
    );
}

/// A Required board with non-empty cookies but no valid connected status (e.g.
/// `{"connected":false,"connected_at":0}`) must be skipped — the fix to also
/// check `session_age_ms(…).is_none()` covers this case. The cookie-empty branch
/// would NOT fire here, so this test specifically exercises the new branch.
#[tokio::test]
async fn required_board_cookies_but_no_valid_status_is_skipped() {
    use crate::scraping::board_login::{auth_status_path, write_cookies, StoredCookie};

    let board_id = "no-status-engine-test-board";

    static NO_STATUS_SCRAPER: std::sync::LazyLock<RequiredPanicker> =
        std::sync::LazyLock::new(|| RequiredPanicker::new("req-no-status"));

    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path();

    // Write non-empty cookies so the empty-cookie branch does NOT fire.
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

    // Write an auth-status with connected:false → session_age_ms returns None.
    // connected:false → session_age_ms() == None (clause 2 fires) BEFORE connected_at is read;
    // near-now ts means session_is_stale() is false, so clause 3 cannot mask the fix.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let apath = auth_status_path(data_dir, board_id);
    std::fs::write(
        &apath,
        format!(r#"{{"connected":false,"connected_at":{now_ms}}}"#),
    )
    .expect("write connected:false auth-status");

    let engine = ScraperEngine::new();
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &[board_id.to_string()],
            fake_input(1),
            "job-no-status-skip".to_string(),
            None,
            None,
            data_dir,
            |_id| Ok(&*NO_STATUS_SCRAPER as &'static dyn Scraper),
        )
        .await
        .expect("no-valid-status skip must return Ok");

    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].skipped.as_deref(),
        Some("needs-login"),
        "Required board with non-empty cookies but no valid connected status must be skipped with 'needs-login'"
    );
    assert_eq!(summaries[0].count, 0);
    assert!(summaries[0].error.is_none());
}

/// Every board Required + no session → returns Ok with empty postings and
/// all-skipped summaries (NOT Err). No network calls, no panics.
#[tokio::test]
async fn all_required_no_session_returns_ok_empty() {
    static A: std::sync::LazyLock<RequiredPanicker> =
        std::sync::LazyLock::new(|| RequiredPanicker::new("req-noop"));
    static B: std::sync::LazyLock<RequiredPanicker> =
        std::sync::LazyLock::new(|| RequiredPanicker::new("req-noop"));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = engine
        .scrape_boards_with_resolver(
            &["req-a".to_string(), "req-b".to_string()],
            fake_input(5),
            "job-all-req-no-session".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "req-a" => Ok(&*A as &'static dyn Scraper),
                "req-b" => Ok(&*B as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unknown: {other}")),
            },
        )
        .await;

    let (postings, summaries) =
        result.expect("all-Required-no-session must return Ok (not Err) — skips are not failures");
    assert!(postings.is_empty(), "no postings when all boards skipped");
    assert_eq!(summaries.len(), 2, "one summary per board");
    for s in &summaries {
        assert_eq!(
            s.skipped.as_deref(),
            Some("needs-login"),
            "board '{}' must be skipped with 'needs-login'",
            s.board
        );
        assert_eq!(s.count, 0, "skipped board '{}' must have count=0", s.board);
        assert!(
            s.error.is_none(),
            "skipped board '{}' must not carry an error",
            s.board
        );
    }
}

/// An unknown board id (resolver returns Err) must produce an error summary,
/// not a skip summary — it must not be silently treated as a skipped board.
#[tokio::test]
async fn unknown_board_errors_not_skipped() {
    let engine = ScraperEngine::new();
    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["totally-unknown-board".to_string()],
            fake_input(1),
            "job-unknown-test".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| Err(anyhow::anyhow!("Unknown board: {id}")),
        )
        .await
        .expect("unknown board returns Ok (not Err) because parent is not cancelled");

    assert_eq!(summaries.len(), 1);
    let s = &summaries[0];
    assert_eq!(s.board, "totally-unknown-board");
    assert!(
        s.skipped.is_none(),
        "unknown board must not appear as skipped; got skipped={:?}",
        s.skipped
    );
    assert!(
        s.error.is_some(),
        "unknown board must carry an error summary"
    );
    assert_eq!(s.count, 0, "unknown board must have count=0");
}
