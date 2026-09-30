use super::super::*;
use super::support::*;

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
        SearchBudget::items_only(100),
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
        SearchBudget::items_only(100),
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
        msg.contains("guessed market") && msg.contains("supplied location"),
        "diagnostic must name the guessed-market cause using the generic \
         'supplied location' phrase — the raw user-entered location must NEVER \
         be interpolated into a persisted diagnostic; got: {msg}"
    );
    assert!(
        !msg.contains("London"),
        "diagnostic must NOT leak the raw user-entered location (PII); got: {msg}"
    );
    assert!(
        msg.contains("add a JSearch or Jooble key in Settings"),
        "diagnostic error must include the actionable fallback-remedy suffix; got: {msg}"
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
        SearchBudget::items_only(100),
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

/// Guessed market + non-empty location + a SPARSE Adzuna result (2 < the broaden
/// floor of 3) + JSearch configured → the sparse hits are not trusted; JSearch is
/// consulted and its results win. This is the "London → stray German hits" case.
#[tokio::test]
async fn guessed_market_sparse_with_location_falls_back_to_jsearch() {
    let jsearch_posting = sample_posting("g-sparse", "jsearch");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        // Two stray hits (< ADZUNA_BROADEN_FLOOR) from the GUESSED "de" market.
        Box::new(FakeProvider::ok(
            "adzuna",
            vec![
                sample_posting("s1", "adzuna"),
                sample_posting("s2", "adzuna"),
            ],
        )),
        Box::new(FakeProvider::ok("jsearch", vec![jsearch_posting.clone()])),
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
    .unwrap();

    assert_eq!(
        result.len(),
        1,
        "a sparse (< floor) result from a GUESSED market must fall through to \
         JSearch, not be trusted as authoritative"
    );
    assert_eq!(result[0].external_id, jsearch_posting.external_id);
}

/// Guessed market + a result AT the broaden floor (3 == ADZUNA_BROADEN_FLOOR) →
/// authoritative; JSearch must NOT be consulted (the fake JSearch errors so any
/// call would surface as an Err and fail `.unwrap()`).
#[tokio::test]
async fn guessed_market_at_floor_does_not_fall_back() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok(
            "adzuna",
            vec![
                sample_posting("f1", "adzuna"),
                sample_posting("f2", "adzuna"),
                sample_posting("f3", "adzuna"),
            ],
        )),
        Box::new(FakeProvider::err(
            "jsearch",
            "must not be called at/above the broaden floor",
        )),
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
    .unwrap();

    assert_eq!(
        result.len(),
        3,
        "a result at the broaden floor from a guessed market is authoritative — no fallback"
    );
    assert!(
        result.iter().all(|p| p
            .external_id
            .as_deref()
            .unwrap_or("")
            .starts_with("adzuna-")),
        "the at-floor result must be Adzuna's, not JSearch's"
    );
}
