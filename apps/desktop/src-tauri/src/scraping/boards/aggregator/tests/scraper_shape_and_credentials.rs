use super::super::*;
use super::support::*;

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
        note_sink: None,
        base_url: ADZUNA_BASE_URL.to_string(),
    };
    let result = p
        .search("engineer", "berlin", "de", false, None, None, make_token())
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
        .search("engineer", "berlin", "de", false, None, None, make_token())
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
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(100),
        signal,
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
        _country_guessed: bool,
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
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(100),
        signal,
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
//
// The lock + slot-name/clear-slots fixtures live in `support.rs`
// (`AGG_KEYRING_LOCK`, `adzuna_slots`, …) — shared with `needs_keys_skip.rs`,
// which touches the same fixed keyring slots.

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
