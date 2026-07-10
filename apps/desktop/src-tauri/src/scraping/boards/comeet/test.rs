use super::*;
use std::sync::Mutex;

use crate::ipc_contracts::provider_slots::{COMEET_API_TOKEN, COMEET_COMPANY_UID};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_input() -> BoardSearchInput {
    BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: vec![],
    }
}

fn make_ctx() -> ScrapeContext {
    ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
    }
}

// ---------------------------------------------------------------------------
// Scraper metadata
// ---------------------------------------------------------------------------

#[test]
fn test_comeet_scraper_id() {
    assert_eq!(ComeetScraper.id(), "comeet");
}

#[test]
fn test_comeet_scraper_display_name() {
    assert_eq!(ComeetScraper.display_name(), "Comeet");
}

#[test]
fn test_comeet_scraper_mode() {
    assert_eq!(ComeetScraper.mode(), ScraperMode::Http);
}

#[test]
fn test_comeet_auth_is_guest() {
    assert_eq!(
        ComeetScraper.auth(),
        AuthRequirement::Guest,
        "credentials are read internally, not via the board-login connect flow"
    );
}

#[test]
fn test_comeet_does_not_require_company() {
    assert!(
        !ComeetScraper.requires_company(),
        "comeet is scoped by a fixed credential, not a per-search companies[] input"
    );
}

// ---------------------------------------------------------------------------
// URL guard — is_valid_comeet_url (host-lock)
// ---------------------------------------------------------------------------

#[test]
fn url_guard_accepts_comeet_host_and_subdomains() {
    assert!(is_valid_comeet_url(
        "https://www.comeet.co/jobs/acme/12.345/Backend-Engineer/AB.CDE"
    ));
    assert!(is_valid_comeet_url("https://acme.comeet.co/jobs/12.345"));
}

#[test]
fn url_guard_rejects_off_host_and_non_https() {
    assert!(
        !is_valid_comeet_url("http://www.comeet.co/jobs/1"),
        "non-https rejected"
    );
    assert!(
        !is_valid_comeet_url("https://evil.example/jobs/1"),
        "off-host rejected"
    );
    assert!(
        !is_valid_comeet_url("https://evil-comeet.co/jobs/1"),
        "lookalike host must be rejected (label-boundary anchored)"
    );
    assert!(
        !is_valid_comeet_url("not-a-url"),
        "unparseable url rejected"
    );
}

// ---------------------------------------------------------------------------
// parse_comeet_time
// ---------------------------------------------------------------------------

#[test]
fn parse_comeet_time_accepts_unix_seconds_and_rfc3339() {
    assert_eq!(parse_comeet_time("1700000000"), Some(1_700_000_000_000));
    let rfc3339 = parse_comeet_time("2023-11-14T22:13:20Z");
    assert!(rfc3339.is_some());
    assert!(parse_comeet_time("not-a-time").is_none());
}

// ---------------------------------------------------------------------------
// rows_to_jobs — per-row resilience
// ---------------------------------------------------------------------------

#[test]
fn rows_to_jobs_skips_malformed_rows_keeps_valid_ones() {
    let values: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"name": "Valid", "uid": "abc", "url_comeet_hosted_page": "https://www.comeet.co/jobs/1"},
            {"name": "Bad location shape", "uid": "def", "location": "not-an-object"},
            {"name": "Also Valid", "uid": "ghi", "url_active_page": "https://www.comeet.co/jobs/2"}
        ]"#,
    )
    .unwrap();
    let jobs = rows_to_jobs(values);
    assert_eq!(
        jobs.len(),
        2,
        "the row with a malformed 'location' type must be dropped, valid rows kept"
    );
}

#[test]
fn rows_to_jobs_empty_input_returns_empty() {
    assert!(rows_to_jobs(vec![]).is_empty());
}

// ---------------------------------------------------------------------------
// parse_comeet_response — fixture-based parsing (career-ops-shaped)
// ---------------------------------------------------------------------------

#[test]
fn parse_comeet_response_happy_path_falls_back_to_company_uid() {
    let json = r#"[
        {
            "name": "Backend Engineer",
            "uid": "AB.CDE",
            "url_comeet_hosted_page": "https://www.comeet.co/jobs/acme/12.345/Backend-Engineer/AB.CDE",
            "location": {"name": null, "city": "Tel Aviv", "country": "Israel"},
            "time_updated": "1700000000"
        }
    ]"#;
    let positions: Vec<CmPosition> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_comeet_response(positions, "acme-uid", 1_700_000_000_000);

    assert_eq!(postings.len(), 1);
    let p = &postings[0];
    assert_eq!(p.title, "Backend Engineer");
    assert_eq!(
        p.url,
        "https://www.comeet.co/jobs/acme/12.345/Backend-Engineer/AB.CDE"
    );
    // No company_name in the fixture (unconfirmed field) → falls back to the uid.
    assert_eq!(p.company, "acme-uid");
    assert_eq!(p.location, Some("Tel Aviv, Israel".to_string()));
    assert_eq!(p.id, "comeet:AB.CDE");
    assert_eq!(p.external_id, Some("AB.CDE".to_string()));
    assert_eq!(p.source, "comeet");
    assert_eq!(p.posted_at, Some(1_700_000_000_000));
    assert_eq!(p.captured_at, 1_700_000_000_000);
}

#[test]
fn parse_comeet_response_uses_company_name_when_present() {
    let json = r#"[
        {
            "name": "Frontend Engineer",
            "uid": "XY.Z12",
            "url_active_page": "https://www.comeet.co/jobs/acme/12.345/Frontend-Engineer/XY.Z12",
            "company": "Acme Inc"
        }
    ]"#;
    let positions: Vec<CmPosition> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_comeet_response(positions, "acme-uid", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].company, "Acme Inc");
}

#[test]
fn parse_comeet_response_uses_location_name_over_city_country() {
    let json = r#"[
        {"name": "Role", "uid": "L1", "url_active_page": "https://www.comeet.co/jobs/1", "location": {"name": "Remote - Worldwide", "city": "Tel Aviv", "country": "Israel"}}
    ]"#;
    let positions: Vec<CmPosition> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_comeet_response(positions, "uid", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].location, Some("Remote - Worldwide".to_string()));
}

#[test]
fn parse_comeet_response_empty_positions_returns_empty_vec() {
    assert!(parse_comeet_response(vec![], "uid", 0).is_empty());
}

/// Missing title, missing uid, and an off-host url each drop the row; a valid
/// row in the same payload must still come through.
#[test]
fn parse_comeet_response_drops_malformed_rows() {
    let json = r#"[
        {"name": "Valid One", "uid": "OK1", "url_active_page": "https://www.comeet.co/jobs/1"},
        {"name": null, "uid": "OK2", "url_active_page": "https://www.comeet.co/jobs/2"},
        {"name": "Missing UID", "uid": null, "url_active_page": "https://www.comeet.co/jobs/3"},
        {"name": "Off Host", "uid": "OK4", "url_active_page": "https://evil.example/jobs/4"},
        {"name": "Valid Two", "uid": "OK5", "url_active_page": "https://www.comeet.co/jobs/5"}
    ]"#;
    let positions: Vec<CmPosition> = rows_to_jobs(serde_json::from_str(json).unwrap());
    let postings = parse_comeet_response(positions, "uid", 0);
    let titles: Vec<&str> = postings.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Valid One", "Valid Two"],
        "malformed rows must be dropped without panicking, valid rows kept: {titles:?}"
    );
}

// ---------------------------------------------------------------------------
// search() — credential-gated behavior (hermetic mock keyring, no real network)
// ---------------------------------------------------------------------------

// Process-wide mock keyring install + a local lock, same idiom as the
// aggregator's `AGG_KEYRING_LOCK`/`install_mock_keyring` — see that module's
// test.rs for the full rationale (keyring_core::set_default_store is
// process-global; tests serialize on this lock instead of swapping stores).
static COMEET_KEYRING_LOCK: Mutex<()> = Mutex::new(());

fn comeet_slots() -> [String; 2] {
    [
        format!("ai:{COMEET_COMPANY_UID}"),
        format!("ai:{COMEET_API_TOKEN}"),
    ]
}

fn clear_comeet_slots() {
    for slot in comeet_slots() {
        if let Ok(entry) = keyring_core::Entry::new(crate::credentials::SERVICE, &slot) {
            let _ = entry.delete_credential();
        }
    }
}

/// Build a current-thread runtime for the credential-gated tests below. Each
/// test needs to hold `COMEET_KEYRING_LOCK` (a std `Mutex`, since the mock
/// keyring install is process-global state, not async state) across an
/// `await` — `#[tokio::test]` would make the whole test body async and trip
/// `clippy::await_holding_lock` on the strict pre-push lint
/// (`--all-targets --all-features -D warnings`). Using a plain `#[test]` +
/// `block_on` instead keeps the test function itself sync (the lock guard is
/// released before any `.await` point crosses a suspend boundary observable
/// to clippy's lint), while `block_on` still drives the real async
/// `search()` — correctness is unchanged, only the harness shape. Same
/// resolution the aggregator's own lock-holding tests use (theirs just never
/// needed to await while holding the lock in the first place).
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

/// No credentials configured → keyless-empty, same contract as the
/// aggregator's Adzuna/JSearch/Apify providers: never an error just because
/// the user hasn't added keys yet, and no real network call is attempted.
#[test]
fn search_returns_empty_without_error_when_credentials_absent() {
    let _guard = COMEET_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_comeet_slots();

    let result = block_on(ComeetScraper.search(make_input(), make_ctx()));
    assert!(result.is_ok(), "missing credentials must be Ok, not Err");
    assert!(
        result.unwrap().is_empty(),
        "missing credentials must be keyless-empty"
    );
}

/// Only one of the two credentials present (company uid but no token) must
/// still be treated as "not configured" — BOTH are mandatory, mirroring the
/// Apify provider's `token.is_some() && enabled` two-gate contract.
#[test]
fn search_returns_empty_when_only_one_credential_present() {
    let _guard = COMEET_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_comeet_slots();

    let entry = keyring_core::Entry::new(crate::credentials::SERVICE, &comeet_slots()[0]).unwrap();
    entry.set_password("acme-uid").unwrap();

    let result = block_on(ComeetScraper.search(make_input(), make_ctx()));
    assert!(result.is_ok());
    assert!(
        result.unwrap().is_empty(),
        "a lone company-uid with no token must still be keyless-empty"
    );

    clear_comeet_slots();
}

/// A pre-cancelled signal, even with both credentials present, must not fire
/// the network fetch and must return Ok(empty) — cancellation is checked
/// after the credential gate but before the request is built.
#[test]
fn search_cancelled_after_credentials_present_returns_ok_empty() {
    let _guard = COMEET_KEYRING_LOCK.lock().unwrap();
    crate::credentials::install_mock_keyring();
    clear_comeet_slots();

    let uid_entry =
        keyring_core::Entry::new(crate::credentials::SERVICE, &comeet_slots()[0]).unwrap();
    uid_entry.set_password("acme-uid").unwrap();
    let token_entry =
        keyring_core::Entry::new(crate::credentials::SERVICE, &comeet_slots()[1]).unwrap();
    token_entry.set_password("secret-token").unwrap();

    let ctx = make_ctx();
    ctx.signal.cancel();
    let result = block_on(ComeetScraper.search(make_input(), ctx));
    assert!(
        result.is_ok(),
        "cancelled run must return Ok, not Err, and must not attempt network"
    );
    assert!(result.unwrap().is_empty());

    clear_comeet_slots();
}
