//! `parse_web_search` — mapping + capping Ollama Web Search API results.

use serde_json::json;

use super::super::search::parse_web_search;

#[test]
fn parse_web_search_maps_results_and_caps_limit() {
    let body = json!({
        "results": [
            { "title": "Acme — Wikipedia", "url": "https://w/a", "content": "Acme makes widgets." },
            { "title": "Acme careers", "url": "https://a/c", "content": "Series B." },
            { "title": "extra", "url": "https://x", "content": "ignored by limit" },
        ]
    });
    let out = parse_web_search(&body, 2);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].title, "Acme — Wikipedia");
    assert_eq!(out[0].snippet, "Acme makes widgets.");
    assert_eq!(out[1].url, "https://a/c");
}

#[test]
fn parse_web_search_tolerates_missing_fields_and_no_results() {
    assert!(parse_web_search(&json!({}), 5).is_empty());
    let out = parse_web_search(&json!({ "results": [{}] }), 5);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].title, "");
    assert_eq!(out[0].snippet, "");
}
