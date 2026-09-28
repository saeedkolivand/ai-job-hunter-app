//! `ats_seed` auto-population (P2 sourcing depth): an ATS-requiring board
//! with no explicit companies is seeded from the curated `ats_seed` table
//! by `Scraper::id()`, keyed on the board-list id, and left untouched when
//! the caller already supplied companies.

use std::collections::HashMap;

use super::support::*;
use crate::scraping::types::Scraper;

use super::super::*;

// ── ats_seed auto-population (P2 sourcing depth) ──────────────────────────────

/// A real registered id ("greenhouse") with curated `ats_seed` entries and an
/// EMPTY global `companies` list must (a) NOT be skipped, and (b) receive the
/// seed's real slugs — in the seed's order and casing — driven through the live
/// `ats_seed::by_ats` table (real static SEED — no injection).
#[tokio::test]
async fn ats_seed_populates_companies_for_a_seeded_board_and_it_is_not_skipped() {
    static SCRAPER: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::ats("greenhouse"));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = fake_input(5); // companies: Vec::new() — global list is empty

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["greenhouse".to_string()],
            input,
            "job-ats-seed-populates".to_string(),
            None,
            None,
            tmp.path(),
            |_id| Ok(&*SCRAPER as &'static dyn Scraper),
        )
        .await
        .expect("seeded ATS board must return Ok");

    assert_eq!(summaries.len(), 1);
    assert!(
        summaries[0].skipped.is_none(),
        "a seeded ATS board must NOT be skipped even with an empty global \
         company list; got {:?}",
        summaries[0].skipped
    );

    let expected: Vec<String> = crate::scraping::boards::ats_seed::by_ats("greenhouse")
        .map(|e| e.slug.to_string())
        .collect();
    assert!(!expected.is_empty(), "greenhouse must have seed entries");

    let received = SCRAPER
        .captured
        .lock()
        .unwrap()
        .clone()
        .expect("search() must have been called and captured input.companies");
    assert_eq!(
        received, expected,
        "the board must receive the seed's slugs, in seed order and casing"
    );
}

/// When the user supplies an explicit (non-empty) `companies` list, a seeded ATS
/// board must receive exactly the USER's list — the seed must never override an
/// explicit choice.
#[tokio::test]
async fn explicit_companies_override_the_seed_for_a_seeded_board() {
    static SCRAPER: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::ats("greenhouse"));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut input = fake_input(5);
    input.companies = vec!["user-typed-co".to_string()];

    let _ = engine
        .scrape_boards_with_resolver(
            &["greenhouse".to_string()],
            input,
            "job-ats-seed-user-override".to_string(),
            None,
            None,
            tmp.path(),
            |_id| Ok(&*SCRAPER as &'static dyn Scraper),
        )
        .await
        .expect("ATS board with explicit companies must return Ok");

    let received = SCRAPER
        .captured
        .lock()
        .unwrap()
        .clone()
        .expect("search() must have been called");
    assert_eq!(
        received,
        vec!["user-typed-co".to_string()],
        "an explicit user company list must win over the ats_seed table"
    );
}

/// Watched-companies routing (ADR-030 §e): greenhouse + ashby selected, but ONLY
/// ashby has watched slugs. ashby must receive EXACTLY its own slugs; greenhouse
/// must be skipped `needs-company` — never fetched (no cross-ATS request, no
/// `ats_seed` fallback). This is the mock-transport end-to-end proof for the HIGH
/// review finding.
#[tokio::test]
async fn watched_overrides_route_per_ats_and_skip_boards_without_stars() {
    static GH: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::ats("greenhouse"));
    static ASHBY: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::ats("ashby"));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    // Empty global companies — the watched path routes per board via the override.
    let overrides: HashMap<String, Vec<String>> = HashMap::from([(
        "ashby".to_string(),
        vec!["ramp".to_string(), "notion".to_string()],
    )]);

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver_and_overrides(
            &["greenhouse".to_string(), "ashby".to_string()],
            fake_input(5),
            "job-watched-per-ats".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "greenhouse" => Ok(&*GH as &'static dyn Scraper),
                "ashby" => Ok(&*ASHBY as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unexpected board {other}")),
            },
            Some(&overrides),
        )
        .await
        .expect("watched run must return Ok");

    // ashby received EXACTLY its own slugs (no cross-ATS mixing).
    let ashby_got = ASHBY
        .captured
        .lock()
        .unwrap()
        .clone()
        .expect("ashby must have been fetched with its watched slugs");
    assert_eq!(ashby_got, vec!["ramp".to_string(), "notion".to_string()]);

    // greenhouse (no watched slug) was skipped `needs-company` and NEVER fetched.
    assert!(
        GH.captured.lock().unwrap().is_none(),
        "greenhouse must NOT be fetched when it has no watched slugs (no foreign fan-out)"
    );
    let gh = summaries
        .iter()
        .find(|s| s.board == "greenhouse")
        .expect("greenhouse summary present");
    assert_eq!(gh.skipped.as_deref(), Some("needs-company"));
    assert_eq!(gh.count, 0);
}

/// Watched routing keeps each board's company list PURE: with stars for both
/// boards, greenhouse receives ONLY greenhouse slugs and ashby ONLY ashby slugs,
/// so a board's `MAX_COMPANIES` cap is spent on its own slugs and never crowded by
/// a foreign ATS's (the flat-union bug ADR-030 §e closes).
#[tokio::test]
async fn watched_overrides_keep_each_board_list_pure_no_foreign_crowding() {
    static GH: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::ats("greenhouse"));
    static ASHBY: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::ats("ashby"));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let gh_slugs: Vec<String> = (0..5).map(|i| format!("gh-{i}")).collect();
    let ashby_slugs: Vec<String> = (0..5).map(|i| format!("ashby-{i}")).collect();
    let overrides: HashMap<String, Vec<String>> = HashMap::from([
        ("greenhouse".to_string(), gh_slugs.clone()),
        ("ashby".to_string(), ashby_slugs.clone()),
    ]);

    engine
        .scrape_boards_with_resolver_and_overrides(
            &["greenhouse".to_string(), "ashby".to_string()],
            fake_input(5),
            "job-watched-pure".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "greenhouse" => Ok(&*GH as &'static dyn Scraper),
                "ashby" => Ok(&*ASHBY as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unexpected board {other}")),
            },
            Some(&overrides),
        )
        .await
        .expect("watched run must return Ok");

    let gh_got = GH.captured.lock().unwrap().clone().expect("greenhouse ran");
    let ashby_got = ASHBY.captured.lock().unwrap().clone().expect("ashby ran");
    assert_eq!(gh_got, gh_slugs, "greenhouse gets ONLY its own slugs");
    assert_eq!(ashby_got, ashby_slugs, "ashby gets ONLY its own slugs");
    assert!(
        gh_got.iter().all(|s| s.starts_with("gh-")),
        "no foreign (ashby) slug crowded greenhouse's list: {gh_got:?}"
    );
    assert!(
        ashby_got.iter().all(|s| s.starts_with("ashby-")),
        "no foreign (greenhouse) slug crowded ashby's list: {ashby_got:?}"
    );
}

/// A watched override whose entry is PRESENT but EMPTY (`{"greenhouse": []}`) must
/// be treated as absent — the board is skipped `needs-company` and never fetched.
/// Pins the empty-vec branch of the `overrides.get(id).map(|s| !s.is_empty())
/// .unwrap_or(false)` skip gate (engine/mod.rs), distinct from a MISSING key.
#[tokio::test]
async fn watched_override_present_but_empty_is_treated_as_absent() {
    static GH: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::ats("greenhouse"));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    // Present key, EMPTY slug list — must NOT run greenhouse.
    let mut overrides: HashMap<String, Vec<String>> = HashMap::new();
    overrides.insert("greenhouse".to_string(), Vec::new());

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver_and_overrides(
            &["greenhouse".to_string()],
            fake_input(5),
            "job-watched-empty-override".to_string(),
            None,
            None,
            tmp.path(),
            |_id| Ok(&*GH as &'static dyn Scraper),
            Some(&overrides),
        )
        .await
        .expect("run must return Ok");

    assert!(
        GH.captured.lock().unwrap().is_none(),
        "greenhouse must NOT be fetched when its override entry is present-but-empty"
    );
    assert_eq!(summaries.len(), 1);
    assert_eq!(
        summaries[0].skipped.as_deref(),
        Some("needs-company"),
        "a present-but-empty override entry is treated as absent (needs-company skip)"
    );
    assert_eq!(summaries[0].count, 0);
}

/// A non-ATS board's `companies` field must be untouched by the seed
/// auto-population, and running it alongside a seeded ATS board must not
/// disturb result ordering or summary/count assembly.
#[tokio::test]
async fn non_ats_board_companies_untouched_alongside_a_seeded_board() {
    static ATS: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::ats("greenhouse"));
    static NON_ATS: std::sync::LazyLock<SeedCapturingScraper> =
        std::sync::LazyLock::new(|| SeedCapturingScraper::non_ats("guest-board"));

    let engine = ScraperEngine::new();
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = fake_input(5); // companies: Vec::new()

    let (postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["greenhouse".to_string(), "guest-board".to_string()],
            input,
            "job-ats-seed-mixed".to_string(),
            None,
            None,
            tmp.path(),
            |id| match id {
                "greenhouse" => Ok(&*ATS as &'static dyn Scraper),
                "guest-board" => Ok(&*NON_ATS as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("unknown: {other}")),
            },
        )
        .await
        .expect("mixed run must return Ok");

    // Order preserved (input-board order), summaries/counts assembled normally.
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].board, "greenhouse");
    assert_eq!(summaries[1].board, "guest-board");
    assert!(summaries[0].skipped.is_none());
    assert!(summaries[1].skipped.is_none());
    assert_eq!(summaries[0].count, 0);
    assert_eq!(summaries[1].count, 0);
    assert_eq!(
        postings.len(),
        0,
        "both fakes return an empty Vec by design"
    );

    let non_ats_received = NON_ATS
        .captured
        .lock()
        .unwrap()
        .clone()
        .expect("search() must have been called");
    assert!(
        non_ats_received.is_empty(),
        "a non-ATS board's companies must be untouched by the seed \
         auto-population; got {non_ats_received:?}"
    );
}
