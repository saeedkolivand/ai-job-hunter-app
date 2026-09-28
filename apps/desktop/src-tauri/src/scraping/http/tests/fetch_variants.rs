//! `fetch_text` edge-case tests: UA-override replace-not-append, stream-cap
//! without `Content-Length`, charset decoding, 429/`Retry-After` backoff,
//! secret-leak-free transport errors, the per-host limiter seam, and
//! cancellation.

use super::super::*;

/// `FetchOptions::user_agent` REPLACES the shared client's default UA rather
/// than riding alongside it — the exact bug it exists to close: putting
/// `user-agent` in `opts.headers` instead would have appended a second header
/// line (`RequestBuilder::header` appends, it does not replace), sending BOTH
/// `DEFAULT_UA` and the override on the wire. Checked via
/// `MockServer::received_requests` (not just a `header()` matcher, which only
/// proves the override value is PRESENT, not that the default is ABSENT) so a
/// regression back to `opts.headers` would show two header values, not one.
///
/// Mutation check: reverted `fetch_text`'s `user-agent` line back to the
/// unconditional `request.header("user-agent", DEFAULT_UA)` (dropping the
/// `opts.user_agent` branch) — RAN, went red (`values.len()` became 1 with
/// the WRONG value: `DEFAULT_UA`, not the override — reqwest's client-default
/// merge only backfills when the request sent NO value for the header, so an
/// unconditional literal write, not an append, is what the un-fixed line
/// actually did), restored.
#[tokio::test]
async fn test_fetch_text_user_agent_override_replaces_not_appends() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    let signal = tokio_util::sync::CancellationToken::new();

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&mock_server)
        .await;

    fetch_text(
        &mock_server.uri(),
        FetchOptions {
            user_agent: Some("custom-agent/1.0".to_string()),
            ..Default::default()
        },
        signal,
    )
    .await
    .expect("the request must succeed");

    let requests = mock_server.received_requests().await.expect(
        "wiremock request recording must be on by default; if this fails, recording was \
         disabled somewhere upstream of this test",
    );
    assert_eq!(requests.len(), 1);
    let values: Vec<&str> = requests[0]
        .headers
        .get_all("user-agent")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    assert_eq!(
        values,
        vec!["custom-agent/1.0"],
        "exactly one user-agent value must reach the wire, and it must be the override — \
         not the override alongside DEFAULT_UA, and not DEFAULT_UA alone"
    );
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

/// Regression: a transport-level failure (connection refused — no wiremock
/// server involved) must not leak the request URL's query string into the
/// resulting `AppError`. `reqwest::Error`'s own `Display` embeds the full
/// URL (" for url (...)") including query params, which can carry API
/// secrets (Adzuna `app_key`, Comeet `token`, ...); `fetch_text`'s
/// transport-failure branches must call `.without_url()` before stringifying.
/// Port 9 (discard) on loopback deterministically refuses connections —
/// hermetic, no real network reached.
#[tokio::test]
async fn fetch_text_transport_error_does_not_leak_query_secret() {
    let signal = tokio_util::sync::CancellationToken::new();
    let url = "http://127.0.0.1:9/jobs?token=SECRET";

    let result = fetch_text(
        url,
        FetchOptions {
            retries: 0,
            // Port 9 (discard) is refused instantly on Unix, but Windows lets the
            // connect attempt sit until its own timeout — which added over a minute
            // to every `cargo test` run on this platform. The ceiling makes the test
            // bounded everywhere without weakening it: a refused connection still
            // fails fast, a hung one fails at 500ms, and BOTH are the transport
            // errors whose `Display` must not carry the query string.
            timeout: Some(Duration::from_millis(500)),
            ..Default::default()
        },
        signal,
    )
    .await;

    let err = result.expect_err("an unreachable port must fail");
    let msg = err.to_string();
    assert!(
        !msg.contains("SECRET"),
        "error string must not leak the query secret: {msg}"
    );
    assert!(
        !msg.contains("token="),
        "error string must not leak the query string at all: {msg}"
    );
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
