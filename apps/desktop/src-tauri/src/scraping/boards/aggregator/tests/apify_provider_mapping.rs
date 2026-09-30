use super::super::*;
use super::support::*;

// ── Apify LinkedIn provider ────────────────────────────────────────────────────

/// The Rust setting-key literals must equal the cross-language contract in
/// `packages/shared/src/scraping-settings.ts`. The frontend writes these exact
/// strings; this pins the Rust read-side so it can't silently drift.
#[test]
fn aggregator_settings_keys_match_shared_contract() {
    assert_eq!(SCRAPING_SETTINGS_FILE, "scraping-settings.json");
    assert_eq!(SETTING_APIFY_ENABLED, "apifyLinkedinEnabled");
    assert_eq!(SETTING_APIFY_ACTOR_ID, "apifyLinkedinActorId");
}

/// `is_configured()` requires BOTH a token AND the opt-in toggle. A token alone
/// (toggle OFF) must NOT configure the provider — never run a paid scrape just
/// because a token is stored.
#[test]
fn apify_is_configured_requires_token_and_toggle() {
    assert_eq!(apify(Some("t"), true).provider_id(), "apify_linkedin");

    // token + toggle OFF → not configured (the cost gate).
    assert!(
        !apify(Some("t"), false).is_configured(),
        "token without the opt-in toggle must NOT be configured"
    );
    // toggle ON + no token → not configured.
    assert!(
        !apify(None, true).is_configured(),
        "toggle without a token must NOT be configured"
    );
    // neither → not configured.
    assert!(!apify(None, false).is_configured());
    // both → configured.
    assert!(
        apify(Some("t"), true).is_configured(),
        "token + toggle must be configured"
    );
}

/// An unconfigured provider must Err WITHOUT issuing any network request.
#[tokio::test]
async fn apify_unconfigured_returns_err_without_network() {
    let p = apify(Some("t"), false);
    let result = p
        .search("engineer", "berlin", "de", false, None, None, make_token())
        .await;
    assert!(result.is_err(), "unconfigured Apify must return Err");
    assert!(
        result.unwrap_err().to_string().contains("not configured"),
        "error must say 'not configured'"
    );
}

/// `f_TPR` recency mapping: sub-day → r86400, week → r604800, else (month / none /
/// unknown) → r2592000.
#[test]
fn apify_f_tpr_maps_recency() {
    assert_eq!(apify_f_tpr(Some("15m")), "r86400");
    assert_eq!(apify_f_tpr(Some("30m")), "r86400");
    assert_eq!(apify_f_tpr(Some("1h")), "r86400");
    assert_eq!(apify_f_tpr(Some("24h")), "r86400");
    assert_eq!(apify_f_tpr(Some("week")), "r604800");
    assert_eq!(apify_f_tpr(Some("month")), "r2592000");
    assert_eq!(apify_f_tpr(None), "r2592000");
    assert_eq!(apify_f_tpr(Some("nonsense")), "r2592000");
}

/// The LinkedIn search URL is built from query/location/date_filter with
/// percent-encoding and the mapped `f_TPR`.
#[test]
fn apify_builds_linkedin_search_url_with_encoding() {
    let url = build_linkedin_search_url("Rust & C++", "München", Some("week"));
    assert!(
        url.starts_with("https://www.linkedin.com/jobs/search/?"),
        "must be a LinkedIn jobs-search URL; got: {url}"
    );
    // `Rust & C++` → space=%20, &=%26, +=%2B.
    assert!(
        url.contains("keywords=Rust%20%26%20C%2B%2B"),
        "query must be percent-encoded; got: {url}"
    );
    assert!(
        url.contains("location=M%C3%BCnchen"),
        "location umlaut must be percent-encoded; got: {url}"
    );
    assert!(url.contains("f_TPR=r604800"), "week → r604800; got: {url}");

    // No date filter → the month ceiling.
    let url_none = build_linkedin_search_url("dev", "Berlin", None);
    assert!(url_none.contains("f_TPR=r2592000"));
}

/// Representative dataset item (documented field names) → JobPosting.
#[test]
fn apify_maps_representative_item() {
    let json = serde_json::json!({
        "title": "Senior Rust Engineer",
        "companyName": "RustCorp",
        "location": "Berlin, Germany",
        "jobUrl": "https://www.linkedin.com/jobs/view/123",
        "id": 123,
        "postedAt": "2026-06-01T09:00:00Z",
        "descriptionText": "Build things in Rust."
    });
    let item: ApifyItem = serde_json::from_value(json).unwrap();
    let p = map_apify_item(item, 999).expect("a complete item maps");

    assert_eq!(p.title, "Senior Rust Engineer");
    assert_eq!(p.company, "RustCorp");
    assert_eq!(p.location.as_deref(), Some("Berlin, Germany"));
    assert_eq!(p.url, "https://www.linkedin.com/jobs/view/123");
    assert_eq!(p.external_id.as_deref(), Some("linkedin-123"));
    assert_eq!(p.id, "aggregator:linkedin-123");
    assert_eq!(p.source, "aggregator");
    assert!(p.description.as_deref().unwrap_or("").contains("Rust"));
    assert_eq!(p.captured_at, 999);
    let expected_ts = chrono::DateTime::parse_from_rfc3339("2026-06-01T09:00:00Z")
        .unwrap()
        .timestamp_millis();
    assert_eq!(p.posted_at, Some(expected_ts));
}

/// Alternate field names (`jobTitle`, `jobDescription`) + no `jobUrl` → URL is
/// constructed from the numeric `id`, and HTML description is converted.
#[test]
fn apify_maps_alternate_field_names_and_constructs_url_from_id() {
    let json = serde_json::json!({
        "jobTitle": "Backend Dev",
        "companyName": "StartupAG",
        "id": "987",
        "jobDescription": "<ul><li>Write Rust</li></ul>"
    });
    let item: ApifyItem = serde_json::from_value(json).unwrap();
    let p = map_apify_item(item, 0).expect("alternate-named item maps");

    assert_eq!(p.title, "Backend Dev");
    assert_eq!(p.url, "https://www.linkedin.com/jobs/view/987");
    assert_eq!(p.external_id.as_deref(), Some("linkedin-987"));
    assert!(p
        .description
        .as_deref()
        .unwrap_or("")
        .contains("Write Rust"));
}

/// An item with no title — or a title but no URL and no id — is skipped (None).
#[test]
fn apify_skips_item_without_title_or_url() {
    let no_title: ApifyItem = serde_json::from_value(serde_json::json!({
        "jobUrl": "https://www.linkedin.com/jobs/view/1"
    }))
    .unwrap();
    assert!(
        map_apify_item(no_title, 0).is_none(),
        "item without a title must be skipped"
    );

    let no_url: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Ghost Job"
    }))
    .unwrap();
    assert!(
        map_apify_item(no_url, 0).is_none(),
        "item with no jobUrl and no id must be skipped"
    );
}

/// The actor returns a JSON ARRAY of items; the array deserializes and each item
/// maps independently.
#[test]
fn apify_parses_dataset_array() {
    let json = serde_json::json!([
        { "title": "A", "jobUrl": "https://www.linkedin.com/jobs/view/1" },
        { "jobTitle": "B", "id": 2 },
        { "companyName": "Skip Me — no title/url" }
    ]);
    let items: Vec<ApifyItem> = serde_json::from_value(json).unwrap();
    let mapped: Vec<_> = items
        .into_iter()
        .filter_map(|i| map_apify_item(i, 0))
        .collect();
    assert_eq!(mapped.len(), 2, "the third item (no title/url) is dropped");
}
