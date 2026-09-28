//! Location post-filter, continued: the cap/filter-combined delivers-only-
//! matches case, the live-stream-vs-final-result agreement invariant, and
//! the `notes` entry a non-supporting board gets even at zero drops.

use super::location_post_filter_cap_interaction::CapFilterFake;
use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

/// MEDIUM — the live stream gate and the final returned Vec must drop the
/// IDENTICAL set: every item forwarded to the caller's `on_item` during the
/// run must also be present in the final result, and vice versa (no item
/// streamed-then-dropped, and no item present-in-result-but-never-streamed).
#[tokio::test]
async fn scrape_boards_stream_and_final_result_agree_under_location_filter() {
    static CAPFILTER2: std::sync::LazyLock<CapFilterFake> =
        std::sync::LazyLock::new(|| CapFilterFake);

    let streamed: Arc<std::sync::Mutex<Vec<JobPosting>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let streamed_cb = streamed.clone();
    let on_item: Arc<dyn Fn(JobPosting) + Send + Sync> = Arc::new(move |item: JobPosting| {
        if let Ok(mut g) = streamed_cb.lock() {
            g.push(item);
        }
    });

    let engine = ScraperEngine::new();
    let mut input = fake_input(100); // no cap interaction — isolates this invariant
    input.location = Some("Berlin".to_string());

    let (postings, _summaries) = engine
        .scrape_boards_with_resolver(
            &["capfilter2".to_string()],
            input,
            "job-trust-f-stream-agree".to_string(),
            None,
            Some(on_item),
            std::path::Path::new("."),
            |id| {
                if id == "capfilter2" {
                    Ok(&*CAPFILTER2 as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("ok");

    let streamed_ids: std::collections::HashSet<String> = streamed
        .lock()
        .expect("lock")
        .iter()
        .map(|p| p.id.clone())
        .collect();
    let final_ids: std::collections::HashSet<String> =
        postings.iter().map(|p| p.id.clone()).collect();

    assert_eq!(
        streamed_ids, final_ids,
        "the live-streamed kept set must exactly equal the final returned set"
    );
    // Sanity: the agreeing set is exactly the 2 real matches, not e.g. empty sets
    // trivially "agreeing".
    assert_eq!(
        streamed_ids.len(),
        2,
        "2 real matches expected in both sets"
    );
}

/// Trust-story completeness (frontend-reviewer follow-up): the
/// `location-filtered:<n>` note must be UNCONDITIONAL for a non-supporting
/// board when a location was requested — including `n=0`. Emitting it only on
/// `dropped>0` let a non-supporting board that happened to have zero mismatches
/// this run read as indistinguishable from a genuinely location-aware one (a
/// clean chip, "all ok"), half-telling the 17/23-boards-ignore-location story.
/// Covers all three cases: non-supporting+location+0 drops → note "…:0";
/// supporting board+location → no note; (no-location case is already covered
/// by `scrape_boards_no_location_requested_is_inert`).
#[tokio::test]
async fn scrape_boards_zero_drops_still_emits_unconditional_note_for_non_supporting_board() {
    // Non-supporting board whose rows ALL match the request — 0 drops, but the
    // note must still fire since this board never honored location server-side.
    struct AllMatchNonSupporting;
    #[async_trait::async_trait]
    impl Scraper for AllMatchNonSupporting {
        fn id(&self) -> &'static str {
            "allmatch"
        }
        fn display_name(&self) -> &'static str {
            "AllMatch"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            let job = JobPosting {
                id: "allmatch:0".to_string(),
                external_id: Some("0".to_string()),
                title: "Job".to_string(),
                company: "AM".to_string(),
                location: Some("Berlin, Germany".to_string()),
                url: "https://am.example/0".to_string(),
                source: "allmatch".to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra: std::collections::HashMap::new(),
            };
            if let Some(ref on_item) = ctx.on_item {
                on_item(job.clone());
            }
            Ok(vec![job])
        }
    }

    // Supporting board — must NEVER get a location-filtered note, regardless of
    // its own location text (the central filter never touches it).
    struct SupportingBoard;
    #[async_trait::async_trait]
    impl Scraper for SupportingBoard {
        fn id(&self) -> &'static str {
            "supporting"
        }
        fn display_name(&self) -> &'static str {
            "Supporting"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        fn supports_location(&self) -> bool {
            true
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            let job = JobPosting {
                id: "supporting:0".to_string(),
                external_id: Some("0".to_string()),
                title: "Job".to_string(),
                company: "SB".to_string(),
                location: Some("Munich".to_string()), // doesn't matter — never filtered
                url: "https://sb.example/0".to_string(),
                source: "supporting".to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra: std::collections::HashMap::new(),
            };
            if let Some(ref on_item) = ctx.on_item {
                on_item(job.clone());
            }
            Ok(vec![job])
        }
    }

    static ALLMATCH: std::sync::LazyLock<AllMatchNonSupporting> =
        std::sync::LazyLock::new(|| AllMatchNonSupporting);
    static SUPPORTING: std::sync::LazyLock<SupportingBoard> =
        std::sync::LazyLock::new(|| SupportingBoard);

    let engine = ScraperEngine::new();
    let mut input = fake_input(10);
    input.location = Some("Berlin".to_string());

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["allmatch".to_string(), "supporting".to_string()],
            input,
            "job-trust-f-zero-drop-note".to_string(),
            None,
            None, // no live on_item — exercises the post-hoc path
            std::path::Path::new("."),
            |id| match id {
                "allmatch" => Ok(&*ALLMATCH as &'static dyn Scraper),
                "supporting" => Ok(&*SUPPORTING as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("ok");

    let allmatch = summaries
        .iter()
        .find(|s| s.board == "allmatch")
        .expect("allmatch summary missing");
    assert_eq!(
        allmatch.count, 1,
        "the matching row is kept; got {allmatch:?}"
    );
    assert_eq!(
        allmatch.notes,
        vec!["location-filtered:0".to_string()],
        "a non-supporting board must emit the note even with ZERO drops when a \
         location was requested; got {allmatch:?}"
    );

    let supporting = summaries
        .iter()
        .find(|s| s.board == "supporting")
        .expect("supporting summary missing");
    assert!(
        supporting.notes.is_empty(),
        "a server-side location board must never carry a location-filtered note; \
         got {supporting:?}"
    );
}
