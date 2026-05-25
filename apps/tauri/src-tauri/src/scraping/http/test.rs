use super::*;

#[test]
fn test_fetch_options_default() {
    let opts = FetchOptions::default();
    assert!(opts.headers.is_none());
    assert!(opts.method.is_none());
    assert!(opts.body.is_none());
    assert_eq!(opts.retries, 1);
}

#[test]
fn test_strip_html_basic() {
    let html = "<p>Hello <b>World</b></p>";
    let result = strip_html(html);
    assert_eq!(result, "Hello World");
}

#[test]
fn test_strip_html_script() {
    let html = "<script>alert('xss')</script><p>Content</p>";
    let result = strip_html(html);
    assert_eq!(result, "Content");
}

#[test]
fn test_strip_html_style() {
    let html = "<style>body { color: red; }</style><p>Content</p>";
    let result = strip_html(html);
    assert_eq!(result, "Content");
}

#[test]
fn test_strip_html_entities() {
    let html = "Hello &amp; World &lt;3";
    let result = strip_html(html);
    assert_eq!(result, "Hello & World <3");
}

#[test]
fn test_strip_html_nbsp() {
    let html = "Hello&nbsp;&nbsp;World";
    let result = strip_html(html);
    assert_eq!(result, "Hello World");
}

#[test]
fn test_strip_html_quotes() {
    let html = "&quot;Test&quot; and &#39;quote&#39;";
    let result = strip_html(html);
    assert_eq!(result, "\"Test\" and 'quote'");
}

#[test]
fn test_strip_html_whitespace() {
    let html = "<p>Hello</p>   <p>World</p>";
    let result = strip_html(html);
    assert_eq!(result, "Hello World");
}

#[test]
fn test_strip_html_empty() {
    let html = "";
    let result = strip_html(html);
    assert_eq!(result, "");
}

#[test]
fn test_strip_html_nested_tags() {
    let html = "<div><span><b>Bold</b></span></div>";
    let result = strip_html(html);
    assert_eq!(result, "Bold");
}

#[tokio::test]
async fn test_fetch_text_success() {
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

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
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

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
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;
    use serde::Deserialize;

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

    let result = fetch_json::<TestResponse>(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[tokio::test]
async fn test_fetch_json_invalid() {
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("invalid json"))
        .mount(&mock_server)
        .await;

    let result: anyhow::Result<Option<serde_json::Value>> = fetch_json(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_fetch_text_cancelled() {
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();
    signal.cancel();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Hello"))
        .mount(&mock_server)
        .await;

    let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_err());
}
