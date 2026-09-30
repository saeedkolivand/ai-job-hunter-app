//! `endpoint_url`/`scrub_url_secret` — query-string-auth gateway safety.
//!
//! Some OpenAI-compatible gateways (Cloudflare AI Gateway, several self-hosted
//! proxies) authenticate via the base URL's own query string, e.g.
//! `https://gw.example.com/v1?api-key=SECRET`. Plain string concatenation
//! (`format!("{base}/{path}")`) sends that case to the WRONG path (the query
//! value gets `/path` appended to it) and, because reqwest's own error
//! `Display` embeds the request URL verbatim, echoes the secret into any
//! error text built from it.

use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::super::ProviderId;
use super::super::transport::scrub_url_secret;
use super::super::OpenAiClient;
use crate::error::AppError;

#[test]
fn endpoint_url_preserves_a_query_carrying_base_url() {
    let client = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("https://gw.example.com/v1?api-key=SECRET123".to_string()),
    );
    let url = client.endpoint_url("models").unwrap();
    assert_eq!(
        url.as_str(),
        "https://gw.example.com/v1/models?api-key=SECRET123"
    );
}

#[test]
fn endpoint_url_resolves_the_same_path_regardless_of_a_trailing_slash_on_the_base() {
    let no_slash = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("https://gw.example.com/v1?api-key=SECRET123".to_string()),
    )
    .endpoint_url("models")
    .unwrap();
    let with_slash = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("https://gw.example.com/v1/?api-key=SECRET123".to_string()),
    )
    .endpoint_url("models")
    .unwrap();
    assert_eq!(no_slash, with_slash);
}

#[test]
fn endpoint_url_joins_a_multi_segment_path() {
    let client = OpenAiClient::new(ProviderId::OpenAi, None);
    let url = client.endpoint_url("chat/completions").unwrap();
    assert_eq!(url.as_str(), "https://api.openai.com/v1/chat/completions");
}

#[test]
fn endpoint_url_preserves_a_fragment() {
    let client = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("https://gw.example.com/v1#frag".to_string()),
    );
    let url = client.endpoint_url("models").unwrap();
    assert_eq!(url.as_str(), "https://gw.example.com/v1/models#frag");
}

#[test]
fn endpoint_url_errors_on_an_unparseable_base_url() {
    let client = OpenAiClient::new(ProviderId::OpenAiCompatible, Some("not a url".to_string()));
    assert!(matches!(
        client.endpoint_url("models"),
        Err(AppError::Config(_))
    ));
}

#[tokio::test]
async fn scrub_url_secret_removes_the_query_but_keeps_scheme_host_path() {
    // A genuine transport failure (not a synthetic error — `reqwest::Error`
    // has no public constructor) against an unreachable loopback port, so the
    // resulting error carries a real, reqwest-attached URL.
    // Built through `net::http::shared()`, not `reqwest::Client::new()` — R5
    // confines client construction to `net/http.rs`, and this test is more
    // faithful for it: the error being scrubbed comes from the same client
    // the production path uses.
    let url = reqwest::Url::parse("http://127.0.0.1:1/v1/models?api-key=SECRET123").unwrap();
    let err = crate::net::http::shared()
        .get(url)
        .timeout(std::time::Duration::from_millis(500))
        .send()
        .await
        .expect_err("port 1 must be unreachable");
    let text = scrub_url_secret(err).to_string();
    assert!(
        !text.contains("SECRET123"),
        "query secret must not survive scrubbing: {text}"
    );
    assert!(
        text.contains("127.0.0.1:1/v1/models"),
        "scheme/host/path must stay visible for diagnosing a wrong-path bug: {text}"
    );
}

#[tokio::test]
async fn list_models_transport_reaches_the_correct_path_with_a_query_carrying_base_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(query_param("api-key", "SECRET123"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": [{ "id": "gw-model" }] })),
        )
        .mount(&server)
        .await;

    let base = format!("{}/v1?api-key=SECRET123", server.uri());
    let client = OpenAiClient::new(ProviderId::OpenAiCompatible, Some(base));
    let models = client
        .list_models_transport(None)
        .await
        .expect("a query-carrying base URL must resolve to the correct path with the query intact");
    assert_eq!(models, vec![json!({ "name": "gw-model" })]);
}

#[tokio::test]
async fn list_models_transport_never_leaks_the_query_secret_on_a_network_failure() {
    let client = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("http://127.0.0.1:1/v1?api-key=SECRET123".to_string()),
    );
    let err = client
        .list_models_transport(None)
        .await
        .expect_err("port 1 must be unreachable");
    let text = err.to_string();
    assert!(
        !text.contains("SECRET123"),
        "the resolved-but-unreachable query-carrying URL must not leak its secret: {text}"
    );
}
