use super::super::*;
use super::support::*;

/// Explicit country (`country_guessed == false`) + a sparse Adzuna result →
/// unchanged behavior: the sparse hits are authoritative and JSearch is NOT
/// consulted. The floor guard is guessed-market only.
#[tokio::test]
async fn explicit_country_sparse_does_not_fall_back() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok(
            "adzuna",
            vec![
                sample_posting("e1", "adzuna"),
                sample_posting("e2", "adzuna"),
            ],
        )),
        Box::new(FakeProvider::err(
            "jsearch",
            "must not be called for an explicit country",
        )),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "London",
        "gb",
        false, // explicit country — not guessed
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(
        result.len(),
        2,
        "explicit-country sparse results are authoritative; the floor guard is guessed-market only"
    );
}

/// Guessed market + non-empty location + a SPARSE Adzuna result (2 < floor) +
/// JSearch NOT configured → the sparse hits are the best available answer, so they
/// are RETURNED (a user with only Adzuna keys keeps their 2 legit results) rather
/// than discarded for a zero-results diagnostic Err. The uncertainty is logged.
#[tokio::test]
async fn guessed_market_sparse_no_fallback_returns_sparse_items() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok(
            "adzuna",
            vec![
                sample_posting("s1", "adzuna"),
                sample_posting("s2", "adzuna"),
            ],
        )),
        // No configured fallback.
        Box::new(FakeProvider::unconfigured("jsearch")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "London",
        "de",
        true, // country_guessed
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .expect("sparse guessed-market items must be returned, not an Err, with no fallback");

    assert_eq!(
        result.len(),
        2,
        "sparse guessed-market Adzuna items must be returned when there is no JSearch fallback"
    );
    assert!(
        result.iter().all(|p| p
            .external_id
            .as_deref()
            .unwrap_or("")
            .starts_with("adzuna-")),
        "the returned items must be the sparse Adzuna hits"
    );
}

/// Guessed market + non-empty location + a SPARSE Adzuna result + JSearch
/// configured but ERRORING → the fallback failed, so the sparse Adzuna hits are
/// returned (better than nothing) rather than surfacing the JSearch error.
#[tokio::test]
async fn guessed_market_sparse_fallback_errors_returns_sparse_items() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok(
            "adzuna",
            vec![
                sample_posting("s1", "adzuna"),
                sample_posting("s2", "adzuna"),
            ],
        )),
        // Fallback is configured but fails on this call.
        Box::new(FakeProvider::err("jsearch", "jsearch upstream 500")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "London",
        "de",
        true, // country_guessed
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .expect("a failing fallback must not sink the sparse Adzuna items");

    assert_eq!(
        result.len(),
        2,
        "sparse guessed-market Adzuna items must survive a failing JSearch fallback"
    );
    assert!(
        result.iter().all(|p| p
            .external_id
            .as_deref()
            .unwrap_or("")
            .starts_with("adzuna-")),
        "the returned items must be the sparse Adzuna hits, not JSearch's"
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
        note_sink: None,
        base_url: ADZUNA_BASE_URL.to_string(),
    };
    // "xx" is not in the allowlist.
    let result = p
        .search("engineer", "Seoul", "xx", false, None, None, make_token())
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
        note_sink: None,
        base_url: ADZUNA_BASE_URL.to_string(),
    };
    // "de" is in the allowlist; the error that comes back must NOT mention the
    // allowlist — it should be a network/auth error (or similar), not a country error.
    let result = p
        .search("engineer", "Berlin", "de", false, None, None, make_token())
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
