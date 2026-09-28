//! Location post-filter, continued: the back-compat inert case (no
//! location requested) and the item-cap/filter interaction — the cap must
//! count only matching items.

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

/// TRUST PR F back-compat — with NO location requested, a non-supporting board's
/// results pass through byte-identically (the central filter is inert).
#[tokio::test]
async fn scrape_boards_no_location_requested_is_inert() {
    struct AnywhereFake;
    #[async_trait::async_trait]
    impl Scraper for AnywhereFake {
        fn id(&self) -> &'static str {
            "anywherefake"
        }
        fn display_name(&self) -> &'static str {
            "AnywhereFake"
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
            for (i, loc) in ["London, UK", "Tokyo", "Paris"].into_iter().enumerate() {
                let job = JobPosting {
                    id: format!("anywherefake:{i}"),
                    external_id: Some(i.to_string()),
                    title: "Job".to_string(),
                    company: "AF".to_string(),
                    location: Some(loc.to_string()),
                    url: format!("https://af.example/{i}"),
                    source: "anywherefake".to_string(),
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
            Ok(out)
        }
    }
    static ANYWHERE: std::sync::LazyLock<AnywhereFake> = std::sync::LazyLock::new(|| AnywhereFake);

    let engine = ScraperEngine::new();
    // `fake_input` has `location: None` → no location requested → filter inert.
    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["anywherefake".to_string()],
            fake_input(100),
            "job-trust-f-inert".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| {
                if id == "anywherefake" {
                    Ok(&*ANYWHERE as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("ok");

    assert_eq!(
        postings.len(),
        3,
        "no location requested → nothing dropped; got {postings:?}"
    );
    let s = summaries
        .iter()
        .find(|s| s.board == "anywherefake")
        .expect("anywherefake summary missing");
    assert_eq!(s.count, 3, "all rows kept when no location was requested");
    assert!(
        s.notes.is_empty(),
        "no location requested → no location-filtered note; got {s:?}"
    );
}

/// A fake board that streams 4 rows (2 clear location mismatches interleaved
/// with 2 matches) via `ctx.on_item`, in the SAME row order it returns them —
/// mirrors the real board loop pattern (`on_item` then `out.push`, no gap).
pub(super) struct CapFilterFake;

#[async_trait::async_trait]
impl Scraper for CapFilterFake {
    fn id(&self) -> &'static str {
        "capfilter"
    }
    fn display_name(&self) -> &'static str {
        "CapFilter"
    }
    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }
    async fn search(
        &self,
        _input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Rows 1 and 3 clearly mismatch a "Berlin" request; rows 2 and 4 match.
        let rows: [(&str, &str); 4] = [
            ("row1-mismatch", "London, UK"),
            ("row2-match", "Berlin, Germany"),
            ("row3-mismatch", "Paris, France"),
            ("row4-match", "Berlin, Mitte"),
        ];
        let mut out = Vec::new();
        for (slug, loc) in rows {
            if ctx.signal.is_cancelled() {
                break;
            }
            let job = JobPosting {
                id: format!("capfilter:{slug}"),
                external_id: Some(slug.to_string()),
                title: "Job".to_string(),
                company: "CF".to_string(),
                location: Some(loc.to_string()),
                url: format!("https://cf.example/{slug}"),
                source: "capfilter".to_string(),
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
        Ok(out)
    }
}

/// HIGH-1 — cap/filter ordering: the item cap must count only MATCHING items,
/// never raw pre-filter items, and the final result must be the true matching
/// set (not a naive truncate of the raw board return, which can keep an early
/// mismatch while dropping a later real match — see `run_one`'s `has_active_filter`
/// branch). `amount=2`, 4 rows where rows 1 and 3 mismatch → both matches (rows
/// 2 and 4) must be delivered, `count == 2`, and NEITHER mismatch survives.
#[tokio::test]
async fn scrape_boards_cap_and_location_filter_combined_delivers_only_matches() {
    static CAPFILTER: std::sync::LazyLock<CapFilterFake> =
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
    let mut input = fake_input(2); // amount=2 — the cap under test
    input.location = Some("Berlin".to_string());

    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["capfilter".to_string()],
            input,
            "job-trust-f-cap-and-filter".to_string(),
            None,
            Some(on_item),
            std::path::Path::new("."),
            |id| {
                if id == "capfilter" {
                    Ok(&*CAPFILTER as &'static dyn Scraper)
                } else {
                    Err(anyhow::anyhow!("Unknown board: {id}"))
                }
            },
        )
        .await
        .expect("ok");

    let s = summaries
        .iter()
        .find(|s| s.board == "capfilter")
        .expect("capfilter summary missing");
    assert_eq!(
        s.count, 2,
        "both real matches must be delivered despite amount=2 and 2 raw \
         mismatches ahead of/interleaved with them; got {s:?}"
    );
    assert_eq!(
        s.notes,
        vec!["location-filtered:2".to_string()],
        "both mismatches must be counted as dropped; got {s:?}"
    );

    let ids: std::collections::HashSet<&str> = postings.iter().map(|p| p.id.as_str()).collect();
    assert!(
        ids.contains("capfilter:row2-match") && ids.contains("capfilter:row4-match"),
        "both real matches must survive, including the LATER one (row4) that a naive \
         raw truncate(2) would have discarded in favor of the earlier mismatch (row1); \
         got {postings:?}"
    );
    assert!(
        !ids.contains("capfilter:row1-mismatch") && !ids.contains("capfilter:row3-mismatch"),
        "neither mismatch may survive; got {postings:?}"
    );
}
