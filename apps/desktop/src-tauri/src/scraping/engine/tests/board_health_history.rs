//! Track B1: `scrape_boards` folds each run into the per-board reliability
//! history and attaches a noteworthy health to the right summary — never
//! for an unresolvable board id, and never when no store is wired.

use super::super::*;
use super::support::*;

// ── Track B1: per-board reliability history ─────────────────────────────────

/// Build an engine with a real (temp-dir) board-health store attached. The
/// `TempDir` is returned so the DB outlives the engine.
fn engine_with_health() -> (
    tempfile::TempDir,
    ScraperEngine,
    Arc<crate::scraping::BoardHealthStore>,
) {
    let dir = tempfile::TempDir::new().unwrap();
    let store = Arc::new(crate::scraping::BoardHealthStore::open(dir.path()).unwrap());
    let engine = ScraperEngine::new();
    engine.set_health_store(store.clone());
    (dir, engine, store)
}

/// Two runs where one board fails both times and another succeeds both times:
/// the failing board's summary must carry a GROWING failure streak, while the
/// healthy board's summary carries no health at all (nothing to badge).
///
/// This is the user-visible point of Track B1 — after this, "0 results" from a
/// working board and "0 results" from a week-long outage no longer look alike.
#[tokio::test]
async fn scrape_boards_attaches_a_growing_failure_streak_to_the_failing_board() {
    static FAKE_OK: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(2));
    static FAKE_FAIL: std::sync::LazyLock<FailingScraper> =
        std::sync::LazyLock::new(|| FailingScraper);

    let (_dir, engine, store) = engine_with_health();
    let boards = vec!["ok-board".to_string(), "fail-board".to_string()];
    let resolve = |id: &str| match id {
        "ok-board" => Ok(&*FAKE_OK as &'static dyn crate::scraping::types::Scraper),
        "fail-board" => Ok(&*FAKE_FAIL as &'static dyn crate::scraping::types::Scraper),
        other => Err(anyhow::anyhow!("Unknown board: {other}")),
    };

    let (_, summaries) = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(5),
            "job-health-1".to_string(),
            None,
            None,
            std::path::Path::new("."),
            resolve,
        )
        .await
        .expect("one failing board must not fail the run");

    assert!(
        summaries[0].health.is_none(),
        "a healthy board must carry no badge; got {:?}",
        summaries[0].health
    );
    let first = summaries[1]
        .health
        .clone()
        .expect("a failing board must carry its history");
    assert_eq!(first.consecutive_failures, 1);
    assert_eq!(first.status, crate::scraping::BoardHealthStatus::Failing);
    assert_eq!(first.last_success_at, None);
    assert_eq!(first.last_run_id.as_deref(), Some("job-health-1"));

    let (_, summaries) = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(5),
            "job-health-2".to_string(),
            None,
            None,
            std::path::Path::new("."),
            resolve,
        )
        .await
        .expect("second run");

    let second = summaries[1]
        .health
        .clone()
        .expect("the streak must still be reported on the second run");
    assert_eq!(second.consecutive_failures, 2, "the streak must GROW");
    // Anchor the FIRST run's window before comparing the two — `None == None`
    // would otherwise satisfy "keeps its original start" vacuously.
    assert!(
        first.failing_since.is_some(),
        "the first failure must open a streak window"
    );
    assert_eq!(
        second.failing_since, first.failing_since,
        "the streak keeps its original start"
    );
    assert_eq!(second.last_run_id.as_deref(), Some("job-health-2"));
    // The store agrees with what rode the summary.
    assert_eq!(
        store.health_for("fail-board").unwrap().consecutive_failures,
        2
    );
    assert!(
        store
            .health_for("ok-board")
            .unwrap()
            .last_success_at
            .is_some(),
        "the successful board's success must still be recorded, badge or not"
    );
}

/// A user cancel makes every in-flight board report an error. Recording those
/// would manufacture a failure streak out of the user's own click, so a
/// cancelled run must leave the history completely untouched.
#[tokio::test]
async fn a_cancelled_run_records_no_board_history() {
    static FAKE_OK: std::sync::LazyLock<UncancellableScraper> =
        std::sync::LazyLock::new(|| UncancellableScraper { count: 3 });
    static FAKE_FAIL: std::sync::LazyLock<FailingScraper> =
        std::sync::LazyLock::new(|| FailingScraper);

    let (_dir, engine, store) = engine_with_health();
    let token = CancellationToken::new();
    engine.register_token("job-cancelled", token.clone()).await;
    token.cancel();

    let (_, summaries) = engine
        .scrape_boards_with_resolver(
            &["ok-board".to_string(), "fail-board".to_string()],
            fake_input(3),
            "job-cancelled".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "ok-board" => Ok(&*FAKE_OK as &'static dyn crate::scraping::types::Scraper),
                "fail-board" => Ok(&*FAKE_FAIL as &'static dyn crate::scraping::types::Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("partial success under cancel returns Ok");

    assert!(
        summaries.iter().all(|s| s.health.is_none()),
        "a cancelled run must not badge anything; got {summaries:?}"
    );
    assert!(
        store.health_for("fail-board").is_none(),
        "a user cancel must not be recorded as a board failure"
    );
    assert!(
        store.health_for("ok-board").is_none(),
        "a cancelled run records nothing at all, not even the successes"
    );
}

/// A board that is SKIPPED (never contacted) must not be recorded as failing —
/// the `skipped` vs `error` distinction, checked end-to-end through the engine
/// rather than only on the pure fold.
#[tokio::test]
async fn a_skipped_board_gets_no_failure_history_from_the_engine() {
    // A company-scoped ATS board with no curated seed and no user slugs is
    // skipped `needs-company` before any fetch, so it is never contacted.
    static ATS: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::ats("no-seed-ats"));

    let (_dir, engine, store) = engine_with_health();
    let mut input = fake_input(3);
    input.companies = Vec::new();

    let (_, summaries) = engine
        .scrape_boards_with_resolver(
            &["skipme".to_string()],
            input,
            "job-skip".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |_| Ok(&*ATS as &'static dyn crate::scraping::types::Scraper),
        )
        .await
        .expect("a fully-skipped run still returns Ok");

    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].skipped.as_deref(),
        Some("needs-company"),
        "board must be skipped, not run"
    );
    assert!(
        summaries[0].health.is_none(),
        "a skipped board is not unhealthy; got {:?}",
        summaries[0].health
    );
    let stored = store
        .health_for("skipme")
        .expect("the skip is still recorded as 'seen, not verified'");
    assert_eq!(stored.consecutive_failures, 0, "a skip is not a failure");
    assert_eq!(stored.last_verified_at, None, "a skip verifies nothing");
    assert_eq!(stored.verified_runs, 0, "a skip is not a verified run");
    assert_eq!(stored.failed_runs, 0);
    // `last_run_id` names the run that PRODUCED the state. This run never
    // fetched the board, so it must not claim authorship — grepping the logs for
    // `job-skip` would find no `skipme` fetch at all.
    assert_eq!(
        stored.last_run_id, None,
        "a run that never contacted the board must not be stamped as its source"
    );
}

/// A board id the resolver does not recognise must NEVER create a row.
///
/// The engine deliberately lets an unknown id through to an ordinary error
/// summary (rather than a skip) so a typo doesn't silently vanish — but `board`
/// is the health table's PRIMARY KEY and that string comes straight from the
/// renderer (`commands::scrape` clones `req.boards` into the engine, and the
/// generated `ScrapeBoardsRequest.boards` is an unvalidated `Vec<String>`).
/// Without a filter, a looping or XSS'd renderer writes unbounded rows into a
/// new on-disk store — the same threat the scrape limiter already exists to stop.
///
/// The row COUNT is what is asserted: "the id I thought of is absent" cannot
/// see a row created under some other arbitrary key.
#[tokio::test]
async fn an_unresolvable_board_id_never_creates_a_health_row() {
    static FAKE_OK: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(2));

    let (_dir, engine, store) = engine_with_health();

    let (_, summaries) = engine
        .scrape_boards_with_resolver(
            &[
                "ok-board".to_string(),
                "attacker-supplied-\u{1f480}".to_string(),
                "another-bogus-id".to_string(),
            ],
            fake_input(5),
            "job-unknown".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "ok-board" => Ok(&*FAKE_OK as &'static dyn crate::scraping::types::Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("unknown ids must not fail the run");

    // The unknown ids still surface to the user as ordinary error chips — the
    // engine's existing behaviour is unchanged.
    assert_eq!(summaries.len(), 3);
    assert!(
        summaries[1].error.is_some() && summaries[2].error.is_some(),
        "unknown ids must still produce a visible error summary; got {summaries:?}"
    );
    assert!(
        summaries.iter().all(|s| s.health.is_none()),
        "an unresolvable board has no history to badge"
    );

    // …but only the ONE resolvable board may own a row.
    assert_eq!(
        store.tracked_boards(),
        1,
        "only resolver-known boards may create rows; renderer strings must not"
    );
    assert!(store.health_for("ok-board").is_some());
}

/// `record_health` re-attaches `health` by ZIPPING `recorded` (the positions
/// of the resolvable boards within `summaries`) against the store's returned
/// results — NOT by the results' own position. A mutant that swaps the zip for
/// `health.into_iter().enumerate()` reads `health[i]` as belonging to
/// `summaries[i]`, which is only wrong when an unresolvable id sits AHEAD of a
/// resolvable one in `summaries` — the earlier
/// `an_unresolvable_board_id_never_creates_a_health_row` test puts its only
/// valid board at index 0, so both forms agree and the mutant survives it.
///
/// Discriminating shape: a rejected id at index 0, then TWO resolvable boards
/// that both FAIL (two noteworthy verdicts, so a misattribution is visible on
/// either summary, not silently dropped like a healthy board would be).
#[tokio::test]
async fn a_rejected_board_ahead_of_two_failing_ones_never_wears_their_history() {
    static FAIL_A: std::sync::LazyLock<FailingScraper> =
        std::sync::LazyLock::new(|| FailingScraper);
    static FAIL_B: std::sync::LazyLock<FailingScraper> =
        std::sync::LazyLock::new(|| FailingScraper);

    let (_dir, engine, store) = engine_with_health();

    let (_, summaries) = engine
        .scrape_boards_with_resolver(
            &[
                "bogus-id".to_string(),
                "fail-a".to_string(),
                "fail-b".to_string(),
            ],
            fake_input(5),
            "job-mixed".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "fail-a" => Ok(&*FAIL_A as &'static dyn crate::scraping::types::Scraper),
                "fail-b" => Ok(&*FAIL_B as &'static dyn crate::scraping::types::Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("failures must not fail the run");

    assert_eq!(summaries.len(), 3);
    assert!(
        summaries[0].health.is_none(),
        "the unresolvable id has no history row to wear a verdict from; got {:?}",
        summaries[0].health
    );
    let a = summaries[1]
        .health
        .clone()
        .expect("fail-a must carry its own history");
    let b = summaries[2]
        .health
        .clone()
        .expect("fail-b must carry its own history");
    assert_eq!(a.consecutive_failures, 1);
    assert_eq!(b.consecutive_failures, 1);
    assert_eq!(
        store.tracked_boards(),
        2,
        "only the two resolvable boards may own rows"
    );
}
