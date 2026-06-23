use crate::autopilot::AutopilotTarget;
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, SCRAPE_ITEM, SCRAPE_PROGRESS};
use crate::scraping::{BoardScrapeSummary, BoardSearchInput, JobPosting, ScraperEngine};
use tauri::AppHandle;

/// Scrape job postings from one or more boards for an autopilot run. Returns the
/// postings **and** the per-board diagnostics, so the run can explain a zero
/// result (e.g. an aggregator error or a skipped board) instead of silently
/// showing "found 0".
pub async fn autopilot_scrape(
    engine: &ScraperEngine,
    target: &AutopilotTarget,
    job_id: &str,
    app: &AppHandle,
) -> AppResult<(Vec<JobPosting>, Vec<BoardScrapeSummary>)> {
    let input = BoardSearchInput {
        query: target.query.clone(),
        location: target.location.clone(),
        // Autopilot expresses its target in pages, so let the page budget bind and
        // set the central item cap to the maximum (never caps autopilot to 0).
        amount: 100,
        pages: target.pages,
        date_filter: target.date_filter.clone(),
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: target.country_code.clone(),
        latitude: None,
        longitude: None,
        radius_km: None,
        // Autopilot has no per-company target; ATS company slugs are a manual
        // search affordance, so this stays empty (a no-op for every board).
        companies: Vec::new(),
    };

    let app_progress = app.clone();
    let job_id_progress = job_id.to_string();
    let on_progress: std::sync::Arc<dyn Fn(f32) + Send + Sync> =
        std::sync::Arc::new(move |p: f32| {
            emit_event(
                &app_progress,
                SCRAPE_PROGRESS,
                serde_json::json!({ "jobId": job_id_progress, "progress": p }),
            );
        });

    let app_item = app.clone();
    let job_id_item = job_id.to_string();
    let on_item: std::sync::Arc<dyn Fn(JobPosting) + Send + Sync> =
        std::sync::Arc::new(move |item: JobPosting| {
            emit_event(
                &app_item,
                SCRAPE_ITEM,
                serde_json::json!({ "jobId": job_id_item, "item": item }),
            );
        });

    let result = engine
        .scrape_boards(
            &target.boards,
            input,
            job_id.to_string(),
            Some(on_progress),
            Some(on_item),
        )
        .await;

    // Log any skipped or errored boards so operators can diagnose unexpected empty
    // runs, AND return the summaries so the run can surface *why* it found zero.
    result
        .map(|(postings, summaries)| {
            for s in &summaries {
                if let Some(ref reason) = s.skipped {
                    log::warn!(
                        "[autopilot] board '{}' skipped (reason='{}')",
                        s.board,
                        reason
                    );
                }
                if let Some(ref err) = s.error {
                    log::warn!("[autopilot] board '{}' failed (error='{}')", s.board, err);
                }
            }
            (postings, summaries)
        })
        .map_err(AppError::from)
}

/// Turn the per-board scrape summaries into a single human-readable reason string
/// explaining why a run may have come up short — `"<board>: <error>"` for each
/// board that errored or was skipped, joined with `"; "`. Returns an empty string
/// when no board reported a problem. Pure + unit-testable.
pub(crate) fn scrape_diagnostics(summaries: &[BoardScrapeSummary]) -> String {
    summaries
        .iter()
        .filter_map(|s| {
            s.error
                .as_deref()
                .or(s.skipped.as_deref())
                .map(|reason| format!("{}: {reason}", s.board))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scraping::BoardScrapeSummary;

    fn summary(board: &str, error: Option<&str>, skipped: Option<&str>) -> BoardScrapeSummary {
        BoardScrapeSummary {
            board: board.into(),
            count: 0,
            error: error.map(String::from),
            skipped: skipped.map(String::from),
        }
    }

    #[test]
    fn empty_slice_returns_empty_string() {
        assert_eq!(scrape_diagnostics(&[]), "");
    }

    #[test]
    fn board_with_error_and_no_skip_shows_error() {
        let s = summary("linkedin", Some("429 Too Many Requests"), None);
        let diag = scrape_diagnostics(&[s]);
        assert!(
            diag.contains("linkedin"),
            "board name must appear; got: {diag}"
        );
        assert!(
            diag.contains("429 Too Many Requests"),
            "error text must appear; got: {diag}"
        );
    }

    #[test]
    fn exact_format_is_board_colon_space_reason() {
        // Pin the exact `"<board>: <reason>"` format the impl produces.
        // If the separator or spacing ever changes this test catches it immediately.
        assert_eq!(
            scrape_diagnostics(&[summary("aggregator", Some("network timeout"), None)]),
            "aggregator: network timeout"
        );
    }

    #[test]
    fn skipped_only_board_appears_in_output() {
        // A board that was skipped (no error) must still surface its reason.
        let diag = scrape_diagnostics(&[summary("glassdoor", None, Some("needs-login"))]);
        assert!(
            diag.contains("needs-login"),
            "skipped reason must appear; got: {diag}"
        );
    }

    #[test]
    fn error_takes_precedence_over_skipped_when_both_are_set() {
        // `error` is checked first in the `or` chain; `skipped` must be shadowed.
        let s = summary("aggregator", Some("network timeout"), Some("needs-login"));
        let diag = scrape_diagnostics(&[s]);
        assert!(
            diag.contains("network timeout"),
            "error must win over skipped; got: {diag}"
        );
        assert!(
            !diag.contains("needs-login"),
            "skipped must be suppressed when error is set; got: {diag}"
        );
    }

    #[test]
    fn board_with_neither_error_nor_skipped_contributes_nothing() {
        let clean = summary("indeed", None, None);
        assert_eq!(scrape_diagnostics(&[clean]), "");
    }

    #[test]
    fn multiple_errored_boards_are_joined_with_semicolon() {
        let summaries = vec![
            summary("linkedin", Some("rate-limited"), None),
            summary("indeed", None, Some("needs-login")),
        ];
        let diag = scrape_diagnostics(&summaries);
        // Both boards must appear; they are joined with "; ".
        assert!(
            diag.contains("linkedin"),
            "first board missing; got: {diag}"
        );
        assert!(diag.contains("indeed"), "second board missing; got: {diag}");
        assert!(
            diag.contains("; "),
            "boards must be joined with \"; \"; got: {diag}"
        );
    }

    #[test]
    fn clean_board_mixed_with_errored_does_not_appear_in_output() {
        let summaries = vec![
            summary("linkedin", Some("timeout"), None),
            summary("remotive", None, None), // clean — must not appear
        ];
        let diag = scrape_diagnostics(&summaries);
        assert!(
            !diag.contains("remotive"),
            "clean board must not appear; got: {diag}"
        );
        assert!(
            !diag.contains("; "),
            "single problem board must not add trailing separator; got: {diag}"
        );
    }
}
