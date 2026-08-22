//! Client tests for the freehire board.
//!
//! Moved verbatim from `aggregator/test.rs` when freehire stopped being the
//! aggregator's keyless tier and became a board of its own; the tests that
//! pinned its POSITION in `primary_chain` (last rung, skip-on-real-failure,
//! merge-behind-sparse-hits) went with that behaviour and are gone.

use super::{fetch_freehire, freehire_posted_within_days, freehire_user_agent};

/// A cancellation token that is never cancelled.
fn make_token() -> tokio_util::sync::CancellationToken {
    tokio_util::sync::CancellationToken::new()
}

/// A 2xx response round-trips through `fetch_freehire` into a `"freehire-"`
/// -prefixed posting, and the description is passed through UNCHANGED — the
/// request asks for `description_format=markdown`, so unlike Jooble's HTML
/// snippet there is nothing to convert, and converting anyway would mangle it.
#[tokio::test]
async fn freehire_ok_response_maps_end_to_end() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[{"public_slug":"senior-rust-acme-x1","title":"Senior Rust Engineer",
                "company":"Acme","location":"Munich, Bavaria","url":"https://apply.example.com/j/1",
                "description":"Our mission at **Acme** is to ship.","posted_at":"2026-08-08T02:25:32Z",
                "source":"workable","work_mode":"remote"}],"meta":{"total":1}}"#,
        ))
        .mount(&server)
        .await;

    let items = fetch_freehire(&server.uri(), "rust", Some("de"), None, None, make_token())
        .await
        .expect("a 2xx freehire response must map");

    assert_eq!(items.len(), 1);
    let job = &items[0];
    assert_eq!(
        job.external_id.as_deref(),
        Some("freehire-senior-rust-acme-x1")
    );
    assert_eq!(job.id, "aggregator:freehire-senior-rust-acme-x1");
    assert_eq!(job.title, "Senior Rust Engineer");
    assert_eq!(job.company, "Acme");
    assert_eq!(job.source, "aggregator");
    assert_eq!(
        job.description.as_deref(),
        Some("Our mission at **Acme** is to ship."),
        "markdown was requested AND html_to_markdown early-returns tag-free input \
         verbatim, so real markdown must survive the defensive pass unescaped"
    );
    assert!(job.posted_at.is_some(), "an RFC3339 posted_at must parse");
    assert_eq!(
        job.extra.get("aggregatorSource").and_then(|v| v.as_str()),
        Some("workable"),
        "freehire's own upstream must be carried so a posting is not attributed \
         to freehire itself"
    );
}

/// The request is built from the PUBLISHED spec: the documented
/// `/agent/jobs/search` path, `q` as the full-text parameter (NOT the
/// undocumented `/jobs/search`'s `query`), `description_format=markdown` so
/// scoring never needs a per-result detail fetch, and `reality=fresh` (issue
/// #1026's quality filter — see `fetch_freehire`'s doc for why it is
/// unconditional).
///
/// Mutation check: changed `q=` to `query=` in `fetch_freehire` — RAN, went red
/// here, restored. Same for dropping `description_format`.
#[tokio::test]
async fn freehire_request_follows_the_published_spec() {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/agent/jobs/search"))
        .and(query_param("q", "rust engineer"))
        .and(query_param("description_format", "markdown"))
        .and(query_param("countries", "de"))
        .and(query_param("reality", "fresh"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    fetch_freehire(
        &server.uri(),
        "rust engineer",
        Some("de"),
        None,
        None,
        make_token(),
    )
    .await
    .expect("the spec-shaped request must succeed");
    // MockServer verifies `.expect(1)` on drop: a request that missed any of the
    // matchers above leaves it unsatisfied and panics here.
}

/// The identifying `User-Agent` (issue #1026) is sent, and it is per-request —
/// NOT the shared client's browser-shaped default (`net::http::DEFAULT_UA`,
/// still used everywhere else in the fleet). Regression guard for the exact
/// bug `FetchOptions::user_agent` exists to avoid: `headers` entries are
/// applied via `RequestBuilder::header`, which APPENDS, so putting
/// `user-agent` there instead would have sent it ALONGSIDE the default rather
/// than in place of it.
///
/// Mutation check: reverted `fetch_freehire`'s `user_agent: Some(...)` back to
/// the field's `None` default — RAN, went red (wiremock's `header` matcher no
/// longer saw the expected value, since the request fell back to
/// `DEFAULT_UA`), restored.
#[tokio::test]
async fn freehire_sends_the_identifying_user_agent_in_place_of_the_default() {
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("user-agent", freehire_user_agent().as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    fetch_freehire(&server.uri(), "rust", Some("de"), None, None, make_token())
        .await
        .expect("the identifying-UA request must succeed");
}

/// `freehire_user_agent` carries only the app name, the crate version, and the
/// public repo URL — nothing that could identify a specific user or machine.
#[test]
fn freehire_user_agent_carries_no_user_data() {
    let ua = freehire_user_agent();
    assert!(
        ua.starts_with("ai-job-hunter/"),
        "must lead with the app name; got {ua:?}"
    );
    assert!(
        ua.contains(env!("CARGO_PKG_VERSION")),
        "must carry the real crate version, not a hand-typed one; got {ua:?}"
    );
    assert!(
        ua.contains("github.com/saeedkolivand/ai-job-hunter-app"),
        "must carry the public repo URL; got {ua:?}"
    );
}

/// `posted_within_days` mapping: every generated `date_filter` token maps to a
/// real value (never silently dropped), `None` omits the parameter entirely
/// (freehire's own "no restriction" semantics), and the sub-day tokens share
/// the same 3-day floor as `adzuna_max_days_old`/`jsearch_date_posted` rather
/// than a newly-invented number.
///
/// Mutation check: changed the sub-day arm's `Some(3)` to `Some(1)` — RAN,
/// went red here, restored.
#[test]
fn freehire_posted_within_days_maps_every_generated_token() {
    assert_eq!(freehire_posted_within_days(None), None);
    for token in ["15m", "30m", "1h", "2h", "4h", "8h", "24h"] {
        assert_eq!(
            freehire_posted_within_days(Some(token)),
            Some(3),
            "sub-day token {token:?} must floor at 3 days, matching Adzuna/JSearch"
        );
    }
    assert_eq!(freehire_posted_within_days(Some("week")), Some(7));
    assert_eq!(freehire_posted_within_days(Some("month")), Some(30));

    for &token in crate::ipc_contracts::date_filters::DATE_FILTER_OPTIONS {
        assert!(
            freehire_posted_within_days(Some(token)).is_some(),
            "generated date-filter token {token:?} has no freehire mapping"
        );
    }
}

/// `date_filter` reaches the wire as `posted_within_days`, the real bug issue
/// #1026 reported (previously `_date_filter: Option<&str>` — accepted and
/// thrown away).
#[tokio::test]
async fn freehire_wires_date_filter_to_posted_within_days() {
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(query_param("posted_within_days", "7"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    fetch_freehire(
        &server.uri(),
        "rust",
        Some("de"),
        Some("week"),
        None,
        make_token(),
    )
    .await
    .expect("a date-filtered request must succeed");
}

/// A response the maintainer's `ignored_params` guard reports is treated as a
/// FAILURE, not a warning that still returns `data` — see `fetch_freehire`'s
/// doc for why. The one behavior under test that must NOT happen: getting
/// `Ok` back with the (unfiltered) job the fixture's `data` array carries.
///
/// Mutation check: dropped the `!resp.meta.ignored_params.is_empty()` guard —
/// RAN, went red here (the call returned `Ok` with the one fixture job),
/// restored.
#[tokio::test]
async fn freehire_refuses_a_response_with_ignored_params() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[{"title":"Should Not Surface","url":"https://example.com/j/1"}],
                "meta":{"total":1,"ignored_params":[{"param":"country","did_you_mean":"countries"}]}}"#,
        ))
        .mount(&server)
        .await;

    let err = fetch_freehire(&server.uri(), "rust", Some("de"), None, None, make_token())
        .await
        .expect_err("a response reporting an ignored param must not be treated as filtered");
    let msg = err.to_string();
    assert!(
        msg.contains("ignored"),
        "the error must say why it refused; got: {msg}"
    );
    assert!(
        msg.contains("countries"),
        "the ignored param's did_you_mean detail must reach the log-visible error; got: {msg}"
    );
}

/// The companion half of the guard above: a CLEAN response (no `meta` block,
/// or a `meta` with an empty/absent `ignored_params`) must map normally — the
/// guard must not become a false-positive that drops every result.
#[tokio::test]
async fn freehire_maps_normally_when_no_params_are_ignored() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[{"title":"Fine","url":"https://example.com/j/1"}],"meta":{"total":1}}"#,
        ))
        .mount(&server)
        .await;

    let items = fetch_freehire(&server.uri(), "rust", Some("de"), None, None, make_token())
        .await
        .expect("a clean response (no ignored_params) must map normally");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Fine");
}

/// `None` sends NO `countries` filter at all — which is what the board does on
/// every search, since `supports_location` is `false` and nothing derives a
/// country from the free-text location.
///
/// This is the shape that matters: a country the caller did not actually choose
/// must never silently narrow the search to one market. It is the guessed-market
/// bug already fixed for Adzuna, and it was live here too while freehire was the
/// aggregator's tier and inherited that board's `"de"` default.
///
/// Mutation check: made the `Option` unconditionally emit `&countries=` — RAN,
/// went red here, restored.
#[tokio::test]
async fn freehire_sends_no_country_filter_when_none_was_chosen() {
    use wiremock::matchers::{method, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(query_param_is_missing("countries"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
        .expect(1)
        .mount(&server)
        .await;

    fetch_freehire(&server.uri(), "rust", None, None, None, make_token())
        .await
        .expect("a guessed-country search must still run");
}

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

    let msg = fetch_freehire(&server.uri(), "rust", Some("de"), None, None, make_token())
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

    let items = fetch_freehire(&server.uri(), "rust", Some("de"), None, None, make_token())
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

    let items = fetch_freehire(&server.uri(), "rust", Some("de"), None, None, make_token())
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
        Some("de"),
        None,
        Some(5_000),
        make_token(),
    )
    .await
    .expect("an over-large amount must clamp, not fail");
}
