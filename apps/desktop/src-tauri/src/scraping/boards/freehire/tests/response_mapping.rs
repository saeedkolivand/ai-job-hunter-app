//! `fetch_freehire` tests: response mapping, request shape, the identifying
//! user-agent, and date-filter wiring. `request_shape.rs` is the split-out
//! sibling: the ignored-params/limit-clamp/city-geography tests.

use super::super::{fetch_freehire, freehire_posted_within_days, freehire_user_agent};

/// A cancellation token that is never cancelled.
pub(super) fn make_token() -> tokio_util::sync::CancellationToken {
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
    assert_eq!(
        job.extra.get("workType").and_then(|v| v.as_str()),
        Some("remote"),
        "work_mode must map to extra.workType (renamed from the unread workMode key)"
    );
}

/// `work_mode` truth table via the same end-to-end path: hybrid and onsite
/// round-trip, and an absent field writes nothing (Unknown), never a guessed
/// value.
#[tokio::test]
async fn freehire_work_mode_maps_every_declared_value_and_absence() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[
                {"public_slug":"hybrid-1","title":"Hybrid Engineer","company":"Acme",
                    "url":"https://apply.example.com/j/2","work_mode":"hybrid"},
                {"public_slug":"onsite-1","title":"Onsite Engineer","company":"Acme",
                    "url":"https://apply.example.com/j/3","work_mode":"onsite"},
                {"public_slug":"absent-1","title":"Undeclared Engineer","company":"Acme",
                    "url":"https://apply.example.com/j/4"}
            ],"meta":{"total":3}}"#,
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
    .expect("a 2xx freehire response must map");

    let work_type = |slug_suffix: &str| -> Option<String> {
        items
            .iter()
            .find(|p| p.external_id.as_deref() == Some(&format!("freehire-{slug_suffix}")))
            .and_then(|p| p.extra.get("workType"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    assert_eq!(work_type("hybrid-1"), Some("hybrid".to_string()));
    assert_eq!(
        work_type("onsite-1"),
        Some("on-site".to_string()),
        "freehire's no-hyphen 'onsite' spelling must still normalize"
    );
    assert_eq!(
        work_type("absent-1"),
        None,
        "an absent work_mode must write nothing, not a guessed value"
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
        None,
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

    fetch_freehire(
        &server.uri(),
        "rust",
        None,
        Some("de"),
        None,
        None,
        make_token(),
    )
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
        None,
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

    let err = fetch_freehire(
        &server.uri(),
        "rust",
        None,
        Some("de"),
        None,
        None,
        make_token(),
    )
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

    fetch_freehire(&server.uri(), "rust", None, None, None, None, make_token())
        .await
        .expect("a guessed-country search must still run");
}
