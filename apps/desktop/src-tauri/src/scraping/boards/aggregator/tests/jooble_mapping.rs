use super::super::*;
use super::support::*;

// ── Jooble response → JobPosting mapping ─────────────────────────────────────

#[test]
fn jooble_response_maps_to_job_posting() {
    let json = serde_json::json!({
        "totalCount": 1,
        "jobs": [{
            "id": "abc-1",
            "title": "Backend Engineer",
            "company": "Acme",
            "location": "Munich",
            "snippet": "Truncated description…",
            "salary": "€60,000",
            "link": "https://jooble.org/desc/abc-1",
            "updated": "2026-05-01T10:00:00Z"
        }]
    });
    let resp: JoobleResp = serde_json::from_value(json).unwrap();
    let posting = map_jooble_job(resp.jobs.into_iter().next().unwrap(), 0).unwrap();

    assert_eq!(posting.external_id.as_deref(), Some("jooble-abc-1"));
    assert_eq!(posting.id, "aggregator:jooble-abc-1");
    assert_eq!(posting.title, "Backend Engineer");
    assert_eq!(posting.company, "Acme");
    assert_eq!(posting.location.as_deref(), Some("Munich"));
    assert_eq!(posting.url, "https://jooble.org/desc/abc-1");
    assert_eq!(
        posting.extra.get("salaryText").and_then(|v| v.as_str()),
        Some("€60,000")
    );
    assert!(posting.posted_at.unwrap() > 0);
}

/// Jooble's REAL live `updated` value has NO timezone offset despite looking
/// ISO-8601-ish (e.g. "2026-05-15T00:00:00.0000000", 7-digit fraction) — a
/// live-key test found `parse_from_rfc3339` returns `None` for every real
/// Jooble job with this shape. The naive-datetime fallback (assumed UTC) must
/// recover it.
#[test]
fn jooble_updated_without_timezone_parses_as_utc() {
    let job = JoobleJob {
        id: Some("no-tz".to_string()),
        title: Some("No-TZ job".to_string()),
        company: None,
        location: None,
        snippet: None,
        salary: None,
        link: Some("https://jooble.org/desc/no-tz".to_string()),
        updated: Some("2026-05-15T00:00:00.0000000".to_string()),
    };
    let posting = map_jooble_job(job, 0).unwrap();

    let expected = chrono::NaiveDate::from_ymd_opt(2026, 5, 15)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();
    assert_eq!(
        posting.posted_at,
        Some(expected),
        "the naive-datetime (no-TZ) fallback must recover Jooble's real timestamp shape"
    );
}

/// A TZ-bearing RFC3339 `updated` (in case an entry ever does carry an offset)
/// must still parse via the FIRST branch, not the naive-datetime fallback.
#[test]
fn jooble_updated_with_timezone_still_parses_via_rfc3339() {
    let job = JoobleJob {
        id: Some("with-tz".to_string()),
        title: Some("With-TZ job".to_string()),
        company: None,
        location: None,
        snippet: None,
        salary: None,
        link: Some("https://jooble.org/desc/with-tz".to_string()),
        updated: Some("2026-05-15T02:00:00+02:00".to_string()),
    };
    let posting = map_jooble_job(job, 0).unwrap();

    let expected = chrono::NaiveDate::from_ymd_opt(2026, 5, 15)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();
    assert_eq!(
        posting.posted_at,
        Some(expected),
        "a TZ-bearing timestamp (+02:00, i.e. 00:00 UTC) must parse via RFC3339"
    );
}

/// A malformed `updated` string matches neither branch → `posted_at` stays
/// `None`, never a panic or a wrong guess.
#[test]
fn jooble_malformed_updated_yields_no_posted_at() {
    let job = JoobleJob {
        id: Some("bad-date".to_string()),
        title: Some("Bad-date job".to_string()),
        company: None,
        location: None,
        snippet: None,
        salary: None,
        link: Some("https://jooble.org/desc/bad-date".to_string()),
        updated: Some("not-a-date".to_string()),
    };
    let posting = map_jooble_job(job, 0).unwrap();
    assert_eq!(posting.posted_at, None);
}

/// Jooble's `id` can arrive as a bare integer — must parse and normalise to
/// String (same shape as the Adzuna `id` regression this mirrors).
#[test]
fn jooble_integer_id_deserializes_to_string() {
    let json = serde_json::json!({
        "jobs": [{
            "id": 987654,
            "title": "QA Engineer",
            "company": "Corp",
            "location": null,
            "snippet": null,
            "salary": null,
            "link": "https://jooble.org/desc/987654",
            "updated": null
        }]
    });
    let resp: JoobleResp = serde_json::from_value(json).expect("integer id must deserialize");
    assert_eq!(resp.jobs[0].id.as_deref(), Some("987654"));
}

/// A job missing `id` falls back to the URL as the dedupe key, still prefixed
/// `"jooble-"` — dedup must never crash/collapse on a missing id.
#[test]
fn jooble_missing_id_falls_back_to_url_based_external_id() {
    let job = JoobleJob {
        id: None,
        title: Some("No-id job".to_string()),
        company: Some("Co".to_string()),
        location: None,
        snippet: None,
        salary: None,
        link: Some("https://jooble.org/desc/no-id".to_string()),
        updated: None,
    };
    let posting = map_jooble_job(job, 0).unwrap();
    assert_eq!(
        posting.external_id.as_deref(),
        Some("jooble-https://jooble.org/desc/no-id")
    );
}

/// Jobs missing a title, or missing a link, are dropped — neither can be shown
/// nor opened.
#[test]
fn jooble_drops_jobs_without_title_or_link() {
    let no_title = JoobleJob {
        id: Some("1".to_string()),
        title: None,
        company: None,
        location: None,
        snippet: None,
        salary: None,
        link: Some("https://jooble.org/desc/1".to_string()),
        updated: None,
    };
    let no_link = JoobleJob {
        id: Some("2".to_string()),
        title: Some("Has title".to_string()),
        company: None,
        location: None,
        snippet: None,
        salary: None,
        link: None,
        updated: None,
    };
    assert!(map_jooble_job(no_title, 0).is_none());
    assert!(map_jooble_job(no_link, 0).is_none());
}

// ── Jooble is_configured() / network-avoidance guard ───────────────────────────

#[tokio::test]
async fn jooble_unconfigured_returns_err_without_network() {
    let p = JoobleProvider { api_key: None };
    let result = p
        .search("engineer", "berlin", "de", false, None, None, make_token())
        .await;
    assert!(result.is_err(), "unconfigured Jooble must return Err");
    assert!(
        result.unwrap_err().to_string().contains("not configured"),
        "error must say 'not configured'"
    );
}

// ── Jooble non-2xx / redact_path wiring (wiremock) ──────────────────────────────

/// A non-2xx (e.g. a bad-key 403) propagates as a provider failure prefixed
/// `"jooble:"` — so `BoardScrapeSummary.error` names which provider failed —
/// and the API key (embedded in the URL PATH per Jooble's contract, not a
/// header/query) never appears in the surfaced message, only the HTTP status.
#[tokio::test]
async fn jooble_non_2xx_maps_to_prefixed_err() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(403).set_body_string(r#"{"message":"bad key"}"#))
        .mount(&mock_server)
        .await;

    let result = fetch_jooble(
        &mock_server.uri(),
        "totally-fake-key",
        "engineer",
        "Berlin",
        None,
        make_token(),
    )
    .await;

    let msg = result.unwrap_err().to_string();
    assert!(
        msg.starts_with("jooble:"),
        "error must be prefixed so BoardScrapeSummary.error names the provider; got: {msg}"
    );
    assert!(
        msg.contains("403"),
        "the HTTP status must be carried; got: {msg}"
    );
    assert!(
        !msg.contains("totally-fake-key"),
        "the API key (URL-path-embedded) must never appear in the surfaced error; got: {msg}"
    );
}

/// A 2xx Jooble response round-trips end-to-end through `fetch_jooble` (POST +
/// path-embedded key + `redact_path` wiring + response mapping) into a
/// `"jooble-"`-prefixed posting.
#[tokio::test]
async fn jooble_ok_response_maps_end_to_end() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "totalCount": 1,
            "jobs": [{
                "id": 555,
                "title": "Rust Engineer",
                "company": "JoobleCo",
                "location": "Remote",
                "snippet": "Truncated…",
                "salary": "$80,000 - $100,000",
                "link": "https://jooble.org/desc/555",
                "updated": "2026-06-01T09:00:00Z"
            }]
        })))
        .mount(&mock_server)
        .await;

    let items = fetch_jooble(
        &mock_server.uri(),
        "fake-key",
        "engineer",
        "Berlin",
        None,
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].external_id.as_deref(), Some("jooble-555"));
    assert_eq!(items[0].url, "https://jooble.org/desc/555");
}

/// `jooble_endpoint` puts the key in the PATH, not a query param — pins the
/// URL shape independently of the network round trip above.
#[test]
fn jooble_endpoint_puts_key_in_path() {
    let url = jooble_endpoint("https://jooble.org", "my-key");
    assert_eq!(url, "https://jooble.org/api/my-key");
    assert!(
        !url.contains('?'),
        "the key must be a path segment, not a query param"
    );
}
