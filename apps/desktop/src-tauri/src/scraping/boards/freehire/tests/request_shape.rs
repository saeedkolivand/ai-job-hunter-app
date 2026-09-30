//! `fetch_freehire` tests: the ignored-params guard, non-2xx errors, row
//! dedup, the limit clamp, and the city/country geography resolution.

use super::super::fetch_freehire;
use super::response_mapping::make_token;

/// Non-2xx surfaces as a prefixed `Err` at the `fetch_freehire` level, so the
/// failure stays observable and testable...
#[tokio::test]
async fn freehire_non_2xx_maps_to_prefixed_err() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_string("upstream down"))
        .mount(&server)
        .await;

    let msg = fetch_freehire(
        &server.uri(),
        "rust",
        None,
        Some("de"),
        None,
        None,
        make_token(),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        msg.starts_with("freehire:"),
        "the error must name the provider; got: {msg}"
    );
    assert!(
        msg.contains("503"),
        "the status must be carried; got: {msg}"
    );
}

// ...and the BOARD lets it through. As the aggregator's always-on keyless tier
// this module swallowed every fault to `Ok(empty)`, because nobody had opted
// into it and a third party's outage should not raise an error banner on a
// search the user never pointed at them. Selecting freehire in the catalog IS
// opting in, so that premise is gone and the failure is now the diagnostic the
// user needs — an empty result would read as "no such jobs" instead.
//
// The inversion lives in `FreehireScraper::search`, which propagates with `?`
// rather than matching the error away. That one-line contract is pinned by the
// `Err` above plus the absence of a swallow; the old
// `freehire_provider_degrades_silently_rather_than_failing_the_board` test was
// removed WITH the behaviour it protected rather than left asserting a rule the
// board no longer follows.

/// A row missing the fields a posting cannot exist without (title, url) is
/// DROPPED rather than mapped to a blank entry, and does not take the valid
/// rows in the same response with it.
#[tokio::test]
async fn freehire_drops_unusable_rows_without_losing_the_page() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[
                {"title":"No URL","company":"A"},
                {"url":"https://example.com/j/2","company":"B"},
                {"public_slug":"ok-1","title":"Good","url":"https://example.com/j/3"}
            ]}"#,
        ))
        .mount(&server)
        .await;

    let items = fetch_freehire(
        &server.uri(),
        "rust",
        None,
        Some("de"),
        None,
        None,
        make_token(),
    )
    .await
    .expect("a page with unusable rows must still map the usable ones");

    assert_eq!(items.len(), 1, "only the complete row maps");
    assert_eq!(items[0].title, "Good");
}

/// A slugless row keys off its URL, not a shared constant — otherwise every
/// slugless posting in a page would collapse into one under `dedupe`.
#[tokio::test]
async fn freehire_slugless_rows_key_off_their_url_not_each_other() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[
                {"title":"One","url":"https://example.com/j/1"},
                {"title":"Two","url":"https://example.com/j/2"}
            ]}"#,
        ))
        .mount(&server)
        .await;

    let items = fetch_freehire(
        &server.uri(),
        "rust",
        None,
        Some("de"),
        None,
        None,
        make_token(),
    )
    .await
    .expect("slugless rows must map");

    assert_eq!(items.len(), 2);
    // Concrete values, not just pairwise inequality. A review mutated the
    // fallback to a non-deterministic counter and this test STAYED GREEN:
    // pairwise inequality holds for any per-row-unique id, including one that
    // changes every run — which would break cross-run dedupe and resurface
    // every slugless posting as "new" on each re-scrape.
    assert_eq!(
        items[0].external_id.as_deref(),
        Some("freehire-https://example.com/j/1"),
        "a slugless row must key off its own URL, deterministically"
    );
    assert_eq!(
        items[1].external_id.as_deref(),
        Some("freehire-https://example.com/j/2")
    );
}

/// `limit` is clamped to the spec's 1..=100 range. Sending 0 or >100 is a 4xx
/// on the real API, not a silent clamp, so an out-of-range `amount` would turn
/// the whole tier off rather than just capping it.
#[tokio::test]
async fn freehire_clamps_limit_to_the_specs_range() {
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    fetch_freehire(
        &server.uri(),
        "rust",
        None,
        Some("de"),
        None,
        Some(5_000),
        make_token(),
    )
    .await
    .expect("an over-large amount must clamp, not fail");
}

/// The requested city reaches freehire as a `cities` filter — and does so
/// ALONE. This is the regression guard for the shipped defect: the board sent
/// no geography whatsoever, so a "Berlin" search fetched the worldwide first
/// page (Johannesburg, Bengaluru, Singapore) and the engine's post-filter
/// discarded nearly all of it.
///
/// The `absent("countries")` half is not decoration. Geography is one OR-GROUP
/// on this API, so `countries=de&cities=Berlin` reads "Germany OR Berlin" and
/// is WIDER than either alone — sending both would look like a tighter filter
/// while actually undoing the fix.
#[tokio::test]
async fn freehire_sends_the_resolved_city_alone() {
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    // The typeahead resolves the free text to freehire's canonical value. The
    // reply deliberately leads with a near miss to pin the exact-match rule.
    Mock::given(method("GET"))
        .and(path("/geo/cities"))
        .and(query_param("q", "Berlin"))
        .and(query_param("country", "de"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[{"value":"Berlin-Mitte","country":"de"},{"value":"Berlin","country":"de"}]}"#,
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/agent/jobs/search"))
        .and(query_param("cities", "Berlin"))
        .and(query_param_is_missing("countries"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    // The UI's location is a `"{city}, {country}"` label; only the city segment
    // may be sent — freehire splits a comma into two OR'd values.
    fetch_freehire(
        &server.uri(),
        "rust",
        Some("Berlin, Germany"),
        Some("de"),
        None,
        None,
        make_token(),
    )
    .await
    .expect("a located search must run");
}

/// A place freehire's dictionary does not know ("München" — the facet holds
/// "Munich" and matches nothing on a near miss) falls back to the country
/// filter rather than to no geography at all. A country-wide page is still
/// vastly narrower than the worldwide one the post-filter used to be handed.
#[tokio::test]
async fn freehire_falls_back_to_country_when_the_city_is_unknown() {
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/geo/cities"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/agent/jobs/search"))
        .and(query_param("countries", "de"))
        .and(query_param_is_missing("cities"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    fetch_freehire(
        &server.uri(),
        "rust",
        Some("München"),
        Some("de"),
        None,
        None,
        make_token(),
    )
    .await
    .expect("an unresolvable city must not fail the search");
}

/// A city that resolves only in the WRONG country is not used. "London" exists
/// in both `gb` and `ca` and the `cities` facet carries no country qualifier,
/// so the country check has to happen client-side — otherwise a Canadian
/// London search would silently filter on the British one.
#[tokio::test]
async fn freehire_rejects_a_city_match_from_another_country() {
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/geo/cities"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"data":[{"value":"London","country":"gb"}]}"#),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/agent/jobs/search"))
        .and(query_param("countries", "ca"))
        .and(query_param_is_missing("cities"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    fetch_freehire(
        &server.uri(),
        "rust",
        Some("London, Canada"),
        Some("ca"),
        None,
        None,
        make_token(),
    )
    .await
    .expect("a mismatched city must fall back, not filter on the wrong London");
}

/// A location-free search sends NEITHER geography parameter — the no-location
/// path stays byte-identical to what shipped, and the typeahead is not called.
#[tokio::test]
async fn freehire_sends_no_geography_without_a_location_or_country() {
    use wiremock::matchers::{method, path, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/geo/cities"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(0)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/agent/jobs/search"))
        .and(query_param_is_missing("cities"))
        .and(query_param_is_missing("countries"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    fetch_freehire(&server.uri(), "rust", None, None, None, None, make_token())
        .await
        .expect("an unlocated search must still run");
}

/// A typeahead outage degrades to the country filter instead of failing the
/// search: the resolver is an optimisation, never a dependency.
#[tokio::test]
async fn freehire_survives_a_city_typeahead_outage() {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/geo/cities"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/agent/jobs/search"))
        .and(query_param("countries", "de"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    fetch_freehire(
        &server.uri(),
        "rust",
        Some("Berlin"),
        Some("de"),
        None,
        None,
        make_token(),
    )
    .await
    .expect("a typeahead outage must not fail the search");
}
