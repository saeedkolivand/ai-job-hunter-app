//! `fetch_text`/`fetch_json` happy-path + basic-failure tests (mocked via
//! `wiremock`): success, 404, JSON schema-drift, non-2xx status, and
//! caller-header preservation.

use super::super::*;

#[tokio::test]
async fn test_fetch_text_success() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Hello World"))
        .mount(&mock_server)
        .await;

    let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_ok());
    let fetch_result = result.unwrap();
    assert_eq!(fetch_result.status_code, 200);
    assert_eq!(fetch_result.text, "Hello World");
}

#[tokio::test]
async fn test_fetch_text_404() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_ok());
    let fetch_result = result.unwrap();
    assert_eq!(fetch_result.status_code, 404);
}

#[tokio::test]
async fn test_fetch_json_success() {
    use serde::Deserialize;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Deserialize)]
    struct TestResponse {
        message: String,
    }

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"message":"test"}"#))
        .mount(&mock_server)
        .await;

    let parsed = fetch_json::<TestResponse>(&mock_server.uri(), FetchOptions::default(), signal)
        .await
        .expect("fetch_json should succeed and deserialize the response");
    assert_eq!(parsed.message, "test");
}

/// (b) A 2xx body that doesn't deserialize into the target type is a
/// representable schema-drift failure (`AppError::Parse`), not a silent empty
/// success — so a board sees the failure and can surface it.
#[tokio::test]
async fn test_fetch_json_invalid() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("invalid json"))
        .mount(&mock_server)
        .await;

    let result: crate::error::AppResult<serde_json::Value> =
        fetch_json(&mock_server.uri(), FetchOptions::default(), signal).await;
    match result {
        Err(crate::error::AppError::Parse(msg)) => {
            // Pin the exact static message — the serde detail (which would
            // quote fragments of the body) is logged separately and must
            // never reach the returned error, since that error crosses IPC
            // into `BoardScrapeSummary.error` → renderer.
            assert_eq!(
                msg, "response body did not match the expected schema",
                "Parse message must be the static no-leak string, not serde detail"
            );
            assert!(
                !msg.contains("invalid json"),
                "Parse message must not contain the mock response body: {msg:?}"
            );
        }
        other => panic!("expected a Parse error on schema drift, got {other:?}"),
    }
}

/// (a) A non-2xx response is a representable HTTP failure that carries the
/// status code (`AppError::Provider("HTTP <status>")`), never a silent empty
/// success — this is what makes a blocked/rotted board distinguishable from
/// "no jobs".
#[tokio::test]
async fn test_fetch_json_non_2xx_carries_status() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    // 403 is not 429/503, so the retry loop returns it immediately.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(403).set_body_string(r#"{"message":"blocked"}"#))
        .mount(&mock_server)
        .await;

    let result: crate::error::AppResult<serde_json::Value> =
        fetch_json(&mock_server.uri(), FetchOptions::default(), signal).await;
    match result {
        Err(crate::error::AppError::Provider(msg)) => {
            assert!(
                msg.contains("403"),
                "status code should be carried, got {msg:?}"
            );
        }
        other => panic!("expected a Provider error carrying the status, got {other:?}"),
    }
}

/// fetch_json must forward caller-supplied headers (e.g. X-API-Key) intact.
/// Previously it overwrote opts.headers entirely with just accept:application/json.
#[tokio::test]
async fn test_fetch_json_preserves_caller_headers() {
    use serde::Deserialize;
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Deserialize)]
    struct Resp {
        ok: bool,
    }

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    // Only respond when the custom auth header is present — proves the header was forwarded.
    Mock::given(method("GET"))
        .and(header("x-api-key", "secret"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .mount(&mock_server)
        .await;

    let result = fetch_json::<Resp>(
        &mock_server.uri(),
        FetchOptions {
            headers: Some(vec![("x-api-key".to_string(), "secret".to_string())]),
            ..Default::default()
        },
        signal,
    )
    .await;

    let parsed = result.expect("fetch_json should succeed and deserialize the response");
    assert!(parsed.ok, "expected ok:true — X-API-Key header was dropped");
}
