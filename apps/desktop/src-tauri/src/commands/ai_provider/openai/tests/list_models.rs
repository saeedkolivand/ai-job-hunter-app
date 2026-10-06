//! `list_models_transport`'s HTTP behaviour (wiremock — status
//! classification, body-shape errors, key/no-key auth header handling).

use serde_json::{json, Value};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::super::ProviderId;
use super::super::OpenAiClient;
use crate::error::AppError;

#[tokio::test]
async fn list_models_transport_errors_on_http_500() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect_err("a non-success status must reject, never silently degrade to empty");
    // Routed through `friendly_api_error`, not a bare `AppError::Provider` —
    // 500 classifies as `Network` (retriable), distinct from a 401 (`Config`,
    // bad key) or a 429 (`Network`, rate limit). This IS the PR's premise:
    // once the curated fallback is gone, this classification is the user's
    // only explanation.
    assert!(matches!(err, AppError::Network(_)));
}

#[tokio::test]
async fn list_models_transport_classifies_401_as_a_config_error_not_a_bare_provider_error() {
    // `test_key`'s status branch (not independently testable here — it needs
    // a live `AppHandle` this crate has no mock harness for) constructs its
    // error via the identical `friendly_api_error(self.id, status,
    // &body_text)` call on the same `list_models_request` transport, so this
    // covers both by construction.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect_err("a 401 must reject");
    assert!(matches!(err, AppError::Config(_)));
}

#[tokio::test]
async fn list_models_transport_ok_with_the_native_openai_filter_applied() {
    let server = MockServer::start().await;
    let payload = json!({
        "data": [{ "id": "gpt-4o" }, { "id": "text-embedding-3-small" }]
    });
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let models = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect("ok");
    assert_eq!(models, vec![json!({ "name": "gpt-4o" })]);
}

#[tokio::test]
async fn list_models_transport_ok_empty_on_a_genuinely_empty_catalogue() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let models = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect("ok");
    assert_eq!(models, Vec::<Value>::new());
}

#[tokio::test]
async fn list_models_transport_errors_on_a_non_json_200_body() {
    // The captive-portal case: a 200 whose body is HTML, not JSON — a
    // self-hosted gateway behind a broken proxy/auth wall can return this.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>captive portal</html>"))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect_err("a non-JSON 200 body must reject, never silently degrade to empty");
    assert!(matches!(err, AppError::Provider(_)));
}

/// Proves `list_models_transport` actually routes its 200-body read through
/// `crate::net::http::read_json_capped` (fix for the "44 unbounded reads in
/// the AI adapters" hardening pass) rather than a bare `resp.json()`: a body
/// over `DEFAULT_MAX_BODY_BYTES` (8 MB) must reject, not buffer unbounded
/// into memory. wiremock auto-computes a real `Content-Length` for
/// `set_body_string`, so this exercises the cheap pre-check the same way
/// `net::http`'s own `read_text_capped_returns_the_body_under_the_cap`
/// sibling test proves the streaming guard — see that module for the
/// `Transfer-Encoding: chunked` variant that forces the streaming path
/// instead.
#[tokio::test]
async fn list_models_transport_rejects_a_response_over_the_body_cap() {
    let server = MockServer::start().await;
    let oversized = "x".repeat(crate::net::http::DEFAULT_MAX_BODY_BYTES + 1);
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_string(oversized))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect_err("a body over the cap must reject, never buffer unbounded");
    assert!(
        format!("{err}").to_lowercase().contains("too large"),
        "expected a size-cap error, got: {err}"
    );
}

#[tokio::test]
async fn list_models_transport_errors_on_a_bare_array_200_body() {
    // Well-formed JSON, but not the `{ "data": [...] }` envelope — a deployment
    // that returns the array directly rather than wrapping it.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{ "id": "gpt-4o" }])))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect_err("a bare-array body must reject, not silently return an empty list");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_succeeds_with_no_key_for_a_keyless_deployment() {
    // A keyless `OpenAiCompatible` deployment (LM Studio, vLLM, …) must still
    // be able to list — and must never send an empty `Authorization: Bearer`
    // header (some gateways reject a malformed header rather than ignoring it).
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(|req: &wiremock::Request| !req.headers.contains_key("authorization"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": [{ "id": "local-model" }] })),
        )
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAiCompatible, Some(server.uri()));
    let models = client
        .list_models_transport(None)
        .await
        .expect("a keyless request must still reach the mock (no Authorization header required)");
    assert_eq!(models, vec![json!({ "name": "local-model" })]);
}

#[tokio::test]
async fn list_models_transport_sends_the_bearer_header_when_a_key_is_present() {
    // The `Some(key)` sibling of the keyless test above — every other
    // `list_models_transport` mock in this file never REQUIRES an
    // `authorization` header, so a regression that dropped `bearer_auth`
    // entirely (e.g. accidentally hardcoding `None`) would keep every one of
    // them green. This one requires the header to be present with the exact
    // value, closing that gap.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("authorization", "Bearer dummy-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": [{ "id": "gpt-4o" }] })),
        )
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let models = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect("a keyed request must send the bearer header the mock requires");
    assert_eq!(models, vec![json!({ "name": "gpt-4o" })]);
}

#[tokio::test]
async fn a_key_echoed_in_an_upstream_error_body_never_reaches_the_wire_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": { "message": "bad credential AIzaSyTESTKEYabcdefghijklmnop" }
        })))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("AIzaSyTESTKEYabcdefghijklmnop"))
        .await
        .expect_err("a 400 must reject");
    assert!(err.to_string().contains("AIzaSyTESTKEYabcdefghijklmnop"));
    let wire = crate::commands::ai_provider::finish_provider_result::<()>(
        Err(err),
        Some("AIzaSyTESTKEYabcdefghijklmnop"),
        None,
    )
    .unwrap_err()
    .to_string();
    assert!(
        !wire.contains("TESTKEY"),
        "key reached the wire error: {wire}"
    );
}
