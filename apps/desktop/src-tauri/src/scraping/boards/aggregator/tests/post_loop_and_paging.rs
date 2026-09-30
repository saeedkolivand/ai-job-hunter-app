use super::super::*;
use super::support::*;

// ── Post-loop policy wiring (AdzunaProvider::search over the page loop) ──────
//
// `should_broaden` and `guessed_market_note` both read the loop's POST-DEDUP
// count, so the loop and the policy on top of it are one seam. These drive the
// real `search` (not the fetchers underneath it) through a mock host.

/// Sparse city result → the country-wide broaden retry still fires ON TOP of the
/// page loop, and its larger result set wins. Page 1 is short (2 < the floor of
/// 3), so the loop stops after one fetch and the retry is the second.
#[tokio::test]
async fn search_broadens_country_wide_after_a_sparse_paged_result() {
    use wiremock::matchers::{path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    // Narrow (`where=Berlin`) → 2 results, under ADZUNA_BROADEN_FLOOR.
    Mock::given(path(adzuna_path(1)))
        .and(query_param("where", "Berlin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(1, 2)))
        .expect(1)
        .mount(&server)
        .await;
    // Broadened (`where=`) → more results, so it replaces the narrow set.
    Mock::given(path(adzuna_path(1)))
        .and(query_param("where", ""))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(100, 7)))
        .expect(1)
        .mount(&server)
        .await;

    let notes: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink_notes = notes.clone();
    let provider = wiremock_adzuna(
        server.uri(),
        Some(std::sync::Arc::new(move |n: String| {
            sink_notes.lock().expect("note sink mutex").push(n)
        })),
    );

    let items = provider
        // country_guessed = false → an explicit market, so broadening is allowed.
        .search(
            "engineer",
            "Berlin",
            "de",
            false,
            None,
            Some(100),
            make_token(),
        )
        .await
        .unwrap();

    assert_eq!(items.len(), 7, "the broadened set replaces the sparse one");
    assert_eq!(
        notes.lock().expect("note sink mutex").as_slice(),
        ["broadened:de"],
        "the broadening must surface as a user-facing note"
    );
}

/// A Stop must not buy the broaden retry's extra quota call.
///
/// A cancelled run is short BY CONSTRUCTION (the page loop returns what it had),
/// so without an explicit cancellation guard the deliberate stop looks exactly
/// like a sparse market and spends one more metered request on the way out — the
/// opposite of what pressing Stop means.
#[tokio::test]
async fn search_does_not_broaden_after_a_cancelled_page_loop() {
    use wiremock::matchers::{path, query_param};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    struct CancelWhileServing {
        token: tokio_util::sync::CancellationToken,
        body: serde_json::Value,
    }
    impl Respond for CancelWhileServing {
        fn respond(&self, _req: &Request) -> ResponseTemplate {
            self.token.cancel();
            ResponseTemplate::new(200).set_body_json(self.body.clone())
        }
    }

    let signal = make_token();
    let server = MockServer::start().await;
    // Narrow page 1 lands with 2 results (under ADZUNA_BROADEN_FLOOR) and cancels.
    Mock::given(path(adzuna_path(1)))
        .and(query_param("where", "Berlin"))
        .respond_with(CancelWhileServing {
            token: signal.clone(),
            body: adzuna_body(1, 2),
        })
        .expect(1)
        .mount(&server)
        .await;
    // The country-wide retry must NEVER be issued after a Stop.
    Mock::given(path(adzuna_path(1)))
        .and(query_param("where", ""))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(100, 7)))
        .expect(0)
        .mount(&server)
        .await;

    let provider = wiremock_adzuna(server.uri(), None);
    let items = provider
        // country_guessed = false → broadening would otherwise be allowed here.
        .search("engineer", "Berlin", "de", false, None, Some(100), signal)
        .await
        .expect("a cancelled search is a clean stop");

    assert_eq!(
        items.len(),
        2,
        "the narrow result collected before the Stop is kept as-is"
    );
    // The `.expect(0)` above is the real assertion, verified on MockServer drop.
}

/// A GUESSED market that returns an authoritative (>= floor) PAGED result emits
/// the guessed-market note — the count it is judged on is the loop's accumulated,
/// post-dedup total, so this is the loop→policy wiring, not just the helper.
#[tokio::test]
async fn search_reports_guessed_market_note_for_a_paged_authoritative_result() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(path(adzuna_path(1)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(1, 50)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(adzuna_path(2)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(51, 4)))
        .expect(1)
        .mount(&server)
        .await;

    let notes: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink_notes = notes.clone();
    let provider = wiremock_adzuna(
        server.uri(),
        Some(std::sync::Arc::new(move |n: String| {
            sink_notes.lock().expect("note sink mutex").push(n)
        })),
    );

    let items = provider
        // country_guessed = true → "de" was a GUESS, not a supplied target.
        .search(
            "engineer",
            "Berlin",
            "de",
            true,
            None,
            Some(100),
            make_token(),
        )
        .await
        .unwrap();

    assert_eq!(items.len(), 54, "both pages are accumulated");
    assert_eq!(
        notes.lock().expect("note sink mutex").as_slice(),
        ["guessed-market:de"],
        "a guessed market that produced the authoritative result must say so"
    );
    // A guessed market must NEVER broaden (that would defeat primary_chain's
    // guessed-market → JSearch fallback) — the `.expect(1)`s above pin that no
    // third, `where=`-broadened request was issued.
}

// ── JSearch num_pages mapping ────────────────────────────────────────────────

/// JSearch returns 10 per page and bills `num_pages` multiplicatively, so the
/// mapping is `ceil(amount / 10)` hard-clamped to `JSEARCH_MAX_PAGES`.
#[test]
fn jsearch_num_pages_is_amount_driven_and_clamped() {
    assert_eq!(jsearch_num_pages(None), 1, "no target → cheapest form");
    assert_eq!(jsearch_num_pages(Some(0)), 1, "0 must never mean 0 pages");
    assert_eq!(jsearch_num_pages(Some(1)), 1);
    assert_eq!(
        jsearch_num_pages(Some(JSEARCH_PAGE_SIZE)),
        1,
        "an exact page does not spill into a second billed page"
    );
    assert_eq!(jsearch_num_pages(Some(JSEARCH_PAGE_SIZE + 1)), 2);
    assert_eq!(jsearch_num_pages(Some(25)), 3);
    assert_eq!(
        jsearch_num_pages(Some(100)),
        JSEARCH_MAX_PAGES,
        "the UI's max amount is still capped at the billing ceiling"
    );
    assert_eq!(jsearch_num_pages(Some(u32::MAX)), JSEARCH_MAX_PAGES);
}

/// The mapping has to reach the WIRE, not just the helper: `jsearch_url` builds
/// exactly what `JSearchProvider::search` sends, so the `num_pages` it carries is
/// the billed page count.
#[test]
fn jsearch_url_carries_the_amount_derived_num_pages() {
    let one = jsearch_url(JSEARCH_BASE_URL, "engineer in Berlin", None, Some(10));
    assert!(
        one.contains("num_pages=1"),
        "an amount within one page must stay a single-page request; got: {one}"
    );

    let three = jsearch_url(
        JSEARCH_BASE_URL,
        "engineer in Berlin",
        Some("week"),
        Some(100),
    );
    assert!(
        three.contains(&format!("num_pages={JSEARCH_MAX_PAGES}")),
        "a large amount must request the clamped page count; got: {three}"
    );
    // The paging change must not disturb the freshness contract.
    assert!(
        three.contains("date_posted=week") && three.contains("sort_by=date"),
        "date window + newest-first sort must survive; got: {three}"
    );
    assert!(
        three.contains("query=engineer%20in%20Berlin"),
        "the combined query must stay URL-encoded; got: {three}"
    );
}
