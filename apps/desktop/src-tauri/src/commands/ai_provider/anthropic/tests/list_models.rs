//! `list_models_transport`'s HTTP loop (wiremock — pagination, error
//! propagation, cumulative deadline) plus the `reqwest::Error::is_timeout`
//! classification every adapter's transport-error mapping relies on.

use std::time::Duration;

use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::super::AppError;
use super::super::AnthropicClient;
use super::support::{refused_connection_error, slow_body_server, wholly_slow_server};

#[tokio::test]
async fn list_models_transport_propagates_the_cursor_into_the_next_requests_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-page1-last",
        })))
        .mount(&server)
        .await;
    // Only matches when `after_id` carries EXACTLY page 1's `last_id` —
    // proves the cursor is wired from the parsed response into the next
    // request's query, not just decided in the abstract by `pagination_step`.
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-page1-last"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-opus-5" }],
            "has_more": false,
        })))
        .mount(&server)
        .await;

    let models = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect("both pages must be fetched, the cursor propagated between them");
    assert_eq!(
        models,
        vec![
            json!({ "name": "claude-sonnet-5" }),
            json!({ "name": "claude-opus-5" }),
        ]
    );
}

#[tokio::test]
async fn list_models_transport_propagates_a_mid_pagination_status_error_instead_of_the_pages_already_collected(
) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-page1-last",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-page1-last"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err(
            "a failure on page 2 must reject the whole fetch, never return page 1's models alone",
        );
    // 500 classifies as Network via `friendly_api_error` — not the point of
    // this test (see the classification tests), but asserted so a future
    // change that silently swallows page 2's error can't slip through.
    assert!(matches!(err, AppError::Network(_)));
}

#[tokio::test]
async fn list_models_transport_errors_when_page_two_is_malformed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-page1-last",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-page1-last"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "unexpected": "shape" })))
        .mount(&server)
        .await;

    let err = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a malformed page 2 body must reject the whole fetch");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_preserves_provider_context_on_a_non_json_page_two_body() {
    // Regression pin: the `.await??` rewrite in 70738c63 dropped the
    // "{name}: parse: " context and flipped this from `Provider` to `Parse`
    // (`error.rs`'s `From<serde_json::Error>`) — a change to the renderer-
    // visible error the sweep never intended. Unlike the "malformed shape"
    // test above (valid JSON, wrong fields — caught by `parse_model_page`),
    // this body isn't JSON at all, so it fails inside `read_json_capped`
    // itself, exercising the exact line that regressed.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-page1-last",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-page1-last"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
        .mount(&server)
        .await;

    let err = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a non-JSON page 2 body must reject the whole fetch");
    assert!(matches!(err, AppError::Provider(_)));
    assert_eq!(
        err.to_string(),
        "anthropic: parse: response body did not match the expected schema"
    );
}

#[tokio::test]
async fn list_models_transport_errors_when_the_provider_reports_a_stalled_cursor() {
    // `has_more: true` with the SAME `last_id` twice: the provider claims
    // more data exists but gives no way to reach it. The exact regression
    // this whole finding is about — silently treating this as `Done` and
    // returning the one page collected as `Ok` — must not happen at the
    // transport level either, not just in the pure `pagination_step` unit
    // tests.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param_is_missing("after_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-stuck",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("after_id", "claude-stuck"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "claude-sonnet-5" }],
            "has_more": true,
            "last_id": "claude-stuck",
        })))
        .mount(&server)
        .await;

    let err = AnthropicClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a stalled cursor must reject, never silently return the partial catalogue");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_errors_when_the_cumulative_deadline_fires_across_multiple_pages() {
    // Page 1 responds instantly (well within budget) and reports a cursor.
    // Page 2 writes its HEADERS instantly too — `send()` resolves fast — but
    // stalls its BODY well past the REMAINING budget. A deadline that only
    // wraps `send()` (the pre-fix bug) would let this succeed late instead
    // of erroring; only wrapping the body reads too catches it. Verified
    // (before applying that fix) that this test genuinely fails against the
    // unfixed transport — it doesn't error at all, it just returns `Ok`
    // after the full body delay, which is the exact silent-non-enforcement
    // bug this test exists to catch.
    let page1 = json!({
        "data": [{ "id": "claude-sonnet-5" }],
        "has_more": true,
        "last_id": "claude-page1-last",
    })
    .to_string();
    let page2 = json!({
        "data": [{ "id": "claude-opus-5" }],
        "has_more": false,
    })
    .to_string();
    let base = slow_body_server(vec![
        (page1, Duration::ZERO),
        (page2, Duration::from_millis(100)),
    ])
    .await;

    let err = AnthropicClient
        .list_models_transport(&base, "dummy-key", Duration::from_millis(40))
        .await
        .expect_err("page 2's body delay must exceed the REMAINING cumulative budget after page 1");
    assert!(matches!(err, AppError::Network(_)));
}

// ── `reqwest::Error::is_timeout` — the assumption every `complete_impl` now relies on ──
//
// `AppError::Timeout` (see `complete_impl`'s `send_with_retry` failure branch,
// and its siblings in `openai.rs`/`gemini.rs`/`ollama.rs`) depends on
// `is_timeout()` firing whenever a call gave up waiting on ITS OWN `.timeout()`
// deadline — which, per reqwest's own source, is checked by walking the whole
// error chain (its `TimedOut` marker, an inner `hyper::Error::is_timeout()`,
// or a raw `io::ErrorKind::TimedOut`), not by matching one specific error
// shape. It must NOT also fire for a connection this process itself refused —
// that is a different failure (nothing was ever waited on) and would
// misreport an actively-down host as "try a faster model". `reqwest::Error`
// has no public constructor, so this pins both halves of that boundary
// against REAL errors rather than trusting the library's docs. One place for
// all four adapters: the classification is a `reqwest` fact, not an
// Anthropic-specific one.

#[tokio::test]
async fn reqwest_is_timeout_fires_for_the_clients_own_deadline_and_never_for_a_connect_failure() {
    let base = wholly_slow_server(Duration::from_millis(200)).await;
    let timed_out = crate::net::http::shared()
        .get(&base)
        .timeout(Duration::from_millis(50))
        .send()
        .await
        .expect_err("a 50ms deadline against a 200ms-silent server must fail");
    assert!(timed_out.is_timeout(), "{timed_out}");

    // A REFUSED connection is a DIFFERENT `reqwest::Error` shape —
    // `is_timeout()` must not also fire for it, or every actively-refused
    // provider host would misreport as "try a faster model". A well-known
    // port like 1 is NOT a reliable negative case (some environments
    // firewall/blackhole it instead of sending RST, which is
    // indistinguishable from a timeout — correctly so, since that IS what a
    // client-side deadline is for). A loopback port this process just bound
    // and then dropped is: the kernel's own TCP stack refuses it immediately,
    // no firewall or DNS involved, hermetic and portable across OSes.
    let refused = refused_connection_error().await;
    assert!(!refused.is_timeout(), "{refused}");
}
