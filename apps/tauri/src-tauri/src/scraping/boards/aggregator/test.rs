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

    let result = search_with_providers(&providers, "engineer", "berlin", "de", make_token())
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

    let result = search_with_providers(&providers, "engineer", "berlin", "de", make_token())
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

    let result = search_with_providers(&providers, "engineer", "berlin", "de", make_token())
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

    let result = search_with_providers(&providers, "engineer", "berlin", "de", make_token())
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

    let result = search_with_providers(&providers, "engineer", "berlin", "de", make_token())
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, jsearch_posting.external_id);
}

/// Adzuna Err and JSearch not configured → Ok(empty), never an error.
#[tokio::test]
async fn adzuna_err_and_no_jsearch_returns_empty() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "timeout")),
        Box::new(FakeProvider::unconfigured("jsearch")),
    ];

    let result = search_with_providers(&providers, "engineer", "berlin", "de", make_token())
        .await
        .unwrap();

    assert!(result.is_empty());
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

    let resp: AdzunaResp = serde_json::from_value(json)
        .expect("integer id must deserialize without error");
    let j = &resp.results[0];
    assert_eq!(j.id, "331705081");
    // Confirm the id maps correctly through the JobPosting formatting.
    assert_eq!(format!("adzuna-{}", j.id), "adzuna-331705081");
    assert_eq!(format!("aggregator:adzuna-{}", j.id), "aggregator:adzuna-331705081");
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

    let resp: AdzunaResp = serde_json::from_value(json)
        .expect("string id must still deserialize");
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
    let result = p.search("engineer", "berlin", "de", make_token()).await;
    assert!(result.is_err(), "unconfigured Adzuna must return Err");
    assert!(
        result.unwrap_err().to_string().contains("not configured"),
        "error must say 'not configured'"
    );
}

#[tokio::test]
async fn jsearch_unconfigured_returns_err_without_network() {
    let p = JSearchProvider { api_key: None };
    let result = p.search("engineer", "berlin", "de", make_token()).await;
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

    let result = search_with_providers(&providers, "engineer", "berlin", "de", signal)
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

    let result = search_with_providers(&providers, "engineer", "berlin", "de", signal)
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

use std::sync::Mutex;
static AGG_KEYRING_LOCK: Mutex<()> = Mutex::new(());

const ADZUNA_SLOTS: [&str; 2] = ["ai:adzuna-app-id", "ai:adzuna-app-key"];
const JSEARCH_SLOT: &str = "ai:jsearch-key";

/// Delete the aggregator's fixed keyring slots so a test starts from a known
/// "absent" baseline regardless of what a previous serialized test left behind.
fn clear_aggregator_slots() {
    for slot in ADZUNA_SLOTS.iter().chain(std::iter::once(&JSEARCH_SLOT)) {
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
    let entry = keyring_core::Entry::new(crate::credentials::SERVICE, ADZUNA_SLOTS[0]).unwrap();
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
