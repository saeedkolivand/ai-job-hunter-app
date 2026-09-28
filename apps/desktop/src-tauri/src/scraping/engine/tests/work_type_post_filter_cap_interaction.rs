//! Work-type post-filter, continued: the item-cap/filter interaction and
//! the back-compat all-match case.

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

/// A fake board that streams 4 rows (2 clear work-type mismatches interleaved
/// with 2 matches) via `ctx.on_item`, in the SAME order it returns them —
/// mirrors [`CapFilterFake`]'s location-filter counterpart.
struct WorkTypeCapFilterFake;
#[async_trait::async_trait]
impl Scraper for WorkTypeCapFilterFake {
    fn id(&self) -> &'static str {
        "wtcapfilter"
    }
    fn display_name(&self) -> &'static str {
        "WorkTypeCapFilter"
    }
    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }
    async fn search(
        &self,
        _input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let rows: [(&str, &str); 4] = [
            ("row1-mismatch", "on-site"),
            ("row2-match", "remote"),
            ("row3-mismatch", "hybrid"),
            ("row4-match", "remote"),
        ];
        let mut out = Vec::new();
        for (slug, work_type) in rows {
            if ctx.signal.is_cancelled() {
                break;
            }
            let mut extra = std::collections::HashMap::new();
            extra.insert("workType".to_string(), serde_json::json!(work_type));
            let job = JobPosting {
                id: format!("wtcapfilter:{slug}"),
                external_id: Some(slug.to_string()),
                title: "Job".to_string(),
                company: "WTCF".to_string(),
                location: None,
                url: format!("https://wtcf.example/{slug}"),
                source: "wtcapfilter".to_string(),
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

/// HIGH-1 sibling for work type — the item cap must count only MATCHING items;
/// `amount=2`, 4 rows where rows 1 and 3 mismatch → both matches (rows 2 and 4)
/// must be delivered, `count == 2`, and NEITHER mismatch survives (mirrors
/// `scrape_boards_cap_and_location_filter_combined_delivers_only_matches`).
#[tokio::test]
async fn scrape_boards_cap_and_work_type_filter_combined_delivers_only_matches() {
    static WTCAPFILTER: std::sync::LazyLock<WorkTypeCapFilterFake> =
        std::sync::LazyLock::new(|| WorkTypeCapFilterFake);

    let streamed: Arc<std::sync::Mutex<Vec<JobPosting>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let streamed_cb = streamed.clone();
    let on_item: Arc<dyn Fn(JobPosting) + Send + Sync> = Arc::new(move |item: JobPosting| {
        if let Ok(mut g) = streamed_cb.lock() {
            g.push(item);
        }
    });

    let engine = ScraperEngine::new();
    let mut input = fake_input(2); // amount=2 — the cap under test
    input.work_types = Some(vec![crate::scraping::types::WorkType::Remote]);

    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["wtcapfilter".to_string()],
            input,
            "job-wt-cap-and-filter".to_string(),
            None,
            Some(on_item),
            std::path::Path::new("."),
            |id| {
                if id == "wtcapfilter" {
                    Ok(&*WTCAPFILTER as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("ok");

    let s = summaries
        .iter()
        .find(|s| s.board == "wtcapfilter")
        .expect("wtcapfilter summary missing");
    assert_eq!(
        s.count, 2,
        "both real matches must be delivered despite amount=2 and 2 raw \
         mismatches ahead of/interleaved with them; got {s:?}"
    );
    assert_eq!(
        s.notes,
        vec!["work-type-filtered:2".to_string()],
        "both mismatches must be counted as dropped; got {s:?}"
    );

    let ids: std::collections::HashSet<&str> = postings.iter().map(|p| p.id.as_str()).collect();
    assert!(
        ids.contains("wtcapfilter:row2-match") && ids.contains("wtcapfilter:row4-match"),
        "both real matches must survive, including the LATER one (row4); got {postings:?}"
    );
    assert!(
        !ids.contains("wtcapfilter:row1-mismatch") && !ids.contains("wtcapfilter:row3-mismatch"),
        "neither mismatch may survive; got {postings:?}"
    );
}

/// Same shape as `scrape_boards_zero_drops_still_emits_unconditional_note_for_non_supporting_board`,
/// for work type: a non-supporting board whose rows all declare a wanted type
/// still gets `work-type-filtered:0` (checked, nothing hidden), while a
/// supporting board never gets the note at all regardless of its own data.
#[tokio::test]
async fn scrape_boards_work_type_zero_drops_still_emits_unconditional_note() {
    struct AllMatchNonSupportingWt;
    #[async_trait::async_trait]
    impl Scraper for AllMatchNonSupportingWt {
        fn id(&self) -> &'static str {
            "wtallmatch"
        }
        fn display_name(&self) -> &'static str {
            "WtAllMatch"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            let mut extra = std::collections::HashMap::new();
            extra.insert("workType".to_string(), serde_json::json!("remote"));
            let job = JobPosting {
                id: "wtallmatch:0".to_string(),
                external_id: Some("0".to_string()),
                title: "Job".to_string(),
                company: "WTAM".to_string(),
                location: None,
                url: "https://wtam.example/0".to_string(),
                source: "wtallmatch".to_string(),
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
    static WTALLMATCH: std::sync::LazyLock<AllMatchNonSupportingWt> =
        std::sync::LazyLock::new(|| AllMatchNonSupportingWt);

    let engine = ScraperEngine::new();
    let mut input = fake_input(10);
    input.work_types = Some(vec![crate::scraping::types::WorkType::Remote]);

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["wtallmatch".to_string()],
            input,
            "job-wt-zero-drop-note".to_string(),
            None,
            None, // no live on_item — exercises the post-hoc path
            std::path::Path::new("."),
            |id| {
                if id == "wtallmatch" {
                    Ok(&*WTALLMATCH as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("ok");

    let s = summaries
        .iter()
        .find(|s| s.board == "wtallmatch")
        .expect("wtallmatch summary missing");
    assert_eq!(s.count, 1, "the matching row is kept; got {s:?}");
    assert_eq!(
        s.notes,
        vec!["work-type-filtered:0".to_string()],
        "a non-supporting board must emit the note even with ZERO drops when a \
         work type was requested; got {s:?}"
    );
}
