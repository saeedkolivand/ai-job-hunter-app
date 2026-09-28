//! TRUST PR D: a board's silent location policy (guessed market / sparse-
//! city broadening) surfaces as a per-board `BoardScrapeSummary.notes` entry,
//! attributed to the right board even when a sibling reports nothing or
//! later errors.

use super::support::*;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper};

use super::super::*;

/// PR D — a board that applied a silent location policy (the aggregator's guessed
/// market / sparse-city broadening) reports it through `ctx.report_note`; the
/// engine tags it with the board name and attributes it to THAT board's summary
/// as an entry in `BoardScrapeSummary.notes`, while a sibling that reports
/// nothing stays `notes: []`. Exercised through the engine seam (the aggregator
/// hardcodes its providers) rather than a live network — same pattern as the
/// truncation test.
#[tokio::test]
async fn scrape_boards_surfaces_location_note_on_the_right_summary() {
    struct NotingScraper;

    #[async_trait::async_trait]
    impl Scraper for NotingScraper {
        fn id(&self) -> &'static str {
            "noting"
        }
        fn display_name(&self) -> &'static str {
            "Noting"
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
                id: "noting:0".to_string(),
                external_id: Some("0".to_string()),
                title: "Job".to_string(),
                company: "Note Co".to_string(),
                location: None,
                url: "https://note.example.com/0".to_string(),
                source: "noting".to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra: std::collections::HashMap::new(),
            };
            if let Some(ref on_item) = ctx.on_item {
                on_item(job.clone());
            }
            // A location policy was applied (e.g. no country supplied → market
            // guessed) — surface it, country code only.
            ctx.report_note("guessed-market:de".to_string());
            Ok(vec![job])
        }
    }

    static NOTING: std::sync::LazyLock<NotingScraper> = std::sync::LazyLock::new(|| NotingScraper);
    // A sibling that applies no policy — must report empty `notes`.
    static PLAIN: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(2));

    let engine = ScraperEngine::new();
    let boards = vec!["note-board".to_string(), "plain-board".to_string()];

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(10),
            "job-trust-d-note".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "note-board" => Ok(&*NOTING as &'static dyn Scraper),
                "plain-board" => Ok(&*PLAIN as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("a board that only reports a note still succeeds");

    let noted = summaries
        .iter()
        .find(|s| s.board == "note-board")
        .expect("note-board summary missing");
    assert_eq!(
        noted.notes,
        vec!["guessed-market:de".to_string()],
        "the location-policy note must survive into BoardScrapeSummary.notes on the \
         reporting board"
    );
    assert!(noted.error.is_none() && noted.skipped.is_none() && noted.truncated.is_none());

    let plain = summaries
        .iter()
        .find(|s| s.board == "plain-board")
        .expect("plain-board summary missing");
    assert!(
        plain.notes.is_empty(),
        "a board that applied no location policy must not be tagged with a note; got {plain:?}"
    );
}

/// PR D regression guard — a board that reports a note and THEN fails (returns
/// `Err`) must end up `notes: []` on its summary: the Err arm of
/// `scrape_boards_with_resolver` intentionally does not read the notes map, so
/// the note is dropped rather than misattributed to an error summary that also
/// carries stale/irrelevant location-policy context. Pins the intended Err-arm
/// behavior for any future note-emitting board.
#[tokio::test]
async fn scrape_boards_drops_note_when_board_then_errors() {
    struct NotingThenFailingScraper;

    #[async_trait::async_trait]
    impl Scraper for NotingThenFailingScraper {
        fn id(&self) -> &'static str {
            "noting-failing"
        }
        fn display_name(&self) -> &'static str {
            "NotingThenFailing"
        }
        fn mode(&self) -> ScraperMode {
            ScraperMode::Http
        }
        async fn search(
            &self,
            _input: BoardSearchInput,
            ctx: ScrapeContext,
        ) -> anyhow::Result<Vec<JobPosting>> {
            // Report a location-policy note, then fail — e.g. Adzuna guessed a
            // market and reported it, but the subsequent network call errored.
            ctx.report_note("guessed-market:de".to_string());
            Err(anyhow::anyhow!("board error"))
        }
    }

    static NOTING_FAILING: std::sync::LazyLock<NotingThenFailingScraper> =
        std::sync::LazyLock::new(|| NotingThenFailingScraper);

    let engine = ScraperEngine::new();
    let boards = vec!["noting-failing-board".to_string()];

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &boards,
            fake_input(10),
            "job-trust-d-note-err".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "noting-failing-board" => Ok(&*NOTING_FAILING as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect(
            "one board erroring with no items recovered is still an Ok run \
                 (parent token was never cancelled)",
        );

    let summary = summaries
        .iter()
        .find(|s| s.board == "noting-failing-board")
        .expect("noting-failing-board summary missing");
    assert!(
        summary.notes.is_empty(),
        "a note reported before an Err must be dropped, not attached to the \
         error summary; got {summary:?}"
    );
    assert!(
        summary.error.is_some(),
        "the board error itself must still surface"
    );
}

/// trust-H HIGH fix, widened for the `notes: Vec<String>` shape — a non-location
/// board's OWN note (e.g. an ATS board's `slugs-invalid:<n>`, trust-H) must
/// COEXIST with the central `location-filtered` note, not be silently clobbered
/// by (or clobber) it: `notes` carries both, board-native FIRST. A sibling
/// non-location board that reports no note of its own still gets exactly
/// `location-filtered` alone.
#[tokio::test]
async fn scrape_boards_board_native_note_coexists_with_location_filtered() {
    /// Non-location board (default `supports_location() == false`) that reports
    /// its own note AND streams one row whose location clearly mismatches the
    /// requested "Berlin" — so `dropped > 0` and `location-filtered` would fire
    /// if the board hadn't already reported a note.
    struct NativeNotingNonLocFake;
    #[async_trait::async_trait]
    impl Scraper for NativeNotingNonLocFake {
        fn id(&self) -> &'static str {
            "nativenoting"
        }
        fn display_name(&self) -> &'static str {
            "NativeNoting"
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
                id: "nativenoting:0".to_string(),
                external_id: Some("0".to_string()),
                title: "Job".to_string(),
                company: "NN".to_string(),
                location: Some("London, UK".to_string()),
                url: "https://nn.example/0".to_string(),
                source: "nativenoting".to_string(),
                description: None,
                requirements: None,
                posted_at: None,
                captured_at: 0,
                extra: std::collections::HashMap::new(),
            };
            if let Some(ref on_item) = ctx.on_item {
                on_item(job.clone());
            }
            // Mirrors an ATS board's trust-H partial note (e.g. slugs-invalid:2).
            ctx.report_note("slugs-invalid:2".to_string());
            Ok(vec![job])
        }
    }

    /// Non-location board with no note of its own — the plain case, must still
    /// get `location-filtered` when a location was requested.
    struct QuietNonLocFake;
    #[async_trait::async_trait]
    impl Scraper for QuietNonLocFake {
        fn id(&self) -> &'static str {
            "quietnonloc"
        }
        fn display_name(&self) -> &'static str {
            "QuietNonLoc"
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
                id: "quietnonloc:0".to_string(),
                external_id: Some("0".to_string()),
                title: "Job".to_string(),
                company: "QN".to_string(),
                location: Some("Tokyo".to_string()),
                url: "https://qn.example/0".to_string(),
                source: "quietnonloc".to_string(),
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

    static NATIVE_NOTING: std::sync::LazyLock<NativeNotingNonLocFake> =
        std::sync::LazyLock::new(|| NativeNotingNonLocFake);
    static QUIET_NON_LOC: std::sync::LazyLock<QuietNonLocFake> =
        std::sync::LazyLock::new(|| QuietNonLocFake);

    let engine = ScraperEngine::new();
    let mut input = fake_input(10);
    input.location = Some("Berlin".to_string());

    let (_postings, summaries) = engine
        .scrape_boards_with_resolver(
            &["nativenoting".to_string(), "quietnonloc".to_string()],
            input,
            "job-trust-h-note-precedence".to_string(),
            None,
            None,
            std::path::Path::new("."),
            |id| match id {
                "nativenoting" => Ok(&*NATIVE_NOTING as &'static dyn Scraper),
                "quietnonloc" => Ok(&*QUIET_NON_LOC as &'static dyn Scraper),
                other => Err(anyhow::anyhow!("Unknown board: {other}")),
            },
        )
        .await
        .expect("a located run over two non-location boards is still Ok");

    let native = summaries
        .iter()
        .find(|s| s.board == "nativenoting")
        .expect("nativenoting summary missing");
    assert_eq!(
        native.notes,
        vec![
            "slugs-invalid:2".to_string(),
            "location-filtered:1".to_string()
        ],
        "the board's own note must survive AND location-filtered must still be appended, \
         board-native first; got {native:?}"
    );

    let quiet = summaries
        .iter()
        .find(|s| s.board == "quietnonloc")
        .expect("quietnonloc summary missing");
    assert_eq!(
        quiet.notes,
        vec!["location-filtered:1".to_string()],
        "a board with no note of its own must still get location-filtered; got {quiet:?}"
    );
}
