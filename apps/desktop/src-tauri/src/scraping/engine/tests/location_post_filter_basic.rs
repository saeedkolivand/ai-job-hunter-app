//! TRUST PR F: the central location post-filter — a non-supporting board's
//! results are conservatively filtered against the requested location, and
//! a supporting board's results pass through untouched (no double-filter).

use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

// ── TRUST PR F: central conservative location post-filter ─────────────────────

/// TRUST PR F — for a board WITHOUT server-side location support
/// (`supports_location() == false`), the engine drops postings whose OWN location
/// clearly mismatches the requested one, but conservatively keeps remote and
/// unknown-location rows. The drop count surfaces as a `location-filtered:<n>`
/// note (PR D grammar). A board that DOES consume location server-side is left
/// untouched — re-filtering it could drop a legitimate in-radius match its server
/// correctly included. Exercised through the engine seam (boards hardcode hosts).
#[tokio::test]
async fn scrape_boards_central_location_filter_drops_only_clear_mismatches() {
    // Ignores location (supports_location defaults false). Returns a mix: matching
    // city, clear mismatch, remote-flagged, and unknown-location.
    struct LocationFake;
    #[async_trait::async_trait]
    impl Scraper for LocationFake {
        fn id(&self) -> &'static str {
            "locfake"
        }
        fn display_name(&self) -> &'static str {
            "LocFake"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            let rows: [(&str, Option<&str>, bool); 4] = [
                ("keep-berlin", Some("Berlin, Germany"), false),
                ("drop-london", Some("London, UK"), false),
                ("keep-remote", Some("USA Only"), true), // remote flag → keep
                ("keep-unknown", None, false),           // unknown → keep
            ];
            let mut out = Vec::new();
            for (slug, loc, remote) in rows {
                let mut extra = std::collections::HashMap::new();
                if remote {
                    extra.insert("remote".to_string(), serde_json::json!(true));
                }
                let job = JobPosting {
                    id: format!("locfake:{slug}"),
                    external_id: Some(slug.to_string()),
                    title: "Job".to_string(),
                    company: "LF".to_string(),
                    location: loc.map(str::to_string),
                    url: format!("https://lf.example/{slug}"),
                    source: "locfake".to_string(),
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

    // A board that DOES consume location server-side — the central filter must
    // leave it alone even though it returns a row for a different city name.
    struct LocationAwareFake;
    #[async_trait::async_trait]
    impl Scraper for LocationAwareFake {
        fn id(&self) -> &'static str {
            "locaware"
        }
        fn display_name(&self) -> &'static str {
            "LocAware"
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
            // An in-radius suburb the server correctly included — must survive.
            let job = JobPosting {
                id: "locaware:0".to_string(),
                external_id: Some("0".to_string()),
                title: "Job".to_string(),
                company: "LA".to_string(),
                location: Some("Potsdam".to_string()),
                url: "https://la.example/0".to_string(),
                source: "locaware".to_string(),
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

    static LOCFAKE: std::sync::LazyLock<LocationFake> = std::sync::LazyLock::new(|| LocationFake);
    static LOCAWARE: std::sync::LazyLock<LocationAwareFake> =
        std::sync::LazyLock::new(|| LocationAwareFake);

    let engine = ScraperEngine::new();
    let input = BoardSearchInput {
        query: "q".to_string(),
        location: Some("Berlin".to_string()),
        amount: 100,
        pages: 10,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: Vec::new(),
    };

    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["locfake".to_string(), "locaware".to_string()],
            input,
            "job-trust-f-locfilter".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "locfake" => Ok(&*LOCFAKE as &'static dyn Scraper),
                "locaware" => Ok(&*LOCAWARE as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("a filtered run is still Ok");

    // Non-supporting board: London dropped; Berlin + remote + unknown kept (3).
    let locfake = summaries
        .iter()
        .find(|s| s.board == "locfake")
        .expect("locfake summary missing");
    assert_eq!(
        locfake.count, 3,
        "only the clear London mismatch is dropped; got {locfake:?}"
    );
    assert_eq!(
        locfake.notes,
        vec!["location-filtered:1".to_string()],
        "the single drop must surface as a location-filtered note; got {locfake:?}"
    );
    assert!(
        !postings
            .iter()
            .any(|p| p.location.as_deref() == Some("London, UK")),
        "the wrong-city row must not appear in the aggregated result"
    );
    assert!(
        postings
            .iter()
            .any(|p| p.location.as_deref() == Some("Berlin, Germany")),
        "the matching-city row must survive"
    );
    assert!(
        postings
            .iter()
            .any(|p| p.source == "locfake" && p.location.is_none()),
        "the unknown-location row must survive (never dropped)"
    );

    // Location-aware board: NOT filtered — its differently-named-city row survives
    // and it carries no location-filtered note.
    let locaware = summaries
        .iter()
        .find(|s| s.board == "locaware")
        .expect("locaware summary missing");
    assert_eq!(
        locaware.count, 1,
        "a server-side location board is not re-filtered; got {locaware:?}"
    );
    assert!(
        locaware.notes.is_empty(),
        "a supporting board must not get a location-filtered note; got {locaware:?}"
    );
    assert!(
        postings
            .iter()
            .any(|p| p.location.as_deref() == Some("Potsdam")),
        "the location-aware board's in-radius row must survive the central filter"
    );
}
