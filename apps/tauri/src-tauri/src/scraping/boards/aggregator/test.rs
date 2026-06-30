use super::*;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_token() -> tokio_util::sync::CancellationToken {
    tokio_util::sync::CancellationToken::new()
}

fn sample_posting(id: &str, provider: &str) -> JobPosting {
    JobPosting {
        id: format!("aggregator:{provider}-{id}"),
        external_id: Some(format!("{provider}-{id}")),
        title: format!("Engineer {id}"),
        company: "Acme".to_string(),
        location: Some("Berlin, DE".to_string()),
        url: format!("https://{provider}.example.com/job/{id}"),
        source: "aggregator".to_string(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: std::collections::HashMap::new(),
    }
}

// ── Fake providers ────────────────────────────────────────────────────────────

struct FakeProvider {
    id: &'static str,
    configured: bool,
    result: Result<Vec<JobPosting>, &'static str>,
}

impl FakeProvider {
    fn ok(id: &'static str, items: Vec<JobPosting>) -> Self {
        Self {
            id,
            configured: true,
            result: Ok(items),
        }
    }

    fn err(id: &'static str, msg: &'static str) -> Self {
        Self {
            id,
            configured: true,
            result: Err(msg),
        }
    }

    fn unconfigured(id: &'static str) -> Self {
        Self {
            id,
            configured: false,
            result: Ok(vec![]),
        }
    }
}

#[async_trait::async_trait]
impl JobProvider for FakeProvider {
    fn provider_id(&self) -> &'static str {
        self.id
    }

    fn is_configured(&self) -> bool {
        self.configured
    }

    async fn search(
        &self,
        _query: &str,
        _location: &str,
        _country: &str,
        _date_filter: Option<&str>,
        _signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        match &self.result {
            Ok(v) => Ok(v.clone()),
            Err(msg) => Err(anyhow::anyhow!(*msg)),
        }
    }
}

// ── Fallback logic tests ──────────────────────────────────────────────────────

/// Adzuna Ok(items) → those items returned, JSearch not called (fake JSearch
/// always errors to prove it wasn't reached).
#[tokio::test]
async fn adzuna_ok_returns_items_no_jsearch() {
    let posting = sample_posting("1", "adzuna");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![posting.clone()])),
        Box::new(FakeProvider::err("jsearch", "should not be called")),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, make_token())
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, posting.external_id);
}

/// Adzuna Ok(empty) → empty returned, JSearch NOT called.
#[tokio::test]
async fn adzuna_ok_empty_does_not_call_jsearch() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![])),
        // JSearch is configured and would return items — but must not be called.
        Box::new(FakeProvider::ok(
            "jsearch",
            vec![sample_posting("1", "jsearch")],
        )),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, make_token())
        .await
        .unwrap();

    // Adzuna returned empty → result is empty (JSearch was bypassed).
    assert_eq!(
        result.len(),
        0,
        "JSearch must not be called when Adzuna returns Ok(empty)"
    );
}

/// Adzuna Err → JSearch called and its results returned.
#[tokio::test]
async fn adzuna_err_falls_back_to_jsearch() {
    let jsearch_posting = sample_posting("42", "jsearch");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "api error")),
        Box::new(FakeProvider::ok("jsearch", vec![jsearch_posting.clone()])),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, make_token())
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, jsearch_posting.external_id);
}

/// Neither configured → Ok(empty), never an error.
#[tokio::test]
async fn neither_configured_returns_empty() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::unconfigured("adzuna")),
        Box::new(FakeProvider::unconfigured("jsearch")),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, make_token())
        .await
        .unwrap();

    assert!(result.is_empty());
}

/// Only JSearch configured (Adzuna absent) → JSearch used.
#[tokio::test]
async fn only_jsearch_configured_uses_jsearch() {
    let jsearch_posting = sample_posting("7", "jsearch");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::unconfigured("adzuna")),
        Box::new(FakeProvider::ok("jsearch", vec![jsearch_posting.clone()])),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, make_token())
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, jsearch_posting.external_id);
}

/// Adzuna configured + Err, JSearch not configured → diagnostic Err (not silent empty).
/// Previously this returned Ok(empty), which was the silent-zero bug. The new contract
/// surfaces an actionable error so the engine records it in BoardScrapeSummary.error.
#[tokio::test]
async fn adzuna_configured_err_and_no_jsearch_returns_diagnostic_err() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "timeout")),
        Box::new(FakeProvider::unconfigured("jsearch")),
    ];

    let result =
        search_with_providers(&providers, "engineer", "berlin", "de", None, make_token()).await;

    assert!(
        result.is_err(),
        "Adzuna configured+failed with no JSearch must surface an Err, not silent Ok(empty)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("timeout"),
        "diagnostic error must include the original Adzuna failure ('timeout'); got: {msg}"
    );
    assert!(
        msg.contains("add a JSearch key in Settings"),
        "diagnostic error must include the actionable JSearch-remedy suffix; got: {msg}"
    );
}

/// Deduplication: two items with the same external_id → only first kept.
#[tokio::test]
async fn deduplication_by_external_id() {
    let dup = sample_posting("99", "adzuna");
    let items = vec![dup.clone(), dup.clone()];
    let deduped = dedupe(items);
    assert_eq!(deduped.len(), 1);
}

// ── is_configured reflects key presence ──────────────────────────────────────

#[test]
fn adzuna_is_configured_requires_both_keys() {
    // Neither key → not configured.
    let unconfigured = AdzunaProvider {
        app_id: None,
        app_key: None,
    };
    assert!(!unconfigured.is_configured());

    // Only one key → not configured.
    let partial = AdzunaProvider {
        app_id: Some("id123".to_string()),
        app_key: None,
    };
    assert!(!partial.is_configured());

    // Both keys → configured.
    let full = AdzunaProvider {
        app_id: Some("id123".to_string()),
        app_key: Some("key456".to_string()),
    };
    assert!(full.is_configured());
}

#[test]
fn jsearch_is_configured_reflects_key_presence() {
    let no_key = JSearchProvider { api_key: None };
    assert!(!no_key.is_configured());

    let with_key = JSearchProvider {
        api_key: Some("rapidapikey".to_string()),
    };
    assert!(with_key.is_configured());
}

// ── URL / query building ──────────────────────────────────────────────────────

#[test]
fn adzuna_url_encodes_query_and_location() {
    // Verify that special characters in query/location don't break the URL.
    // (We test the encoding indirectly via urlencoding::encode behavior.)
    let q = "Rust & C++";
    let loc = "München, Bayern";
    let q_enc = urlencoding::encode(q).to_string();
    let loc_enc = urlencoding::encode(loc).to_string();

    assert!(q_enc.contains("%26"), "& must be percent-encoded");
    assert!(!q_enc.contains(' '), "spaces must be encoded");
    assert!(loc_enc.contains("%C3%BC"), "ü must be percent-encoded");
}

#[test]
fn jsearch_combines_query_and_location() {
    let query = "software engineer";
    let location = "Berlin";
    let combined = format!("{query} in {location}");
    assert_eq!(combined, "software engineer in Berlin");
}

#[test]
fn jsearch_query_only_when_location_empty() {
    let query = "software engineer";
    let location = "";
    let combined = if location.is_empty() {
        query.to_string()
    } else {
        format!("{query} in {location}")
    };
    assert_eq!(combined, "software engineer");
}

#[test]
fn adzuna_defaults_country_to_de() {
    let country = "";
    let resolved = if country.is_empty() { "de" } else { country };
    assert_eq!(resolved, "de");
}

// ── Adzuna response → JobPosting mapping ─────────────────────────────────────

#[test]
fn adzuna_response_maps_to_job_posting() {
    // Parse a fixture that mirrors the real Adzuna JSON shape.
    let json = serde_json::json!({
        "count": 1,
        "results": [{
            "id": "abc123",
            "title": "Senior Rust Engineer",
            "company": { "display_name": "RustCorp" },
            "location": { "display_name": "Berlin, Germany", "area": ["Germany", "Berlin"] },
            "redirect_url": "https://api.adzuna.com/v1/api/jobs/de/redirects/abc123",
            "description": "<p>Job description here</p>",
            "created": "2026-06-01T09:00:00Z",
            "salary_min": 70000.0,
            "salary_max": 90000.0
        }]
    });

    let resp: AdzunaResp = serde_json::from_value(json).unwrap();
    let j = &resp.results[0];

    assert_eq!(j.id, "abc123");
    assert_eq!(j.title, "Senior Rust Engineer");
    assert_eq!(
        j.company.as_ref().and_then(|c| c.display_name.as_deref()),
        Some("RustCorp")
    );
    assert_eq!(
        j.location.as_ref().and_then(|l| l.display_name.as_deref()),
        Some("Berlin, Germany")
    );
    assert_eq!(
        j.redirect_url,
        "https://api.adzuna.com/v1/api/jobs/de/redirects/abc123"
    );
    assert!(j
        .description
        .as_deref()
        .unwrap_or("")
        .contains("Job description"));
    // posted_at: 2026-06-01T09:00:00Z → positive ms timestamp
    let ts = chrono::DateTime::parse_from_rfc3339(j.created.as_deref().unwrap())
        .unwrap()
        .timestamp_millis();
    assert!(ts > 0);
    assert_eq!(j.salary_min, Some(70000.0));
    assert_eq!(j.salary_max, Some(90000.0));
}

/// Adzuna live API sends `id` as an integer — must parse and normalise to String.
/// Regression for: "invalid type: integer `331705081`, expected a string".
#[test]
fn adzuna_integer_id_deserializes_to_string() {
    let json = serde_json::json!({
        "results": [{
            "id": 331705081_i64,
            "title": "Rust Engineer",
            "company": { "display_name": "Corp" },
            "location": { "display_name": "Berlin" },
            "redirect_url": "https://api.adzuna.com/v1/api/jobs/de/redirects/331705081",
            "description": null,
            "created": null,
            "salary_min": null,
            "salary_max": null
        }]
    });

    let resp: AdzunaResp =
        serde_json::from_value(json).expect("integer id must deserialize without error");
    let j = &resp.results[0];
    assert_eq!(j.id, "331705081");
    // Confirm the id maps correctly through the JobPosting formatting.
    assert_eq!(format!("adzuna-{}", j.id), "adzuna-331705081");
    assert_eq!(
        format!("aggregator:adzuna-{}", j.id),
        "aggregator:adzuna-331705081"
    );
}

/// String `id` (original documented shape) must still deserialize correctly
/// after the `de_string_or_number` migration.
#[test]
fn adzuna_string_id_still_deserializes() {
    let json = serde_json::json!({
        "results": [{
            "id": "abc123",
            "title": "Senior Rust Engineer",
            "company": { "display_name": "RustCorp" },
            "location": { "display_name": "Berlin" },
            "redirect_url": "https://api.adzuna.com/v1/api/jobs/de/redirects/abc123",
            "description": null,
            "created": null,
            "salary_min": null,
            "salary_max": null
        }]
    });

    let resp: AdzunaResp = serde_json::from_value(json).expect("string id must still deserialize");
    assert_eq!(resp.results[0].id, "abc123");
}

// ── JSearch response → JobPosting mapping ────────────────────────────────────

#[test]
fn jsearch_response_maps_to_job_posting() {
    let json = serde_json::json!({
        "status": "OK",
        "data": [{
            "job_id": "xyz789",
            "job_title": "Backend Developer",
            "employer_name": "StartupAG",
            "job_city": "Munich",
            "job_country": "DE",
            "job_apply_link": "https://startupag.example.com/jobs/xyz789",
            "job_description": "<ul><li>Write Rust</li></ul>",
            "job_posted_at_datetime_utc": "2026-05-15T12:00:00Z"
        }]
    });

    let resp: JSearchResp = serde_json::from_value(json).unwrap();
    let j = &resp.data[0];

    assert_eq!(j.job_id, "xyz789");
    assert_eq!(j.job_title, "Backend Developer");
    assert_eq!(j.employer_name.as_deref(), Some("StartupAG"));
    assert_eq!(j.job_city.as_deref(), Some("Munich"));
    assert_eq!(j.job_country.as_deref(), Some("DE"));
    assert_eq!(
        j.job_apply_link.as_deref(),
        Some("https://startupag.example.com/jobs/xyz789")
    );
    assert!(j
        .job_description
        .as_deref()
        .unwrap_or("")
        .contains("Write Rust"));
    let ts = chrono::DateTime::parse_from_rfc3339(j.job_posted_at_datetime_utc.as_deref().unwrap())
        .unwrap()
        .timestamp_millis();
    assert!(ts > 0);
}

/// JSearch: `job_google_link` is used as fallback when `job_apply_link` is null.
#[test]
fn jsearch_uses_google_link_when_apply_link_null() {
    let json = serde_json::json!({
        "status": "OK",
        "data": [{
            "job_id": "b",
            "job_title": "Has google link only",
            "employer_name": "Co",
            "job_city": null,
            "job_country": null,
            "job_apply_link": null,
            "job_google_link": "https://google.com/jobs/b",
            "job_description": null,
            "job_posted_at_datetime_utc": null
        }]
    });

    let resp: JSearchResp = serde_json::from_value(json).unwrap();
    let j = &resp.data[0];
    // The fallback logic: apply_link.or_else(|| google_link) must yield the google link.
    let url = j
        .job_apply_link
        .clone()
        .or_else(|| j.job_google_link.clone());
    assert_eq!(
        url.as_deref(),
        Some("https://google.com/jobs/b"),
        "job_google_link must be used when job_apply_link is null"
    );
}

/// JSearch: jobs with neither `job_apply_link` nor `job_google_link` are dropped.
#[test]
fn jsearch_drops_jobs_without_any_link() {
    let json = serde_json::json!({
        "status": "OK",
        "data": [
            {
                "job_id": "a",
                "job_title": "Has apply link",
                "employer_name": "Co",
                "job_city": null,
                "job_country": null,
                "job_apply_link": "https://example.com/a",
                "job_google_link": null,
                "job_description": null,
                "job_posted_at_datetime_utc": null
            },
            {
                "job_id": "b",
                "job_title": "No link at all",
                "employer_name": "Co",
                "job_city": null,
                "job_country": null,
                "job_apply_link": null,
                "job_google_link": null,
                "job_description": null,
                "job_posted_at_datetime_utc": null
            }
        ]
    });

    let resp: JSearchResp = serde_json::from_value(json).unwrap();
    // Simulate the filter_map fallback: apply_link.or_else(|| google_link).
    let count = resp
        .data
        .into_iter()
        .filter(|j| j.job_apply_link.is_some() || j.job_google_link.is_some())
        .count();
    assert_eq!(count, 1, "job without either link must be dropped");
}

// ── Scraper trait basics ──────────────────────────────────────────────────────

#[test]
fn aggregator_scraper_id_and_display_name() {
    let s = AggregatorScraper;
    assert_eq!(s.id(), "aggregator");
    assert_eq!(s.display_name(), "Aggregated Jobs");
    assert_eq!(s.mode(), ScraperMode::Http);
    assert_eq!(s.auth(), AuthRequirement::Guest);
    assert!(!s.requires_company());
}

// ── is_configured() guard: unconfigured providers must return Err without a network call ──

#[tokio::test]
async fn adzuna_unconfigured_returns_err_without_network() {
    let p = AdzunaProvider {
        app_id: None,
        app_key: None,
    };
    let result = p
        .search("engineer", "berlin", "de", None, make_token())
        .await;
    assert!(result.is_err(), "unconfigured Adzuna must return Err");
    assert!(
        result.unwrap_err().to_string().contains("not configured"),
        "error must say 'not configured'"
    );
}

#[tokio::test]
async fn jsearch_unconfigured_returns_err_without_network() {
    let p = JSearchProvider { api_key: None };
    let result = p
        .search("engineer", "berlin", "de", None, make_token())
        .await;
    assert!(result.is_err(), "unconfigured JSearch must return Err");
    assert!(
        result.unwrap_err().to_string().contains("not configured"),
        "error must say 'not configured'"
    );
}

// ── Cancellation before fallback: cancelled signal must not fire JSearch ──────

/// Cancellation set before `search_with_providers` is called →
/// returns Ok(empty) immediately without touching any provider.
#[tokio::test]
async fn cancelled_before_search_returns_empty_no_provider_call() {
    let signal = make_token();
    signal.cancel();

    // JSearch is configured and would return items — must NOT be called.
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "should not be called")),
        Box::new(FakeProvider::ok(
            "jsearch",
            vec![sample_posting("1", "jsearch")],
        )),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, signal)
        .await
        .unwrap();
    assert!(
        result.is_empty(),
        "cancelled signal must prevent any provider call"
    );
}

/// A provider that returns an error AND cancels the supplied token during its
/// `search()` call, simulating a cancel that arrives between Adzuna's failure
/// and the JSearch fallback dispatch.
struct CancelOnSearchProvider {
    token: tokio_util::sync::CancellationToken,
}

#[async_trait::async_trait]
impl JobProvider for CancelOnSearchProvider {
    fn provider_id(&self) -> &'static str {
        "adzuna"
    }

    fn is_configured(&self) -> bool {
        true
    }

    async fn search(
        &self,
        _query: &str,
        _location: &str,
        _country: &str,
        _date_filter: Option<&str>,
        _signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Fail AND cancel so the fallback guard (not the top-of-function guard)
        // catches the cancellation before JSearch is called.
        self.token.cancel();
        Err(anyhow::anyhow!("adzuna: network timeout"))
    }
}

/// Adzuna errors and the token is cancelled during that call → the pre-fallback
/// cancel guard must prevent the paid JSearch call.
#[tokio::test]
async fn cancelled_after_adzuna_err_skips_jsearch() {
    let signal = make_token();

    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(CancelOnSearchProvider {
            token: signal.clone(),
        }),
        Box::new(FakeProvider::ok(
            "jsearch",
            vec![sample_posting("9", "jsearch")],
        )),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, signal)
        .await
        .unwrap();
    assert!(
        result.is_empty(),
        "JSearch must not be called when token is cancelled after Adzuna error"
    );
}

// ── Credential-read degradation: keyring failure / absence → None, never panic ──
//
// `AdzunaProvider::new()` / `JSearchProvider::new()` read OPTIONAL third-party API
// keys via `credentials::read_credential` and collapse BOTH `Err(_)` and
// `Ok(None)` to `None` (graceful degradation: log + treat as absent — never crash
// a user-triggered search over a missing optional key). These tests pin that
// construction-time degradation using keyring-core's in-memory mock store.
//
// The providers read FIXED slot names (`ai:adzuna-app-id`, …), unlike the
// UUID-isolated credentials tests, so these two tests serialize on a shared mutex
// and clean up after themselves to stay race-safe within the multi-thread test
// binary. The mock store install is the same process-wide `Once` the credentials
// tests use, so it is never swapped mid-run.
//
// The asserted slot strings are derived from the SAME generated source of truth
// the providers read (`ipc_contracts::provider_slots`) + the `ai:` namespace, so
// this test fails if a slot literal ever drifts from that single source.

use std::sync::Mutex;

use crate::ipc_contracts::provider_slots::{ADZUNA_APP_ID, ADZUNA_APP_KEY, JSEARCH_KEY};

static AGG_KEYRING_LOCK: Mutex<()> = Mutex::new(());

/// The aggregator's `ai:`-namespaced keyring slots, built from the generated
/// bare slot consts so the test asserts against the single cross-language source
/// of truth (drift in `provider_slots` flows straight through here).
fn adzuna_slots() -> [String; 2] {
    [
        format!("ai:{ADZUNA_APP_ID}"),
        format!("ai:{ADZUNA_APP_KEY}"),
    ]
}

fn jsearch_slot() -> String {
    format!("ai:{JSEARCH_KEY}")
}

/// Delete the aggregator's fixed keyring slots so a test starts from a known
/// "absent" baseline regardless of what a previous serialized test left behind.
fn clear_aggregator_slots() {
    let adzuna = adzuna_slots();
    let jsearch = jsearch_slot();
    for slot in adzuna.iter().chain(std::iter::once(&jsearch)) {
        if let Ok(entry) = keyring_core::Entry::new(crate::credentials::SERVICE, slot) {
            // NoEntry on a clean slot is fine; we only care it ends up absent.
            let _ = entry.delete_credential();
        }
    }
}

/// Absent keys (NoEntry → Ok(None) → None): both providers must construct with
/// `None` credentials and report `is_configured() == false`, so the aggregator
/// degrades to keyless-empty instead of panicking.
#[test]
fn providers_degrade_to_unconfigured_when_keys_absent() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    let adzuna = AdzunaProvider::new();
    assert!(adzuna.app_id.is_none(), "absent adzuna app-id must be None");
    assert!(
        adzuna.app_key.is_none(),
        "absent adzuna app-key must be None"
    );
    assert!(
        !adzuna.is_configured(),
        "Adzuna must be unconfigured when both keys are absent"
    );

    let jsearch = JSearchProvider::new();
    assert!(jsearch.api_key.is_none(), "absent jsearch key must be None");
    assert!(
        !jsearch.is_configured(),
        "JSearch must be unconfigured when the key is absent"
    );
}

/// Keyring read FAILURE (non-NoEntry → Err) must ALSO degrade to None at
/// construction (the provider's `.unwrap_or_else(|_| None)`), not propagate or
/// panic. We arm a non-NoEntry error on one Adzuna slot's mock `Cred`; the next
/// read of that slot returns it, exercising the `Err → None` branch.
#[test]
fn providers_degrade_to_unconfigured_on_keyring_error() {
    let _guard = AGG_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_aggregator_slots();

    // Arm a non-NoEntry failure on the app-id slot. `read_credential` maps it to
    // Err(AppError::Storage), which the provider collapses to None.
    let entry = keyring_core::Entry::new(crate::credentials::SERVICE, &adzuna_slots()[0]).unwrap();
    let mock: &keyring_core::mock::Cred = entry.as_any().downcast_ref().unwrap();
    mock.set_error(keyring_core::Error::Invalid(
        "induced".to_string(),
        "non-NoEntry keyring failure".to_string(),
    ));

    // Must NOT panic; the errored slot collapses to None → not configured.
    let adzuna = AdzunaProvider::new();
    assert!(
        adzuna.app_id.is_none(),
        "keyring error on app-id must degrade to None, not crash"
    );
    assert!(
        !adzuna.is_configured(),
        "Adzuna must be unconfigured when a key read errors"
    );

    clear_aggregator_slots();
}

// ── Date-filter mapping helpers ───────────────────────────────────────────────

#[test]
fn adzuna_max_days_old_maps_correctly() {
    // All sub-day windows collapse to 1 day (Adzuna's day granularity).
    assert_eq!(adzuna_max_days_old(Some("24h")), 1);
    assert_eq!(adzuna_max_days_old(Some("8h")), 1);
    assert_eq!(adzuna_max_days_old(Some("4h")), 1);
    assert_eq!(adzuna_max_days_old(Some("2h")), 1);
    assert_eq!(adzuna_max_days_old(Some("1h")), 1);
    assert_eq!(adzuna_max_days_old(Some("30m")), 1);
    assert_eq!(adzuna_max_days_old(Some("week")), 7);
    assert_eq!(adzuna_max_days_old(Some("month")), 30);
    // No filter or an unknown token caps at the past month (30 days).
    assert_eq!(adzuna_max_days_old(None), 30);
    assert_eq!(adzuna_max_days_old(Some("99y")), 30);
}

#[test]
fn jsearch_date_posted_maps_correctly() {
    // All sub-day windows collapse to "today" (JSearch's finest token).
    assert_eq!(jsearch_date_posted(Some("24h")), "today");
    assert_eq!(jsearch_date_posted(Some("8h")), "today");
    assert_eq!(jsearch_date_posted(Some("4h")), "today");
    assert_eq!(jsearch_date_posted(Some("2h")), "today");
    assert_eq!(jsearch_date_posted(Some("1h")), "today");
    assert_eq!(jsearch_date_posted(Some("30m")), "today");
    assert_eq!(jsearch_date_posted(Some("week")), "week");
    assert_eq!(jsearch_date_posted(Some("month")), "month");
    // No filter or an unknown token caps at the past month.
    assert_eq!(jsearch_date_posted(None), "month");
    assert_eq!(jsearch_date_posted(Some("99y")), "month");
}

// ── Date-filter exhaustiveness: every generated TS token is handled ────────────
//
// `DATE_FILTER_OPTIONS` is the codegen'd mirror of the TS `DATE_FILTER_OPTIONS`
// (the single source of truth in `packages/shared/src/schemas/index.ts`). Both
// match arms fall through to a DEFAULT for an unknown token, so a NEW TS token
// would silently collapse to that default instead of getting a real mapping.
//
// This test pins the EXPECTED non-default mapping for every known token and
// iterates the generated list, asserting each token maps to its expected value
// for BOTH `adzuna_max_days_old` and `jsearch_date_posted`. A new TS token added
// without a Rust match arm (or without an entry here) FAILS this test, so the
// cross-language drift surfaces at `cargo test` rather than at runtime.

/// Expected `(adzuna_max_days_old, jsearch_date_posted)` for a known token, or
/// `None` if the token is unrecognised (which must FAIL — every generated token
/// is required to have a real, non-default mapping).
fn expected_mapping(token: &str) -> Option<(u32, &'static str)> {
    match token {
        "30m" | "1h" | "2h" | "4h" | "8h" | "24h" => Some((1, "today")),
        "week" => Some((7, "week")),
        "month" => Some((30, "month")),
        _ => None,
    }
}

/// The single token whose mapping is INTENDED to equal the no-filter default
/// pair (`adzuna_max_days_old(None)`, `jsearch_date_posted(None)`). Any OTHER
/// token collapsing to that pair is the silent-default bug this guard catches.
const INTENDED_DEFAULT_EQUAL_TOKEN: &str = "month";

#[test]
fn every_generated_date_filter_token_has_a_real_mapping() {
    // The no-filter default pair, read from the SAME mappers (not a literal), so
    // this guard tracks the real defaults even if they ever change.
    let default_pair = (adzuna_max_days_old(None), jsearch_date_posted(None));

    for &token in crate::ipc_contracts::date_filters::DATE_FILTER_OPTIONS {
        let (exp_days, exp_posted) = expected_mapping(token).unwrap_or_else(|| {
            panic!(
                "generated date-filter token {token:?} has no expected mapping — a new TS token \
                 was added without a Rust match arm in `adzuna_max_days_old` / `jsearch_date_posted`"
            )
        });

        assert_eq!(
            adzuna_max_days_old(Some(token)),
            exp_days,
            "adzuna_max_days_old({token:?}) must map to its expected value, not the default"
        );
        assert_eq!(
            jsearch_date_posted(Some(token)),
            exp_posted,
            "jsearch_date_posted({token:?}) must map to its expected value, not the default"
        );

        // Companion guard: a token whose mapping equals BOTH no-filter defaults at
        // once has silently collapsed to the default in both arms. That is only
        // legitimate for the one documented default-equal token; any other token
        // doing so (e.g. a future token given a default-equal expected mapping by
        // mistake) FAILS here even though its expected-mapping assertion passed.
        let token_pair = (
            adzuna_max_days_old(Some(token)),
            jsearch_date_posted(Some(token)),
        );
        if token != INTENDED_DEFAULT_EQUAL_TOKEN {
            assert_ne!(
                token_pair, default_pair,
                "date-filter token {token:?} maps to the no-filter default pair {default_pair:?} \
                 in BOTH arms — it has silently collapsed to the default instead of getting a real \
                 mapping (only {INTENDED_DEFAULT_EQUAL_TOKEN:?} may equal the default pair)"
            );
        }
    }
}

// ── Adzuna country allowlist ──────────────────────────────────────────────────

/// Every code in ADZUNA_SUPPORTED_COUNTRIES must be accepted by `adzuna_supports_country`.
#[test]
fn adzuna_supported_countries_all_accepted() {
    for &cc in ADZUNA_SUPPORTED_COUNTRIES {
        assert!(
            adzuna_supports_country(cc),
            "country '{cc}' is in the allowlist but adzuna_supports_country returned false"
        );
    }
}

/// Codes not in the allowlist must be rejected (case-sensitive — country is
/// lowercased at the `AggregatorScraper::search` call site, line ~504).
#[test]
fn adzuna_unsupported_countries_rejected() {
    for cc in &["xx", "yy", "kp", "ir", "ru", "cn", "jp", "GB", "US"] {
        assert!(
            !adzuna_supports_country(cc),
            "'{cc}' should not be in the Adzuna allowlist"
        );
    }
}

/// Empty country string resolves to "de" (the Adzuna default) and must pass
/// the allowlist check. Calls the real `AdzunaProvider::search` with an empty
/// country string to prove the production `if country.is_empty() { "de" }` guard
/// fires and that the resulting error is NOT the allowlist-rejection error.
#[tokio::test]
async fn adzuna_empty_country_resolves_to_supported_de() {
    let p = AdzunaProvider {
        app_id: Some("fake-id".to_string()),
        app_key: Some("fake-key".to_string()),
    };
    // Empty country → production code resolves to "de" → passes allowlist → fails
    // downstream at the network/auth layer (no real keys), NOT at country validation.
    let result = p.search("engineer", "Berlin", "", None, make_token()).await;
    let e = result.unwrap_err();
    let msg = e.to_string();
    assert!(
        !msg.contains("not in Adzuna's supported market list"),
        "empty country must resolve to 'de' and pass the allowlist; \
         got allowlist-rejection error instead: {msg}"
    );
}

/// Unsupported country + JSearch configured → JSearch used (transparent fallback).
#[tokio::test]
async fn unsupported_country_with_jsearch_falls_back_to_jsearch() {
    let jsearch_posting = sample_posting("js1", "jsearch");

    // AdzunaProvider::search returns Err for unsupported countries; simulate that
    // with a FakeProvider that errors, which is the exact path AdzunaProvider takes.
    let providers_with_adzuna_err: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err(
            "adzuna",
            "adzuna: country 'xx' is not in Adzuna's supported market list",
        )),
        Box::new(FakeProvider::ok("jsearch", vec![jsearch_posting.clone()])),
    ];

    let result = search_with_providers(
        &providers_with_adzuna_err,
        "engineer",
        "Seoul",
        "xx",
        None,
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, jsearch_posting.external_id);
}

/// Unsupported country + Adzuna configured + NO JSearch → diagnostic Err,
/// not silent Ok(empty). This is the key UX regression test.
#[tokio::test]
async fn unsupported_country_no_jsearch_returns_diagnostic_err() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err(
            "adzuna",
            "adzuna: country 'xx' is not in Adzuna's supported market list",
        )),
        Box::new(FakeProvider::unconfigured("jsearch")),
    ];

    let result =
        search_with_providers(&providers, "engineer", "Seoul", "xx", None, make_token()).await;

    assert!(
        result.is_err(),
        "unsupported country with no JSearch fallback must return Err, not silent Ok(empty)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("adzuna:")
            && msg.contains("'xx'")
            && msg.contains("not in Adzuna's supported market list"),
        "diagnostic error must name the provider ('adzuna:'), the country code (\"'xx'\"), \
         and the supported-market-list phrase; got: {msg}"
    );
}

/// Supported country → Adzuna used normally (allowlist does not interfere).
#[tokio::test]
async fn supported_country_uses_adzuna_normally() {
    let adzuna_posting = sample_posting("de1", "adzuna");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![adzuna_posting.clone()])),
        Box::new(FakeProvider::err("jsearch", "should not be called")),
    ];

    let result = search_with_providers(&providers, "engineer", "Berlin", "de", None, make_token())
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, adzuna_posting.external_id);
}

/// Neither configured + unsupported country → keyless-empty (no keys = no
/// diagnostic; the user hasn't set up any provider at all).
#[tokio::test]
async fn unsupported_country_no_keys_returns_keyless_empty() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::unconfigured("adzuna")),
        Box::new(FakeProvider::unconfigured("jsearch")),
    ];

    let result = search_with_providers(&providers, "engineer", "Seoul", "xx", None, make_token())
        .await
        .unwrap();

    assert!(
        result.is_empty(),
        "no keys at all must still return keyless-empty (no diagnostic needed)"
    );
}

/// `AdzunaProvider::search` returns Err for an unsupported country without
/// making any network call — confirmed by the fact that the provider has
/// valid-looking (non-None) credentials but the country check fires first.
#[tokio::test]
async fn adzuna_provider_rejects_unsupported_country_before_network() {
    let p = AdzunaProvider {
        app_id: Some("fake-id".to_string()),
        app_key: Some("fake-key".to_string()),
    };
    // "xx" is not in the allowlist.
    let result = p
        .search("engineer", "Seoul", "xx", None, make_token())
        .await;
    assert!(
        result.is_err(),
        "AdzunaProvider must Err for unsupported country without a network call"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not in Adzuna's supported market list"),
        "error must mention the allowlist; got: {msg}"
    );
    assert!(
        msg.contains("JSearch"),
        "error must mention JSearch as the remedy; got: {msg}"
    );
}

/// `AdzunaProvider::search` accepts a supported country and proceeds past the
/// allowlist check (it will fail further on network/auth, not on country).
#[tokio::test]
async fn adzuna_provider_accepts_supported_country_passes_allowlist() {
    let p = AdzunaProvider {
        app_id: Some("fake-id".to_string()),
        app_key: Some("fake-key".to_string()),
    };
    // "de" is in the allowlist; the error that comes back must NOT mention the
    // allowlist — it should be a network/auth error (or similar), not a country error.
    let result = p
        .search("engineer", "Berlin", "de", None, make_token())
        .await;
    // We expect an error (no real API key) — an unexpected Ok would mean the test
    // environment somehow hit the real API, which must not silently pass unnoticed.
    let e = result.unwrap_err();
    assert!(
        !e.to_string()
            .contains("not in Adzuna's supported market list"),
        "supported country 'de' must pass the allowlist check; got: {e}"
    );
}

// ── Apify LinkedIn provider ────────────────────────────────────────────────────

/// Helper: a fully-formed Apify provider with explicit fields (no keyring read).
fn apify(token: Option<&str>, enabled: bool) -> ApifyLinkedInProvider {
    ApifyLinkedInProvider {
        token: token.map(str::to_string),
        enabled,
        actor_id: APIFY_DEFAULT_ACTOR.to_string(),
    }
}

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
        .search("engineer", "berlin", "de", None, make_token())
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

// ── Additive merge / dedup ─────────────────────────────────────────────────────

/// Apify results merge ADDITIVELY onto the primary result (not as a fallback) and
/// dedupe by URL: a LinkedIn item sharing the primary's URL is dropped; the
/// primary keeps its first-seen position.
#[tokio::test]
async fn apify_merges_additively_and_dedupes_by_url() {
    let primary = sample_posting("1", "adzuna");
    let li_unique = sample_posting("2", "linkedin");
    // Same URL as the primary, but a different external_id → must dedupe out.
    let mut li_dup = sample_posting("3", "linkedin");
    li_dup.url = primary.url.clone();

    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![primary.clone()])),
        Box::new(FakeProvider::ok(
            "apify_linkedin",
            vec![li_unique.clone(), li_dup.clone()],
        )),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, make_token())
        .await
        .unwrap();

    assert_eq!(result.len(), 2, "primary + unique LinkedIn; dup dropped");
    // Deterministic order: primary first, then LinkedIn.
    assert_eq!(result[0].url, primary.url);
    assert_eq!(result[1].url, li_unique.url);
}

/// Only Apify configured (no Adzuna/JSearch) → its results are returned (primary
/// chain yields keyless-empty, LinkedIn merges onto it).
#[tokio::test]
async fn only_apify_configured_returns_apify_items() {
    let li = sample_posting("x", "linkedin");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::unconfigured("adzuna")),
        Box::new(FakeProvider::unconfigured("jsearch")),
        Box::new(FakeProvider::ok("apify_linkedin", vec![li.clone()])),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, make_token())
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, li.external_id);
}

/// Apify NOT configured → behaviour is identical to the legacy chain: the primary
/// result passes through untouched (here, the Adzuna-failed-no-JSearch diagnostic
/// Err is preserved, NOT swallowed by the merge path).
#[tokio::test]
async fn apify_unconfigured_preserves_primary_diagnostic_err() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "timeout")),
        Box::new(FakeProvider::unconfigured("jsearch")),
        Box::new(FakeProvider::unconfigured("apify_linkedin")),
    ];

    let result =
        search_with_providers(&providers, "engineer", "berlin", "de", None, make_token()).await;

    assert!(result.is_err(), "primary diagnostic Err must be preserved");
    assert!(result.unwrap_err().to_string().contains("timeout"));
}

/// Primary fails but Apify is configured and returns results → show the LinkedIn
/// results rather than hide them behind the primary diagnostic.
#[tokio::test]
async fn apify_results_override_primary_error_when_present() {
    let li = sample_posting("li", "linkedin");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "timeout")),
        Box::new(FakeProvider::unconfigured("jsearch")),
        Box::new(FakeProvider::ok("apify_linkedin", vec![li.clone()])),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, make_token())
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, li.external_id);
}

/// Cancellation before the call → no provider runs, including the paid Apify one.
#[tokio::test]
async fn apify_not_run_after_cancellation() {
    let signal = make_token();
    signal.cancel();

    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok(
            "adzuna",
            vec![sample_posting("1", "adzuna")],
        )),
        Box::new(FakeProvider::ok(
            "apify_linkedin",
            vec![sample_posting("2", "linkedin")],
        )),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", None, signal)
        .await
        .unwrap();

    assert!(
        result.is_empty(),
        "a cancelled signal must prevent any (especially the paid) provider call"
    );
}
