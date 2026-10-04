use super::*;

// ---------------------------------------------------------------------------
// country_code_from_suggestions / should_derive_country_code
//
// The server-side `country_code` backfill for a location that never went
// through the picker. Shared by the autopilot save path and the manual scrape
// path, so these live here (next to `suggest`) rather than in one command
// module.
// ---------------------------------------------------------------------------

#[test]
fn should_derive_country_code_requires_a_real_location() {
    assert!(should_derive_country_code(Some("London")));
    // Whitespace-only or absent location → nothing to geocode.
    assert!(!should_derive_country_code(Some("   ")));
    assert!(!should_derive_country_code(Some("")));
    assert!(!should_derive_country_code(None));
}

#[test]
fn country_code_from_suggestions_takes_first_hit_lowercased() {
    let suggestions = vec![
        json!({ "display": "London, United Kingdom", "countryCode": "GB" }),
        json!({ "display": "London, Canada", "countryCode": "CA" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&suggestions),
        Some("gb".to_string()),
        "must take the first (best-ranked) suggestion and lower-case it to \
         match BoardSearchInput::country_code's convention"
    );
}

#[test]
fn country_code_from_suggestions_empty_or_missing_field_yields_none() {
    assert_eq!(country_code_from_suggestions(&[]), None);
    // A suggestion missing `countryCode` entirely (e.g. an ambiguous hit).
    let no_country = vec![json!({ "display": "Atlantis" })];
    assert_eq!(country_code_from_suggestions(&no_country), None);
}

#[test]
fn country_code_from_suggestions_skips_a_leading_hit_with_no_country_code() {
    // The first (best-ranked) hit has no countryCode (absent AND explicit
    // null) — must not block a usable later suggestion.
    let absent_then_present = vec![
        json!({ "display": "Ambiguous place" }),
        json!({ "display": "Munich, Germany", "countryCode": "DE" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&absent_then_present),
        Some("de".to_string()),
        "an absent countryCode on the first hit must not block a later, \
         usable suggestion"
    );

    let null_then_present = vec![
        json!({ "display": "Ambiguous place", "countryCode": null }),
        json!({ "display": "Munich, Germany", "countryCode": "DE" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&null_then_present),
        Some("de".to_string()),
        "an explicit null countryCode on the first hit must not block a \
         later, usable suggestion"
    );
}

#[test]
fn country_code_from_suggestions_skips_malformed_country_codes() {
    // A geocoded value written server-side bypasses the IPC schema's
    // `^[A-Za-z]{2}$` guard, so a leading 3-letter ("USA") or non-alpha
    // ("1a") countryCode must be SKIPPED in favour of a later valid hit —
    // not accepted (it would fail BoardSearchInput's 2-letter contract).
    let malformed_then_present = vec![
        json!({ "display": "United States", "countryCode": "USA" }),
        json!({ "display": "Nowhere", "countryCode": "1a" }),
        json!({ "display": "Munich, Germany", "countryCode": "DE" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&malformed_then_present),
        Some("de".to_string()),
        "malformed leading countryCodes (3-letter, non-alpha) must be \
         skipped in favour of a valid 2-letter hit"
    );

    // All candidates malformed → None (nothing usable to backfill).
    let all_malformed = vec![
        json!({ "display": "United States", "countryCode": "USA" }),
        json!({ "display": "Nowhere", "countryCode": "1a" }),
        json!({ "display": "Solo", "countryCode": "G" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&all_malformed),
        None,
        "an all-malformed list yields no country code"
    );
}

#[tokio::test]
async fn derive_country_code_skips_geocode_when_location_absent() {
    // No location at all → must resolve to None WITHOUT attempting a
    // network call (a real HTTP hit here would make this test flaky/slow).
    assert_eq!(derive_country_code(None).await, None);
    assert_eq!(derive_country_code(Some("")).await, None);
    assert_eq!(derive_country_code(Some("   ")).await, None);
}

#[tokio::test]
async fn derive_country_code_resolves_typed_locations_from_the_offline_index() {
    // The backfill now rides on the offline index, so the freehand locations
    // that motivated it resolve with no network at all — including the label
    // the picker writes back, and a typed country code. Anything here reaching
    // the network would blow the 2s BACKFILL_GEOCODE_TIMEOUT in CI and fail.
    for (location, expected) in [
        ("Germany", "de"),
        ("Amsterdam", "nl"),
        ("Berlin, Germany", "de"),
        ("München", "de"),
        ("usa", "us"),
        ("Schweiz", "ch"),
    ] {
        assert_eq!(
            derive_country_code(Some(location)).await,
            Some(expected.to_string()),
            "{location:?} must backfill {expected:?} offline"
        );
    }
}
