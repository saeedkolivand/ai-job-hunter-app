use super::super::*;
use super::support::*;

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
        SearchBudget::items_only(100),
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
        SearchBudget::items_only(100),
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

/// JSearch Ok(empty) → empty returned, Jooble NOT called. Symmetric with
/// Adzuna's own "configured Ok, even empty, wins" rule: a JSearch `Ok(vec![])`
/// is a DECISIVE (if empty) result, not "nothing" — it must short-circuit
/// before the Jooble last-resort tier, exactly like
/// `adzuna_ok_empty_does_not_call_jsearch` above one tier up.
///
/// Adzuna is UNCONFIGURED here (not `Ok(empty)`) so control actually reaches
/// the JSearch tier — an `Ok(empty)` Adzuna would short-circuit before JSearch
/// is ever consulted (see the sibling test above), which would make this test
/// pass for the wrong reason.
#[tokio::test]
async fn jsearch_ok_empty_does_not_call_jooble() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::unconfigured("adzuna")),
        Box::new(FakeProvider::ok("jsearch", vec![])),
        // Jooble is configured and would return items — but must not be called.
        Box::new(FakeProvider::err("jooble", "should not be called")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(
        result.len(),
        0,
        "Jooble must not be called when JSearch returns Ok(empty)"
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
        SearchBudget::items_only(100),
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
        SearchBudget::items_only(100),
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
        SearchBudget::items_only(100),
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
        SearchBudget::items_only(100),
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
        msg.contains("add a JSearch or Jooble key in Settings"),
        "diagnostic error must include the actionable fallback-remedy suffix; got: {msg}"
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
        note_sink: None,
        base_url: ADZUNA_BASE_URL.to_string(),
    };
    assert!(!unconfigured.is_configured());

    // Only one key → not configured.
    let partial = AdzunaProvider {
        app_id: Some("id123".to_string()),
        app_key: None,
        note_sink: None,
        base_url: ADZUNA_BASE_URL.to_string(),
    };
    assert!(!partial.is_configured());

    // Both keys → configured.
    let full = AdzunaProvider {
        app_id: Some("id123".to_string()),
        app_key: Some("key456".to_string()),
        note_sink: None,
        base_url: ADZUNA_BASE_URL.to_string(),
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

#[test]
fn jooble_is_configured_reflects_key_presence() {
    let no_key = JoobleProvider { api_key: None };
    assert!(!no_key.is_configured());

    let with_key = JoobleProvider {
        api_key: Some("joobleapikey".to_string()),
    };
    assert!(with_key.is_configured());
}
