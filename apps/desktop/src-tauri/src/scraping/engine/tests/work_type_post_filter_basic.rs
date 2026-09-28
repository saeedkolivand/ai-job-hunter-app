//! Central work-type post-filter (Phase 2b), mirrors the location-filter
//! tests: a non-supporting board's results are filtered to the requested
//! work type, keeping UNDECLARED rows; a supporting board passes through.

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

/// Requesting `[Remote]` against [`WorkTypeFake`] must drop ONLY the declared
/// on-site and hybrid rows; the declared-remote row and the UNDECLARED
/// ("Unknown") row must both survive — pinning the keep-unknowns policy at the
/// engine level (the pure-function truth table lives in `work_type_filter`'s
/// own tests; this proves the engine wiring honors it too).
#[tokio::test]
async fn scrape_boards_central_work_type_filter_drops_only_declared_mismatches_keeps_unknown() {
    static WTFAKE: std::sync::LazyLock<WorkTypeFake> = std::sync::LazyLock::new(|| WorkTypeFake);

    let engine = ScraperEngine::new();
    let mut input = fake_input(10);
    input.work_types = Some(vec![crate::scraping::types::WorkType::Remote]);

    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["wtfake".to_string()],
            input,
            "job-wt-core-filter".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| {
                if id == "wtfake" {
                    Ok(&*WTFAKE as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("a work-type-filtered run is still Ok");

    let s = summaries
        .iter()
        .find(|s| s.board == "wtfake")
        .expect("wtfake summary missing");
    assert_eq!(
        s.count, 2,
        "declared-remote + undeclared (Unknown, never dropped) survive; got {s:?}"
    );
    assert_eq!(
        s.notes,
        vec!["work-type-filtered:2".to_string()],
        "both declared mismatches (on-site, hybrid) must be counted as dropped; got {s:?}"
    );
    let ids: std::collections::HashSet<&str> = postings.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains("wtfake:keep-remote") && ids.contains("wtfake:keep-unknown"));
    assert!(!ids.contains("wtfake:drop-onsite") && !ids.contains("wtfake:drop-hybrid"));
}

/// `Some(vec![])` (the user cleared the filter) must behave EXACTLY like
/// `None` — no filtering at all, not "match nothing". Pins the amendment
/// documented on `BoardSearchInput::work_types`.
#[tokio::test]
async fn scrape_boards_empty_work_types_set_is_a_noop() {
    static WTFAKE2: std::sync::LazyLock<WorkTypeFake> = std::sync::LazyLock::new(|| WorkTypeFake);

    let engine = ScraperEngine::new();
    let mut input = fake_input(10);
    input.work_types = Some(vec![]); // cleared filter, NOT "match nothing"

    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["wtfake".to_string()],
            input,
            "job-wt-empty-set-noop".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| {
                if id == "wtfake" {
                    Ok(&*WTFAKE2 as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("ok");

    assert_eq!(
        postings.len(),
        4,
        "an empty requested set must keep every row, same as no request at all; got {postings:?}"
    );
    let s = summaries
        .iter()
        .find(|s| s.board == "wtfake")
        .expect("wtfake summary missing");
    assert_eq!(s.count, 4, "nothing dropped for an empty requested set");
    assert!(
        s.notes.is_empty(),
        "an empty requested set is inert — it must not even emit a \
         work-type-filtered:0 note (unlike a genuinely requested, non-empty set); got {s:?}"
    );
}

/// A board that DOES declare `supports_work_type() == true` (mirrors
/// smartrecruiters) must NEVER be touched by the central filter, even when its
/// own declared value clearly contradicts the requested set — the engine
/// trusts it was already filtered upstream.
#[tokio::test]
async fn scrape_boards_work_type_supporting_board_never_touched_by_central_filter() {
    struct WorkTypeSupportingFake;
    #[async_trait::async_trait]
    impl Scraper for WorkTypeSupportingFake {
        fn id(&self) -> &'static str {
            "wtsupporting"
        }
        fn display_name(&self) -> &'static str {
            "WorkTypeSupporting"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn supports_work_type(&self) -> bool {
            true
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            let mut extra = std::collections::HashMap::new();
            extra.insert("workType".to_string(), serde_json::json!("on-site"));
            let job = JobPosting {
                id: "wtsupporting:0".to_string(),
                external_id: Some("0".to_string()),
                title: "Job".to_string(),
                company: "WTS".to_string(),
                location: None,
                url: "https://wts.example/0".to_string(),
                source: "wtsupporting".to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra,
            };
            if let Some(ref on_item) = ctx.on_item {
                on_item(job.clone());
            }
            Ok(vec![job])
        }
    }
    static WTSUPPORTING: std::sync::LazyLock<WorkTypeSupportingFake> =
        std::sync::LazyLock::new(|| WorkTypeSupportingFake);

    let engine = ScraperEngine::new();
    let mut input = fake_input(10);
    // Requests Remote while the board's own row declares on-site — a mismatch
    // the central filter would drop for a NON-supporting board.
    input.work_types = Some(vec![crate::scraping::types::WorkType::Remote]);

    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["wtsupporting".to_string()],
            input,
            "job-wt-supporting-untouched".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| {
                if id == "wtsupporting" {
                    Ok(&*WTSUPPORTING as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("ok");

    assert_eq!(
        postings.len(),
        1,
        "a supporting board's mismatching row must survive — the engine never \
         re-filters it; got {postings:?}"
    );
    let s = summaries
        .iter()
        .find(|s| s.board == "wtsupporting")
        .expect("wtsupporting summary missing");
    assert_eq!(s.count, 1);
    assert!(
        s.notes.is_empty(),
        "a supporting board must never carry a work-type-filtered note; got {s:?}"
    );
}
