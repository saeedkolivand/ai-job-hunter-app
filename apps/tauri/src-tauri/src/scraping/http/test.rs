use super::*;

#[test]
fn test_fetch_options_default() {
    let opts = FetchOptions::default();
    assert!(opts.headers.is_none());
    assert!(opts.method.is_none());
    assert!(opts.body.is_none());
    // Default retries raised to 2 to allow one 429/503 backoff before giving up.
    assert_eq!(opts.retries, 2);
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

#[test]
fn test_html_to_text_preserves_structure() {
    let html = "<p>Intro paragraph.</p><p>Responsibilities:</p><ul><li>Build features</li><li>Write tests</li></ul>";
    let result = html_to_text(html);
    assert_eq!(
        result,
        "Intro paragraph.\nResponsibilities:\n• Build features\n• Write tests"
    );
}

#[test]
fn test_html_to_text_br_and_entities() {
    let html = "Line one<br>Line two &amp; more";
    let result = html_to_text(html);
    assert_eq!(result, "Line one\nLine two & more");
}

#[test]
fn test_html_to_text_caps_blank_lines() {
    let html = "<div>A</div><div></div><div></div><div>B</div>";
    let result = html_to_text(html);
    assert_eq!(result, "A\n\nB");
}

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

    let result =
        fetch_json::<TestResponse>(&mock_server.uri(), FetchOptions::default(), signal).await;
    let parsed = result
        .expect("fetch_json should succeed")
        .expect("fetch_json should deserialize the response");
    assert_eq!(parsed.message, "test");
}

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

    let result: crate::error::AppResult<Option<serde_json::Value>> =
        fetch_json(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
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

    let parsed = result
        .expect("fetch_json should succeed")
        .expect("response should be deserialized");
    assert!(parsed.ok, "expected ok:true — X-API-Key header was dropped");
}

/// fetch_text must abort the stream before fully buffering when the body exceeds
/// the cap, even when no Content-Length is declared by the server.
#[tokio::test]
async fn test_fetch_text_stream_cap_no_content_length() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    // 100 bytes body, cap set to 50 — no Content-Length header from wiremock by default.
    let body = "x".repeat(100);
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock_server)
        .await;

    let result = fetch_text(
        &mock_server.uri(),
        FetchOptions {
            max_bytes: Some(50),
            ..Default::default()
        },
        signal,
    )
    .await;

    assert!(
        result.is_err(),
        "should have returned an error for oversized body"
    );
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("too large"),
        "expected 'too large' error, got: {err}"
    );
}

/// fetch_text must correctly decode non-ASCII bytes (UTF-8: German umlauts, €).
#[tokio::test]
async fn test_fetch_text_utf8_decode() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    let body = "Softwareentwickler (m/w/d) – Gehalt: 80.000 € · München";
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/plain; charset=utf-8")
                .set_body_string(body),
        )
        .mount(&mock_server)
        .await;

    let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().text, body);
}

/// A response whose Content-Type declares charset=iso-8859-1 and whose body
/// contains raw ISO-8859-1 bytes must decode to the correct Unicode string.
/// 0xFC is ü in ISO-8859-1.
#[tokio::test]
async fn test_fetch_text_iso8859_decode() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    // Raw ISO-8859-1: "Schl" + 0xFC + "ssel" = "Schlüssel"
    let body_bytes: Vec<u8> = b"Schl\xFCssel".to_vec();

    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html; charset=iso-8859-1")
                .set_body_bytes(body_bytes),
        )
        .mount(&mock_server)
        .await;

    let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().text, "Schlüssel");
}

/// Content-Type with a capital-C "Charset" key and a quoted value must still
/// resolve to the correct encoding — not silently fall back to UTF-8.
/// 0xFC is ü in ISO-8859-1; if the charset is missed it would decode as garbage.
#[tokio::test]
async fn test_fetch_text_iso8859_uppercase_quoted_charset() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    let body_bytes: Vec<u8> = b"Schl\xFCssel".to_vec();

    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html; Charset=\"iso-8859-1\"")
                .set_body_bytes(body_bytes),
        )
        .mount(&mock_server)
        .await;

    let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().text,
        "Schlüssel",
        "capital-C Charset with quoted value must decode ISO-8859-1, not fall back to UTF-8"
    );
}

/// When Content-Type has no charset, fetch_text falls back to UTF-8.
#[tokio::test]
async fn test_fetch_text_no_charset_fallback_utf8() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    let body = "hello wörld";
    Mock::given(method("GET"))
        .respond_with(
            // Content-Type with no charset= parameter.
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string(body),
        )
        .mount(&mock_server)
        .await;

    let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().text, body);
}

/// `retry_after_ms` must clamp a hostile huge `Retry-After` value to ≤ 30 000 ms.
///
/// A board returning `Retry-After: 4294967295` (u32::MAX, well into the
/// saturating_mul danger zone) must NOT produce a 49-day wait.
/// The function is private, so we reach it through a helper that builds a
/// minimal HeaderMap with just the `retry-after` key.
#[test]
fn test_retry_after_overflow_clamped() {
    fn make_headers(value: &str) -> reqwest::header::HeaderMap {
        let mut m = reqwest::header::HeaderMap::new();
        m.insert(
            reqwest::header::HeaderName::from_static("retry-after"),
            reqwest::header::HeaderValue::from_str(value).unwrap(),
        );
        m
    }

    // Large value that would overflow u64::checked_mul(1_000): result must be ≤ 30_000.
    let huge = make_headers("4294967295");
    let ms = retry_after_ms(&huge).expect("should parse as u64");
    assert!(
        ms <= 30_000,
        "huge Retry-After must be clamped to ≤ 30 000 ms, got {ms}"
    );

    // u64::MAX / 1_000 + 1 — saturating_mul would produce u64::MAX without the clamp.
    let near_max = make_headers("18446744073709552");
    let ms2 = retry_after_ms(&near_max).expect("should parse as u64");
    assert!(
        ms2 <= 30_000,
        "near-MAX Retry-After must be clamped to ≤ 30 000 ms, got {ms2}"
    );

    // Sanity: a normal 5-second value is NOT clamped.
    let normal = make_headers("5");
    assert_eq!(
        retry_after_ms(&normal),
        Some(5_000),
        "5 s → 5 000 ms, no clamping"
    );

    // Exactly 30 s — should pass through as 30 000 ms (at the boundary, not over).
    let boundary = make_headers("30");
    assert_eq!(
        retry_after_ms(&boundary),
        Some(30_000),
        "30 s is exactly at the 30 000 ms cap"
    );

    // 31 s — should be clamped to 30 000 ms.
    let over = make_headers("31");
    assert_eq!(
        retry_after_ms(&over),
        Some(30_000),
        "31 s must be clamped to 30 000 ms"
    );
}

/// A 429 with `Retry-After: 0` (or very small) retries immediately and
/// succeeds on the next attempt.
#[tokio::test]
async fn test_fetch_text_429_with_retry_after_succeeds() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    // First request returns 429 with Retry-After: 0 (retry immediately).
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    // Subsequent requests succeed.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&mock_server)
        .await;

    let result = fetch_text(
        &mock_server.uri(),
        FetchOptions {
            retries: 2,
            ..Default::default()
        },
        signal,
    )
    .await;
    assert!(result.is_ok(), "should succeed after 429 + retry");
    assert_eq!(result.unwrap().text, "ok");
}

/// Per-host limiter: `for_host` returns a shared limiter and `record_request`
/// correctly registers a request. This verifies the registry mechanics without
/// running a full HTTP round-trip.
#[tokio::test]
async fn test_per_host_limiter_get_or_create() {
    let rl1 = crate::scraping::rate_limiter::for_host("example.com").await;
    let rl2 = crate::scraping::rate_limiter::for_host("example.com").await;
    // Both calls must return a pointer to the SAME limiter (same Arc address).
    assert!(
        std::sync::Arc::ptr_eq(&rl1, &rl2),
        "same host must return the same rate-limiter Arc"
    );

    // Distinct hosts get distinct limiters.
    let rl3 = crate::scraping::rate_limiter::for_host("other.example.com").await;
    assert!(
        !std::sync::Arc::ptr_eq(&rl1, &rl3),
        "different hosts must have distinct rate-limiters"
    );
}

#[tokio::test]
async fn test_fetch_text_cancelled() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
