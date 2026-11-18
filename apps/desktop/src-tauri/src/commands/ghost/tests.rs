use super::*;
use crate::error::AppError;
use crate::posting_history::signals_for;

#[test]
fn signals_pass_through_when_the_write_succeeded() {
    let mut ok = HashMap::new();
    ok.insert("acme|rust".to_string(), signals_for(0, 0, 0));
    assert_eq!(best_effort(Some(Ok(ok.clone()))), ok);
}

#[test]
fn a_write_error_degrades_to_no_signals_instead_of_failing_the_run() {
    let err = AppError::Storage("disk full".into());
    assert!(best_effort(Some(Err(err))).is_empty());
}

#[test]
fn an_unmanaged_store_yields_no_signals() {
    assert!(best_effort(None).is_empty());
}

fn summary(
    board: &str,
    error: Option<&str>,
    skipped: Option<&str>,
    truncated: Option<&str>,
) -> BoardScrapeSummary {
    BoardScrapeSummary {
        board: board.into(),
        count: 0,
        error: error.map(Into::into),
        skipped: skipped.map(Into::into),
        truncated: truncated.map(Into::into),
        notes: Vec::new(),
        health: None,
    }
}

#[test]
fn only_boards_that_ran_to_completion_advance_their_marker() {
    let all = [
        summary("ok", None, None, None),
        summary("down", Some("HTTP 503"), None, None),
        summary("login", None, Some("needs-login"), None),
        summary("partial", None, None, Some("page 3 of 5 failed")),
        // An empty successful board is evidence of absence, so it stays ok.
        summary("empty", None, None, None),
    ];
    assert_eq!(ok_boards(&all).collect::<Vec<_>>(), ["ok", "empty"]);
}

#[test]
fn search_keys_separate_different_searches() {
    let mut target = crate::autopilot::AutopilotTarget {
        boards: vec!["greenhouse".into()],
        query: "rust".into(),
        location: Some("Berlin".into()),
        country_code: None,
        work_types: None,
        pages: 2,
        date_filter: None,
        top_n: 10,
        watched_companies_only: None,
    };
    let base = target_key(&target);
    assert_eq!(base, target_key(&target.clone()));
    target.query = "rust remote".into();
    assert_ne!(base, target_key(&target));
}

/// The recording call must sit AFTER the run's cancel check: a Stopped run saw
/// a partial result, and a partial result must never count as absence. Both
/// paths need an `AppHandle`, so the ordering is pinned on the source.
#[test]
fn a_stopped_autopilot_run_is_never_recorded() {
    let src = include_str!("../autopilot/run.rs");
    let cancel = src.find("cancelled before recording results").unwrap();
    let record = src.find("ghost::record_scrape(").unwrap();
    assert!(
        cancel < record,
        "record_scrape must come after the cancel check"
    );
    assert_eq!(src.matches("ghost::record_scrape(").count(), 1);
}

#[test]
fn a_stopped_manual_scrape_is_never_recorded() {
    let src = include_str!("../scrape/run.rs");
    let guard = src.find("if !cancel_token.is_cancelled()").unwrap();
    let record = src.find("ghost::record_scrape(").unwrap();
    assert!(
        guard < record,
        "record_scrape must be guarded by the cancel check"
    );
    assert_eq!(src.matches("ghost::record_scrape(").count(), 1);
}
