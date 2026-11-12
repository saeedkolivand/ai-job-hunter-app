//! SearXNG-compatible response mapping and URL validation (#1515).
//!
//! FIXTURES ARE DERIVED, NOT CAPTURED. No live instance serving JSON was reachable (public
//! instances answer 429/403 or HTML for `format=json`), and running a third-party container or
//! binary was ruled out. Each fixture is built from the project's own source for its JSON output;
//! the values are invented (`example.*`), the keys and nesting are the source's:
//!
//! * `fixtures/searxng.json`: SearXNG `searx/webutils.py` `get_json_response` (the top-level
//!   `query` / `results` / `answers` / `corrections` / `infoboxes` / `suggestions` /
//!   `unresponsive_engines` object, lines 161-173) and `searx/result_types/_base.py`
//!   `MainResult` (`url`, `title`, `content`, lines 260-381), `master` branch.
//! * `fixtures/rustxng.json`: RustXNG `src/aggregator.rs` `SearchResponse` (`query`,
//!   `search_query`, `number_of_results`, `results`, ... `timings`, lines 133-147) wrapping
//!   `src/results.rs` `SearchResult` (`url`, `title`, `content`, `engine`, ..., lines 62-80);
//!   served by `src/web/handlers.rs` for `/search?format=json`. Its MCP tool alone names the
//!   snippet `snippet` (`src/mcp/mod.rs` `SearchItem`), covered by an inline case below.

use serde_json::json;

use super::super::searxng::{normalize_searxng_url, parse_searxng_results, searxng_status_error};
use super::super::{falls_through, SearxngSearcher, WebSearcher};
use crate::error::{AppError, FailureKind};

fn fixture(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).expect("fixture is valid JSON")
}

#[test]
fn maps_a_searxng_response_and_drops_a_result_without_content() {
    let body = fixture(include_str!("fixtures/searxng.json"));
    let out = parse_searxng_results(&body, 10).unwrap();
    assert_eq!(out.len(), 2, "the empty-content result is dropped");
    assert_eq!(out[0].title, "About Example Corp");
    assert_eq!(out[0].url, "https://example.com/about");
    assert!(out[0].snippet.starts_with("Example Corp builds payment"));
    assert_eq!(out[1].url, "https://example.org/news/example-corp-funding");
}

#[test]
fn maps_a_rustxng_response() {
    let body = fixture(include_str!("fixtures/rustxng.json"));
    let out = parse_searxng_results(&body, 10).unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].title, "About Example Corp");
    assert_eq!(out[1].snippet, "Headquartered in Utrecht.");
}

/// RustXNG's MCP `search` tool names the snippet `snippet`; `content` wins when both exist.
#[test]
fn reads_snippet_when_there_is_no_content() {
    let body = json!({ "results": [
        { "url": "https://u1.test", "title": "t1", "snippet": "from snippet" },
        { "url": "https://u2.test", "title": "t2", "content": "from content", "snippet": "ignored" },
    ]});
    let out = parse_searxng_results(&body, 10).unwrap();
    assert_eq!(out[0].snippet, "from snippet");
    assert_eq!(out[1].snippet, "from content");
}

#[test]
fn honours_the_limit_and_caps_content_on_a_char_boundary() {
    let long = "é".repeat(5_000);
    let results: Vec<_> = (0..6)
        .map(|i| json!({ "url": format!("https://u{i}.test"), "title": "t", "content": long }))
        .collect();
    let out = parse_searxng_results(&json!({ "results": results }), 3).unwrap();
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].snippet.chars().count(), 1_000);
}

/// Present-but-empty `results` is an answer ("nothing found"); a body that is not search output
/// is a typed error, never an empty `Ok`. Mutation check: default the missing array to `[]` and
/// the error cases fail.
#[test]
fn a_body_that_is_not_search_output_is_an_error_not_an_empty_result() {
    assert!(parse_searxng_results(&json!({ "results": [] }), 5)
        .unwrap()
        .is_empty());
    for bad in [
        json!({}),
        json!({ "results": "nope" }),
        json!({ "error": "bad request" }),
        json!([]),
    ] {
        assert!(
            matches!(parse_searxng_results(&bad, 5), Err(AppError::Provider(_))),
            "{bad}"
        );
    }
}

#[test]
fn non_success_statuses_are_typed() {
    use reqwest::StatusCode as S;
    assert!(matches!(
        searxng_status_error(S::FORBIDDEN),
        AppError::Config(_)
    ));
    assert!(matches!(
        searxng_status_error(S::TOO_MANY_REQUESTS),
        AppError::RateLimited(_)
    ));
    assert!(matches!(
        searxng_status_error(S::BAD_GATEWAY),
        AppError::Network(_)
    ));
    assert!(matches!(
        searxng_status_error(S::BAD_REQUEST),
        AppError::Provider(_)
    ));
}

#[test]
fn the_url_must_be_http_or_https_with_a_host() {
    assert_eq!(
        normalize_searxng_url("  http://localhost:8888/ ").unwrap(),
        Some("http://localhost:8888".to_string())
    );
    assert_eq!(
        normalize_searxng_url("https://search.example.org/searx/").unwrap(),
        Some("https://search.example.org/searx".to_string())
    );
    assert_eq!(normalize_searxng_url("   ").unwrap(), None);
    // Plain http only for this machine and the local network.
    for ok in [
        "http://127.0.0.1:8888",
        "http://192.168.1.20:8080",
        "http://10.0.0.5",
        "http://localhost",
        "http://searx.local:8888",
        "http://[::1]:8888",
        "https://search.example.org",
    ] {
        assert!(
            normalize_searxng_url(ok).unwrap().is_some(),
            "{ok} must be accepted"
        );
    }
    for bad in [
        "ftp://host",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "not a url",
        "http://",
        "http://169.254.169.254",
        "https://user@host.example.org",
        "https://user:pass@host.example.org",
        "http://:secret@localhost:8888",
        "http://search.example.org",
        "http://8.8.8.8:8888",
        "https://host/?q=1",
        "https://host/#frag",
    ] {
        assert!(
            normalize_searxng_url(bad).is_err(),
            "{bad} must be rejected"
        );
    }
}

/// An EMPTY `results` beside failed engines is `Unavailable`, not "found nothing". Mutation
/// check: drop the `engines_failed` branch and the first assertion fails.
#[test]
fn empty_results_with_unresponsive_engines_is_unavailable() {
    let body = json!({
        "results": [],
        "unresponsive_engines": [["duckduckgo", "CAPTCHA"], ["brave", "suspended: too many requests"]]
    });
    let err = parse_searxng_results(&body, 5).err().unwrap();
    assert_eq!(
        FailureKind::of(&err),
        Some(FailureKind::Unavailable),
        "{err}"
    );
    assert!(!falls_through(&err));
    // Plain empty, or results alongside failed engines, are answers.
    assert!(
        parse_searxng_results(&json!({ "results": [], "unresponsive_engines": [] }), 5)
            .unwrap()
            .is_empty()
    );
    let partial = json!({
        "results": [{ "url": "https://a.test", "title": "t", "content": "c" }],
        "unresponsive_engines": [["brave", "timeout"]]
    });
    assert_eq!(parse_searxng_results(&partial, 5).unwrap().len(), 1);
}

#[test]
fn title_is_capped_and_a_result_without_an_http_url_is_dropped() {
    let body = json!({ "results": [
        { "url": "https://a.test", "title": "é".repeat(500), "content": "c" },
        { "url": "javascript:alert(1)", "title": "t", "content": "c" },
        { "url": "", "title": "t", "content": "c" },
        { "url": format!("https://a.test/{}", "x".repeat(3_000)), "title": "t", "content": "c" },
    ]});
    let out = parse_searxng_results(&body, 10).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].title.chars().count(), 200);
}

/// Mutation check: put reqwest's own `Display` (which embeds the URL and the `q=` text) back in
/// the message and the kind turns into Billing/Auth, and the query leaks into the error.
/// A real request against a closed loopback port, with a query a text sniffer would misread.
#[tokio::test]
async fn an_unreachable_server_is_unavailable_and_never_echoes_the_query() {
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let searcher = SearxngSearcher::new(&format!("http://127.0.0.1:{port}")).unwrap();
    let err = searcher
        .try_search("Route 401 Ltd Hospital 402 401(k) 403", 5)
        .await
        .err()
        .unwrap();
    assert_eq!(
        FailureKind::of(&err),
        Some(FailureKind::Unavailable),
        "{err}"
    );
    assert!(!falls_through(&err));
    let text = err.to_string();
    assert!(
        !text.contains("401") && !text.contains("127.0.0.1"),
        "{text}"
    );
}
