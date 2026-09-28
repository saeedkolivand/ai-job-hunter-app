//! Work-type post-filter, continued: both filters active on one board
//! produce independent `notes` entries (location AND work-type), never
//! one clobbering the other.

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

/// Both filters active on ONE non-supporting board must produce BOTH notes,
/// location first, then work type — never one silently clobbering the other.
///
/// Mutation check (do not delete this comment): if `BoardScrapeSummary.notes`
/// were reverted to a single `Option<String>` slot, `location-filtered` would
/// win (first write wins under `get_or_insert_with`) and `work-type-filtered`
/// would be silently dropped — this assertion on the FULL vector fails the
/// instant that happens, which is the point of this test.
#[tokio::test]
async fn scrape_boards_location_and_work_type_notes_coexist_on_one_board() {
    struct BothMismatchFake;
    #[async_trait::async_trait]
    impl Scraper for BothMismatchFake {
        fn id(&self) -> &'static str {
            "bothmismatch"
        }
        fn display_name(&self) -> &'static str {
            "BothMismatch"
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
            // Row 1: matches location, mismatches work type.
            {
                let mut extra = std::collections::HashMap::new();
                extra.insert("workType".to_string(), serde_json::json!("on-site"));
                let job = JobPosting {
                    id: "bothmismatch:wt".to_string(),
                    external_id: Some("wt".to_string()),
                    title: "Job".to_string(),
                    company: "BM".to_string(),
                    location: Some("Berlin, Germany".to_string()),
                    url: "https://bm.example/wt".to_string(),
                    source: "bothmismatch".to_string(),
                    description: None,
                    requirements: None,
                    posted_at: None,
                    captured_at: 0,
                    extra,
                };
                if let Some(ref on_item) = ctx.on_item {
                    on_item(job.clone());
                }
                out.push(job);
            }
            // Row 2: matches work type, mismatches location.
            {
                let mut extra = std::collections::HashMap::new();
                extra.insert("workType".to_string(), serde_json::json!("remote"));
                let job = JobPosting {
                    id: "bothmismatch:loc".to_string(),
                    external_id: Some("loc".to_string()),
                    title: "Job".to_string(),
                    company: "BM".to_string(),
                    location: Some("London, UK".to_string()),
                    url: "https://bm.example/loc".to_string(),
                    source: "bothmismatch".to_string(),
                    description: None,
                    requirements: None,
                    posted_at: None,
                    captured_at: 0,
                    extra,
                };
                if let Some(ref on_item) = ctx.on_item {
                    on_item(job.clone());
                }
                out.push(job);
            }
            Ok(out)
        }
    }
    static BOTH_MISMATCH: std::sync::LazyLock<BothMismatchFake> =
        std::sync::LazyLock::new(|| BothMismatchFake);

    let engine = ScraperEngine::new();
    let mut input = fake_input(10);
    input.location = Some("Berlin".to_string());
    input.work_types = Some(vec![crate::scraping::types::WorkType::Remote]);

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["bothmismatch".to_string()],
            input,
            "job-wt-and-loc-coexist".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| {
                if id == "bothmismatch" {
                    Ok(&*BOTH_MISMATCH as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("ok");

    let s = summaries
        .iter()
        .find(|s| s.board == "bothmismatch")
        .expect("bothmismatch summary missing");
    assert_eq!(
        s.count, 0,
        "row 1 drops on work type, row 2 drops on location — nothing survives; got {s:?}"
    );
    assert_eq!(
        s.notes,
        vec![
            "location-filtered:1".to_string(),
            "work-type-filtered:1".to_string(),
        ],
        "both notes must coexist, location BEFORE work type, each with its OWN \
         independent count; got {s:?}"
    );
}

/// The coexistence test above uses two DISJOINT rows (one fails only work
/// type, one fails only location), so it cannot catch a row that fails BOTH
/// at once — nor does it exercise the LIVE per-item gate at all (its `on_item`
/// param is `None`, so it goes through the post-hoc-only path). A single row
/// failing both is attributed to location ONLY — the composed `keep_item`
/// predicate evaluates the location half first and returns as soon as it
/// drops a row, so the work-type half never runs for that row. This is
/// CORRECT behavior (the drop counts still partition — sum equals total
/// drops, no row counted twice) but `work-type-filtered:0` must still be
/// emitted honestly rather than looking like the filter never ran.
///
/// A real `on_item` sink is wired (unlike the coexistence test) specifically
/// so this exercises the LIVE `location_drops`/`work_type_drops` mutex
/// counters the module comment describes, not just the post-hoc safety net —
/// mutation-checked: reverting the composed predicate to evaluate both halves
/// independently (no short-circuit) makes this fail with
/// `work-type-filtered:1`, confirming the test actually reaches that code.
#[tokio::test]
async fn scrape_boards_single_row_failing_both_filters_attributes_to_location_only() {
    struct BothFailOneRow;
    #[async_trait::async_trait]
    impl Scraper for BothFailOneRow {
        fn id(&self) -> &'static str {
            "bothfailonerow"
        }
        fn display_name(&self) -> &'static str {
            "BothFailOneRow"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            // Mismatches BOTH: wanted location is Berlin, wanted work type is
            // Remote; this row is on-site in London.
            let mut extra = std::collections::HashMap::new();
            extra.insert("workType".to_string(), serde_json::json!("on-site"));
            let job = JobPosting {
                id: "bothfailonerow:1".to_string(),
                external_id: Some("1".to_string()),
                title: "Job".to_string(),
                company: "BFOR".to_string(),
                location: Some("London, UK".to_string()),
                url: "https://bfor.example/1".to_string(),
                source: "bothfailonerow".to_string(),
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
    static BOTH_FAIL_ONE_ROW: std::sync::LazyLock<BothFailOneRow> =
        std::sync::LazyLock::new(|| BothFailOneRow);

    // A real `on_item` sink — required for `has_active_filter` (see `run_one`)
    // to be true, which is what routes items through the LIVE gate instead of
    // the post-hoc-only path. Not asserted on directly; its presence is the
    // point.
    let on_item: Arc<dyn Fn(JobPosting) + Send + Sync> = Arc::new(|_item: JobPosting| {});

    let engine = ScraperEngine::new();
    let mut input = fake_input(10);
    input.location = Some("Berlin".to_string());
    input.work_types = Some(vec![crate::scraping::types::WorkType::Remote]);

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["bothfailonerow".to_string()],
            input,
            "job-both-fail-one-row".to_string(),
            None,
            Some(on_item),
            std::path::Path::new("."),
            |id| {
                if id == "bothfailonerow" {
                    Ok(&*BOTH_FAIL_ONE_ROW as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("ok");

    let s = summaries
        .iter()
        .find(|s| s.board == "bothfailonerow")
        .expect("bothfailonerow summary missing");
    assert_eq!(s.count, 0, "the only row fails both filters; got {s:?}");
    assert_eq!(
        s.notes,
        vec![
            "location-filtered:1".to_string(),
            "work-type-filtered:0".to_string(),
        ],
        "a row failing BOTH filters is counted once, under location — but \
         work-type-filtered must still report 0 honestly, not look like the \
         filter didn't run; got {s:?}"
    );
}
