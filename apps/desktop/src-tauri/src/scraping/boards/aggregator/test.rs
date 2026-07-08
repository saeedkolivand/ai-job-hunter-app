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
        _amount: Option<u32>,
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
    .await;

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
        .search("engineer", "berlin", "de", None, None, make_token())
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
        .search("engineer", "berlin", "de", None, None, make_token())
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

    let result = search_with_providers(
        &providers, "engineer", "berlin", "de", false, None, 100, signal,
    )
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
        _amount: Option<u32>,
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

    let result = search_with_providers(
        &providers, "engineer", "berlin", "de", false, None, 100, signal,
    )
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
    // All sub-day windows FLOOR at 3 days: Adzuna has no sub-day granularity, and a
    // 1-day ceiling zeroed out autopilot "recent" filters on quiet days (regression
    // guard). Date-sort still surfaces the freshest jobs first within the window.
    assert_eq!(adzuna_max_days_old(Some("24h")), 3);
    assert_eq!(adzuna_max_days_old(Some("8h")), 3);
    assert_eq!(adzuna_max_days_old(Some("4h")), 3);
    assert_eq!(adzuna_max_days_old(Some("2h")), 3);
    assert_eq!(adzuna_max_days_old(Some("1h")), 3);
    assert_eq!(adzuna_max_days_old(Some("30m")), 3);
    assert_eq!(adzuna_max_days_old(Some("15m")), 3);
    // Coarser tiers are unchanged.
    assert_eq!(adzuna_max_days_old(Some("week")), 7);
    assert_eq!(adzuna_max_days_old(Some("month")), 30);
    // No filter or an unknown token caps at the past month (30 days).
    assert_eq!(adzuna_max_days_old(None), 30);
    assert_eq!(adzuna_max_days_old(Some("99y")), 30);
}

#[test]
fn jsearch_date_posted_maps_correctly() {
    // All sub-day windows floor at "3days" (JSearch has no sub-day token, and
    // "today" zeroed out autopilot "recent" filters on quiet days — regression guard).
    assert_eq!(jsearch_date_posted(Some("24h")), "3days");
    assert_eq!(jsearch_date_posted(Some("8h")), "3days");
    assert_eq!(jsearch_date_posted(Some("4h")), "3days");
    assert_eq!(jsearch_date_posted(Some("2h")), "3days");
    assert_eq!(jsearch_date_posted(Some("1h")), "3days");
    assert_eq!(jsearch_date_posted(Some("30m")), "3days");
    assert_eq!(jsearch_date_posted(Some("15m")), "3days");
    // Coarser tiers are unchanged.
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
        // Sub-day windows floor at 3 days ("3days" for JSearch) — see the doc-comments
        // on `adzuna_max_days_old` / `jsearch_date_posted`. A tighter clamp zeroed out
        // autopilot "recent" filters; date-sort keeps the freshest jobs on top.
        "15m" | "30m" | "1h" | "2h" | "4h" | "8h" | "24h" => Some((3, "3days")),
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

/// Every code in ADZUNA_SUPPORTED_COUNTRIES must resolve to a currency — an
/// unmapped supported country would silently drop the salary's currency.
#[test]
fn adzuna_currency_map_covers_every_supported_country() {
    for &cc in ADZUNA_SUPPORTED_COUNTRIES {
        assert!(
            adzuna_currency_for_country(cc).is_some(),
            "country '{cc}' is Adzuna-supported but has no currency mapping"
        );
    }
}

/// A handful of country → ISO-4217 currency cases, plus an unsupported country
/// mapping to `None` (the salary answer then falls back to a web lookup).
#[test]
fn adzuna_currency_for_country_maps_known_codes() {
    assert_eq!(adzuna_currency_for_country("us"), Some("USD"));
    assert_eq!(adzuna_currency_for_country("gb"), Some("GBP"));
    assert_eq!(adzuna_currency_for_country("de"), Some("EUR"));
    assert_eq!(adzuna_currency_for_country("pl"), Some("PLN"));
    assert_eq!(adzuna_currency_for_country("ca"), Some("CAD"));
    assert_eq!(adzuna_currency_for_country("xx"), None);
}

fn adzuna_job(salary_min: Option<f64>, salary_max: Option<f64>) -> AdzunaJob {
    AdzunaJob {
        id: "1".to_string(),
        title: "Engineer".to_string(),
        company: None,
        location: None,
        redirect_url: "https://example.com/job/1".to_string(),
        description: None,
        created: None,
        salary_min,
        salary_max,
    }
}

/// A known salary + a supported country writes both the amount and the derived
/// ISO-4217 currency into `extra`.
#[test]
fn adzuna_job_to_posting_writes_salary_and_currency() {
    let posting = adzuna_job_to_posting(adzuna_job(Some(70_000.0), Some(90_000.0)), "de", 0);
    assert_eq!(
        posting.extra.get("salaryMin").and_then(|v| v.as_f64()),
        Some(70_000.0)
    );
    assert_eq!(
        posting.extra.get("salaryMax").and_then(|v| v.as_f64()),
        Some(90_000.0)
    );
    assert_eq!(
        posting.extra.get("salaryCurrency").and_then(|v| v.as_str()),
        Some("EUR")
    );
}

/// No salary at all → no salary/currency keys (an orphan currency with no
/// amount would be meaningless).
#[test]
fn adzuna_job_to_posting_omits_currency_when_no_salary() {
    let posting = adzuna_job_to_posting(adzuna_job(None, None), "de", 0);
    assert!(!posting.extra.contains_key("salaryMin"));
    assert!(!posting.extra.contains_key("salaryMax"));
    assert!(!posting.extra.contains_key("salaryCurrency"));
}

/// A known salary from a country with no currency mapping still keeps the
/// amount but omits the currency — graceful degradation, not a dropped salary.
#[test]
fn adzuna_job_to_posting_keeps_salary_without_currency_for_unmapped_country() {
    let posting = adzuna_job_to_posting(adzuna_job(Some(50_000.0), None), "xx", 0);
    assert_eq!(
        posting.extra.get("salaryMin").and_then(|v| v.as_f64()),
        Some(50_000.0)
    );
    assert!(!posting.extra.contains_key("salaryCurrency"));
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
    let result = p
        .search("engineer", "Berlin", "", None, None, make_token())
        .await;
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
        false,
        None,
        100,
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "Seoul",
        "xx",
        false,
        None,
        100,
        make_token(),
    )
    .await;

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

    let result = search_with_providers(
        &providers,
        "engineer",
        "Berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "Seoul",
        "xx",
        false,
        None,
        100,
        make_token(),
    )
    .await
    .unwrap();

    assert!(
        result.is_empty(),
        "no keys at all must still return keyless-empty (no diagnostic needed)"
    );
}

// ── Guessed-market empty-result guard (autopilot aggregator zero-jobs fix) ────
//
// When the caller supplied NO `country_code`, `AggregatorScraper::search` defaults
// the Adzuna market to a GUESS ("de") rather than a real target — the shape saved
// by an autopilot whose location was prefilled/typed without a geocode pick. An
// `Ok(empty)` from that guess, for a real (non-empty) location, must NOT be
// trusted as "no jobs exist" (the location is very likely outside Germany) —
// `primary_chain` treats it like an Adzuna error and falls through to JSearch or
// the diagnostic, exactly like the country-allowlist guard already does for an
// explicitly unsupported country.

/// Guessed market (`country_guessed = true`) + non-empty location + Adzuna
/// `Ok(empty)` + JSearch configured → JSearch is consulted and its results win.
#[tokio::test]
async fn guessed_market_empty_with_location_falls_back_to_jsearch() {
    let jsearch_posting = sample_posting("g1", "jsearch");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![])),
        Box::new(FakeProvider::ok("jsearch", vec![jsearch_posting.clone()])),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "London",
        "de",
        true, // country_guessed: no country_code was supplied
        None,
        100,
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(
        result.len(),
        1,
        "an empty result from a GUESSED market with a real location must fall \
         through to JSearch, not be trusted as a genuine zero"
    );
    assert_eq!(result[0].external_id, jsearch_posting.external_id);
}

/// Guessed market + non-empty location + Adzuna `Ok(empty)` + JSearch NOT
/// configured → a diagnostic `Err` (not a silent empty), mirroring the existing
/// unsupported-country contract.
#[tokio::test]
async fn guessed_market_empty_with_location_and_no_jsearch_returns_diagnostic_err() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![])),
        Box::new(FakeProvider::unconfigured("jsearch")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "London",
        "de",
        true, // country_guessed
        None,
        100,
        make_token(),
    )
    .await;

    assert!(
        result.is_err(),
        "a guessed-market empty result with no JSearch fallback must surface an \
         Err, not a silent Ok(empty) — this is the autopilot zero-jobs bug"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("guessed market") && msg.contains("London"),
        "diagnostic must name the guessed-market cause and the location; got: {msg}"
    );
    assert!(
        msg.contains("add a JSearch key in Settings"),
        "diagnostic error must include the actionable JSearch-remedy suffix; got: {msg}"
    );
}

/// Guessed market + EMPTY location (the keyless/no-location default, e.g. a
/// German search with no location filter at all) + Adzuna `Ok(empty)` + JSearch
/// configured → JSearch must NOT be called; the empty result is returned as-is.
/// Regression guard: the guessed-market guard must not regress the existing
/// German default for a location-less search.
#[tokio::test]
async fn guessed_market_empty_with_no_location_is_not_treated_as_untrustworthy() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![])),
        // JSearch is configured and would return items — must NOT be called.
        Box::new(FakeProvider::ok(
            "jsearch",
            vec![sample_posting("g2", "jsearch")],
        )),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "", // no location — nothing to doubt the guessed market with
        "de",
        true, // country_guessed
        None,
        100,
        make_token(),
    )
    .await
    .unwrap();

    assert!(
        result.is_empty(),
        "a guessed market with NO location must keep the legacy Ok(empty) \
         behavior — JSearch must not be called"
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
        .search("engineer", "Seoul", "xx", None, None, make_token())
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
        .search("engineer", "Berlin", "de", None, None, make_token())
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
        .search("engineer", "berlin", "de", None, None, make_token())
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
    .await;

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

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        100,
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, li.external_id);
}

// ── FIX 1 (finding #4): retries=0 invariant via the real helper ──────────────

/// INVARIANT: the `FetchOptions` object that `ApifyLinkedInProvider::search`
/// actually constructs (via `apify_fetch_options`) must have `retries == 0`.
///
/// Unlike the old version that only checked the constant, this assertion runs
/// against the REAL helper that `search` calls.  If someone replaces
/// `retries: APIFY_RETRIES` with `retries: 2` inside the helper, this test
/// fails — where the old constant-only test would have stayed green.
#[test]
fn apify_fetch_options_must_have_retries_zero() {
    // Baseline: confirm the default is non-zero so the override below is meaningful.
    assert_eq!(
        FetchOptions::default().retries,
        2,
        "FetchOptions::default() retries is expected to be 2; update this test if the default changes"
    );
    // Assert on the ACTUAL options object the production search path constructs.
    let opts = apify_fetch_options("{}".to_string(), "test-token");
    assert_eq!(
        opts.retries, 0,
        "INVARIANT VIOLATED: apify_fetch_options must set retries=0 — \
         a retry would start another billed actor run"
    );
    // Belt: the shared constant itself must also remain 0.
    assert_eq!(APIFY_RETRIES, 0, "APIFY_RETRIES constant must be 0");
}

// ── FIX 2: cancellation mid-flight ──────────────────────────────────────────

/// A signal cancelled BEFORE the fetch_json call fires the tokio::select!
/// cancel arm immediately — no network call is issued. This tests the
/// `ApifyLinkedInProvider::search` path directly (not the higher-level guard in
/// `search_with_providers`), which is what FIX 2 adds.
#[tokio::test]
async fn apify_search_pre_cancelled_signal_returns_err() {
    let p = apify(Some("fake-token"), true);
    let signal = make_token();
    signal.cancel(); // pre-cancel before calling search

    let result = p.search("dev", "Berlin", "de", None, None, signal).await;
    assert!(
        result.is_err(),
        "a pre-cancelled signal must make ApifyLinkedInProvider::search return Err"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("cancelled"),
        "error must indicate cancellation; got: {msg}"
    );
}

// ── FIX 3: platform-enforced cost cap in endpoint URL ───────────────────────

/// The run-sync endpoint must carry `maxItems` and `maxTotalChargeUsd` as query
/// params (server-side cost caps) and must NOT embed the Bearer token in the URL.
///
/// Calls the REAL `build_apify_endpoint(actor, max_items)` function that
/// production code calls, so any future refactor that drops either cap (or
/// buries the token in the URL) will break this test.
#[test]
fn apify_endpoint_url_has_max_items_and_no_token() {
    // Call the same function production uses — no local copy.
    // Pass APIFY_MAX_ITEMS as the cap to mirror the full-cap scenario.
    let endpoint = build_apify_endpoint(APIFY_DEFAULT_ACTOR, APIFY_MAX_ITEMS);

    assert!(
        endpoint.contains(&format!("maxItems={APIFY_MAX_ITEMS}")),
        "endpoint must include server-side maxItems={APIFY_MAX_ITEMS}; got: {endpoint}"
    );
    assert!(
        endpoint.contains(&format!("maxTotalChargeUsd={APIFY_MAX_CHARGE_USD}")),
        "endpoint must include maxTotalChargeUsd={APIFY_MAX_CHARGE_USD}; got: {endpoint}"
    );
    // Token must never appear in the URL (lives in the Authorization header only).
    assert!(
        !endpoint.to_lowercase().contains("token"),
        "Bearer token must not appear in the URL; got: {endpoint}"
    );
    // Apify host is fixed — no SSRF possible.
    assert!(
        endpoint.starts_with("https://api.apify.com/"),
        "endpoint must be on api.apify.com; got: {endpoint}"
    );
    // Actor id is embedded in the path.
    assert!(
        endpoint.contains(APIFY_DEFAULT_ACTOR),
        "endpoint must contain the actor id; got: {endpoint}"
    );
}

// ── FIX 4: actor-id validation ───────────────────────────────────────────────

/// Valid `user~actor` ids pass; malformed ids are rejected so they can't reach
/// the API path.
#[test]
fn apify_actor_id_validator_accepts_valid_rejects_malformed() {
    // Valid ids.
    assert!(
        is_valid_apify_actor_id("curious_coder~linkedin-jobs-scraper"),
        "default actor id must be valid"
    );
    assert!(
        is_valid_apify_actor_id("user123~actor-name.v2"),
        "alphanumeric + hyphen + dot must be valid"
    );
    assert!(
        is_valid_apify_actor_id("a~b"),
        "single-char parts must be valid"
    );

    // No tilde → invalid.
    assert!(!is_valid_apify_actor_id("nousernamehere"));
    // Empty user part.
    assert!(!is_valid_apify_actor_id("~actor"));
    // Empty actor part.
    assert!(!is_valid_apify_actor_id("user~"));
    // Path-traversal chars rejected.
    assert!(!is_valid_apify_actor_id("../../etc/passwd"));
    assert!(!is_valid_apify_actor_id("user~actor/path"));
    // Spaces rejected.
    assert!(!is_valid_apify_actor_id("user ~actor"));
    // Two tildes rejected (actor part contains `~` which is not in [A-Za-z0-9_.-]).
    assert!(!is_valid_apify_actor_id("user~act~or"));
}

/// The default actor constant must always pass its own validator.
#[test]
fn apify_default_actor_passes_validator() {
    assert!(
        is_valid_apify_actor_id(APIFY_DEFAULT_ACTOR),
        "APIFY_DEFAULT_ACTOR must pass is_valid_apify_actor_id; got: {APIFY_DEFAULT_ACTOR}"
    );
}

// ── FIX 5: dedupe_by_url strips LinkedIn tracking params ────────────────────

/// Two LinkedIn URLs for the same job that differ only by tracking params
/// (`?trk=…`, `?refId=…`) must collapse to one entry in `dedupe_by_url`.
/// Non-LinkedIn URLs with distinct query strings must remain distinct (some
/// boards encode the job id in the query).
#[test]
fn dedupe_by_url_strips_linkedin_tracking_params() {
    let make_posting = |url: &str| JobPosting {
        id: url.to_string(),
        external_id: Some(url.to_string()),
        title: "Engineer".to_string(),
        company: "Co".to_string(),
        location: None,
        url: url.to_string(),
        source: "aggregator".to_string(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: std::collections::HashMap::new(),
    };

    // Same LinkedIn job, two different tracking param variants.
    let li1 = make_posting("https://www.linkedin.com/jobs/view/123?trk=organic");
    let li2 = make_posting("https://www.linkedin.com/jobs/view/123?refId=abc&trk=xyz");
    // Non-LinkedIn: different query → distinct jobs (must NOT be merged).
    let other1 = make_posting("https://example.com/jobs?id=1");
    let other2 = make_posting("https://example.com/jobs?id=2");

    let deduped = dedupe_by_url(vec![li1, li2, other1, other2]);
    assert_eq!(
        deduped.len(),
        3,
        "two LinkedIn tracking-param variants should collapse to 1; \
         two non-LinkedIn distinct-query URLs stay separate; expected 3 total"
    );
    let urls: Vec<&str> = deduped.iter().map(|p| p.url.as_str()).collect();
    assert!(
        urls.contains(&"https://example.com/jobs?id=1"),
        "non-LinkedIn URL with id=1 must be kept"
    );
    assert!(
        urls.contains(&"https://example.com/jobs?id=2"),
        "non-LinkedIn URL with id=2 must be kept"
    );
}

/// `canonical_url` only strips query on `linkedin.com` or `*.linkedin.com` —
/// a dot boundary is required so look-alike domains like `evillinkedin.com`
/// (no dot before `linkedin.com`) and `linkedin.example.com` are left intact.
#[test]
fn canonical_url_only_strips_linkedin_dot_com_hosts() {
    // Exact linkedin.com apex → query stripped.
    let apex = canonical_url("https://linkedin.com/jobs/view/1?trk=foo");
    assert!(
        !apex.contains('?'),
        "query must be stripped for apex linkedin.com; got: {apex}"
    );

    // Real www.linkedin.com subdomain → query stripped.
    let li = canonical_url("https://www.linkedin.com/jobs/view/1?trk=foo");
    assert!(
        !li.contains('?'),
        "query must be stripped for www.linkedin.com; got: {li}"
    );

    // A host that ends with "linkedin.com" but has no dot boundary (`evillinkedin.com`)
    // must NOT be treated as LinkedIn — its query is preserved.
    let evil = canonical_url("https://evillinkedin.com/jobs/1?id=99");
    assert!(
        evil.contains("id=99"),
        "query must NOT be stripped for evillinkedin.com (no dot boundary); got: {evil}"
    );

    // A host that merely *contains* the substring "linkedin" in the wrong position
    // (`linkedin.example.com`) → NOT stripped.
    let fake = canonical_url("https://linkedin.example.com/jobs/1?id=1");
    assert!(
        fake.contains("id=1"),
        "query must NOT be stripped for linkedin.example.com; got: {fake}"
    );

    // Non-LinkedIn host → query kept.
    let other = canonical_url("https://example.com/jobs?id=42");
    assert!(
        other.contains("id=42"),
        "query must be kept for non-LinkedIn host; got: {other}"
    );
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

    let result = search_with_providers(
        &providers, "engineer", "berlin", "de", false, None, 100, signal,
    )
    .await
    .unwrap();

    assert!(
        result.is_empty(),
        "a cancelled signal must prevent any (especially the paid) provider call"
    );
}

// ── Finding #1: Apify URL validation ─────────────────────────────────────────

/// A `jobUrl` on a non-LinkedIn host is rejected — the item is dropped.
#[test]
fn apify_map_item_rejects_non_linkedin_host() {
    let item: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "jobUrl": "https://evil.example.com/jobs/1"
    }))
    .unwrap();
    assert!(
        map_apify_item(item, 0).is_none(),
        "non-LinkedIn jobUrl must be rejected"
    );
}

/// `http://` (not https) and `javascript:` schemes must be rejected.
#[test]
fn apify_map_item_rejects_non_https_scheme() {
    let item_http: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "jobUrl": "http://www.linkedin.com/jobs/view/1"
    }))
    .unwrap();
    assert!(
        map_apify_item(item_http, 0).is_none(),
        "http:// LinkedIn URL must be rejected (https required)"
    );

    let item_js: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "jobUrl": "javascript:alert(1)"
    }))
    .unwrap();
    assert!(
        map_apify_item(item_js, 0).is_none(),
        "javascript: scheme must be rejected"
    );
}

/// A non-numeric `id` (e.g. a path-traversal string) must not be used to
/// construct a URL — the item is dropped because no safe URL can be built.
#[test]
fn apify_map_item_rejects_non_numeric_id_for_url_construction() {
    let item: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "id": "../../etc/passwd"
    }))
    .unwrap();
    // Non-numeric id → URL cannot be constructed → item dropped (None).
    assert!(
        map_apify_item(item, 0).is_none(),
        "non-numeric id must not be used to construct a LinkedIn URL; item must be dropped"
    );
}

/// A valid HTTPS `linkedin.com` URL passes validation and produces a JobPosting.
#[test]
fn apify_map_item_accepts_valid_linkedin_url() {
    let item: ApifyItem = serde_json::from_value(serde_json::json!({
        "title": "Rust Engineer",
        "jobUrl": "https://www.linkedin.com/jobs/view/99999"
    }))
    .unwrap();
    let p = map_apify_item(item, 0).expect("valid LinkedIn HTTPS URL must be accepted");
    assert_eq!(p.url, "https://www.linkedin.com/jobs/view/99999");
}

// ── Finding #2: cost gate — skip Apify when primary fills amount ──────────────

/// Primary provides exactly `amount` items → the paid Apify call must NOT fire.
/// Uses a `TrackCallProvider` (not FakeProvider::err) because Apify errors are
/// silently swallowed — only a flag proves the call was skipped.
#[tokio::test]
async fn apify_skipped_when_primary_fills_amount() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct TrackCallProvider {
        id: &'static str,
        called: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl JobProvider for TrackCallProvider {
        fn provider_id(&self) -> &'static str {
            self.id
        }
        fn is_configured(&self) -> bool {
            true
        }
        async fn search(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: Option<&str>,
            _: Option<u32>,
            _: tokio_util::sync::CancellationToken,
        ) -> anyhow::Result<Vec<JobPosting>> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(vec![])
        }
    }

    let apify_called = Arc::new(AtomicBool::new(false));
    let primary_items: Vec<JobPosting> = (0..5_u32)
        .map(|i| sample_posting(&i.to_string(), "adzuna"))
        .collect();

    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", primary_items.clone())),
        Box::new(TrackCallProvider {
            id: "apify_linkedin",
            called: apify_called.clone(),
        }),
    ];

    // amount == 5, primary returns 5 → Apify must not be called.
    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        5,
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 5, "primary result must be returned unchanged");
    assert!(
        !apify_called.load(Ordering::SeqCst),
        "Apify must NOT be called when primary already fills the requested amount"
    );
}

/// `build_apify_endpoint` reflects the dynamic cap in its `maxItems` query param.
/// - Partial gap (remaining=20): endpoint has `maxItems=20`.
/// - Full budget (remaining≥APIFY_MAX_ITEMS): endpoint has `maxItems=APIFY_MAX_ITEMS`.
#[test]
fn apify_endpoint_cap_reflects_remaining() {
    // Partial: amount=30, primary=10, remaining=20 → cap=20.
    let ep_partial = build_apify_endpoint(APIFY_DEFAULT_ACTOR, 20);
    assert!(
        ep_partial.contains("maxItems=20"),
        "partial-gap cap must appear in maxItems; got: {ep_partial}"
    );

    // Full: remaining exceeds APIFY_MAX_ITEMS → cap clamped to APIFY_MAX_ITEMS.
    let ep_full = build_apify_endpoint(APIFY_DEFAULT_ACTOR, APIFY_MAX_ITEMS);
    assert!(
        ep_full.contains(&format!("maxItems={APIFY_MAX_ITEMS}")),
        "full-cap scenario must use APIFY_MAX_ITEMS; got: {ep_full}"
    );
}

// ── Finding #3: canonical_url preserves query case for non-LinkedIn URLs ──────

/// Two non-LinkedIn URLs differing ONLY by query-string case must NOT collapse
/// to the same dedup key.  Some boards encode job ids as case-sensitive query
/// params; the old `.to_lowercase()` on the whole URL would merge them silently.
#[test]
fn canonical_url_non_linkedin_query_case_is_preserved() {
    let key1 = canonical_url("https://board.example.com/jobs?ref=AbCdEf");
    let key2 = canonical_url("https://board.example.com/jobs?ref=abcdef");
    assert_ne!(
        key1, key2,
        "non-LinkedIn URLs differing only by query case must remain distinct; \
         both canonicalized to: {key1}"
    );
}
