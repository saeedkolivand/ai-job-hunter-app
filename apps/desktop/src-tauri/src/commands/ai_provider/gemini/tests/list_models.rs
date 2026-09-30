//! `list_models_transport`'s HTTP loop (wiremock — pagination, error
//! propagation, cumulative deadline).

use std::time::Duration;

use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::GeminiClient;
use crate::error::AppError;

use super::support::slow_body_server;

#[tokio::test]
async fn list_models_transport_propagates_the_cursor_into_the_next_requests_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-page1-token",
        })))
        .mount(&server)
        .await;
    // Only matches when `pageToken` carries EXACTLY page 1's
    // `nextPageToken` — proves the token is wired from the parsed response
    // into the next request's query, not just decided in the abstract by
    // `pagination_step`.
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-page1-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-2.5-flash" }],
        })))
        .mount(&server)
        .await;

    let models = GeminiClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect("both pages must be fetched, the token propagated between them");
    assert_eq!(
        models,
        vec![
            json!({ "name": "gemini-3-pro" }),
            json!({ "name": "gemini-2.5-flash" }),
        ]
    );
}

#[tokio::test]
async fn list_models_transport_propagates_a_mid_pagination_status_error_instead_of_the_pages_already_collected(
) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-page1-token",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-page1-token"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = GeminiClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err(
            "a failure on page 2 must reject the whole fetch, never return page 1's models alone",
        );
    // 500 classifies as Network via `friendly_api_error` — not the point of
    // this test, but asserted so a future change that silently swallows
    // page 2's error can't slip through.
    assert!(matches!(err, AppError::Network(_)));
}

#[tokio::test]
async fn list_models_transport_errors_when_page_two_is_malformed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-page1-token",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-page1-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "unexpected": "shape" })))
        .mount(&server)
        .await;

    let err = GeminiClient
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
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-page1-token",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-page1-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
        .mount(&server)
        .await;

    let err = GeminiClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a non-JSON page 2 body must reject the whole fetch");
    assert!(matches!(err, AppError::Provider(_)));
    assert_eq!(
        err.to_string(),
        "gemini: parse: response body did not match the expected schema"
    );
}

#[tokio::test]
async fn list_models_transport_errors_when_the_provider_reports_a_stalled_token() {
    // A `nextPageToken` present but identical across two pages: the
    // provider claims more data exists but gives no way to reach it. The
    // exact regression this whole finding is about — silently treating this
    // as `Done` and returning the one page collected as `Ok` — must not
    // happen at the transport level either, not just in the pure
    // `pagination_step` unit tests.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-stuck",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-stuck"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-stuck",
        })))
        .mount(&server)
        .await;

    let err = GeminiClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a stalled token must reject, never silently return the partial catalogue");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_errors_when_the_cumulative_deadline_fires_across_multiple_pages() {
    // Page 1 responds instantly (well within budget) and reports a token.
    // Page 2 writes its HEADERS instantly too — `send()` resolves fast — but
    // stalls its BODY well past the REMAINING budget. A deadline that only
    // wraps `send()` (the pre-fix bug) would let this succeed late instead
    // of erroring; only wrapping the body reads too catches it. Verified
    // (before applying that fix) that this test genuinely fails against the
    // unfixed transport — it doesn't error at all, it just returns `Ok`
    // after the full body delay, which is the exact silent-non-enforcement
    // bug this test exists to catch.
    let page1 = json!({
        "models": [{ "name": "models/gemini-3-pro" }],
        "nextPageToken": "gemini-page1-token",
    })
    .to_string();
    let page2 = json!({ "models": [{ "name": "models/gemini-2.5-flash" }] }).to_string();
    let base = slow_body_server(vec![
        (page1, Duration::ZERO),
        (page2, Duration::from_millis(100)),
    ])
    .await;

    let err = GeminiClient
        .list_models_transport(&base, "dummy-key", Duration::from_millis(40))
        .await
        .expect_err("page 2's body delay must exceed the REMAINING cumulative budget after page 1");
    assert!(matches!(err, AppError::Network(_)));
}
