//! `web_search_transport` (wiremock against `crate::net::http::shared()`,
//! mirroring the pattern in `retry.rs`'s `retry_loop_tests`).

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::super::ProviderId;
use super::super::OpenAiClient;

#[tokio::test]
async fn web_search_transport_degrades_to_empty_on_http_500() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let text = client
        .web_search_transport("dummy-key", "gpt-4o", "system", "user")
        .await
        .expect("never an error, only degrades to empty");
    assert_eq!(text, "");
}

#[tokio::test]
async fn web_search_transport_degrades_to_empty_on_non_json_200() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let text = client
        .web_search_transport("dummy-key", "gpt-4o", "system", "user")
        .await
        .expect("never an error, only degrades to empty");
    assert_eq!(text, "");
}

#[tokio::test]
async fn web_search_transport_extracts_text_from_a_realistic_responses_payload() {
    let server = MockServer::start().await;
    let payload = json!({
        "output": [
            { "type": "web_search_call", "id": "ws_1", "status": "completed" },
            { "type": "message", "role": "assistant", "content": [
                { "type": "output_text", "text": "Acme is a ", "annotations": [] },
                { "type": "output_text", "text": "widget maker.", "annotations": [] }
            ]}
        ]
    });
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let text = client
        .web_search_transport("dummy-key", "gpt-4o", "system", "user")
        .await
        .expect("ok");
    assert_eq!(text, "Acme is a widget maker.");
}
