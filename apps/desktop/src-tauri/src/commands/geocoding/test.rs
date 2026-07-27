//! Unit tests for `to_city_country` and the server-side `country_code`
//! backfill helpers (`should_derive_country_code`,
//! `country_code_from_suggestions`, `derive_country_code`) — the latter moved
//! here with the helpers when the manual scrape path started sharing the
//! autopilot save path's backfill.
//!
//! This child module can reach the private parent helpers via `super::…`.
//! Tests are authored by the test-author stage.

use serde_json::json;

use super::{
    country_code_from_suggestions, derive_country_code, should_derive_country_code, to_city_country,
};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn display(v: &serde_json::Value) -> &str {
    v.get("display")
        .and_then(|d| d.as_str())
        .expect("display field missing")
}

fn country_code(v: &serde_json::Value) -> Option<&str> {
    v.get("countryCode").and_then(|c| c.as_str())
}

fn lat(v: &serde_json::Value) -> Option<f64> {
    v.get("lat").and_then(|l| l.as_f64())
}

fn lon(v: &serde_json::Value) -> Option<f64> {
    v.get("lon").and_then(|l| l.as_f64())
}

// ---------------------------------------------------------------------------
// 1. City result (addresstype == "city")
// ---------------------------------------------------------------------------

#[test]
fn city_result_full_fields() {
    let item = json!({
        "addresstype": "city",
        "lat": "52.5",
        "lon": "13.4",
        "address": {
            "city": "Berlin",
            "country": "Germany",
            "country_code": "de"
        }
    });
    let result = to_city_country(&item).expect("should return Some for city result");

    assert_eq!(display(&result), "Berlin, Germany");
    assert_eq!(country_code(&result), Some("DE"));
    assert_eq!(lat(&result), Some(52.5));
    assert_eq!(lon(&result), Some(13.4));
}

// ---------------------------------------------------------------------------
// 2. town / village / municipality / hamlet fallbacks
// ---------------------------------------------------------------------------

#[test]
fn town_fallback() {
    let item = json!({
        "addresstype": "town",
        "lat": "51.5",
        "lon": "-0.1",
        "address": {
            "town": "Reading",
            "country": "United Kingdom",
            "country_code": "gb"
        }
    });
    let result = to_city_country(&item).expect("should return Some for town");

    assert_eq!(display(&result), "Reading, United Kingdom");
    assert_eq!(country_code(&result), Some("GB"));
    assert_eq!(lat(&result), Some(51.5));
    assert_eq!(lon(&result), Some(-0.1));
}

#[test]
fn village_fallback() {
    let item = json!({
        "addresstype": "village",
        "lat": "48.1",
        "lon": "11.6",
        "address": {
            "village": "Grünwald",
            "country": "Germany",
            "country_code": "de"
        }
    });
    let result = to_city_country(&item).expect("should return Some for village");

    assert_eq!(display(&result), "Grünwald, Germany");
    assert_eq!(country_code(&result), Some("DE"));
}

#[test]
fn municipality_fallback() {
    let item = json!({
        "addresstype": "municipality",
        "lat": "60.0",
        "lon": "25.0",
        "address": {
            "municipality": "Espoo",
            "country": "Finland",
            "country_code": "fi"
        }
    });
    let result = to_city_country(&item).expect("should return Some for municipality");

    assert_eq!(display(&result), "Espoo, Finland");
    assert_eq!(country_code(&result), Some("FI"));
}

#[test]
fn hamlet_fallback() {
    let item = json!({
        "addresstype": "hamlet",
        "lat": "55.0",
        "lon": "10.0",
        "address": {
            "hamlet": "Stengade",
            "country": "Denmark",
            "country_code": "dk"
        }
    });
    let result = to_city_country(&item).expect("should return Some for hamlet");

    assert_eq!(display(&result), "Stengade, Denmark");
    assert_eq!(country_code(&result), Some("DK"));
}

// city takes priority over town when both present
#[test]
fn city_takes_priority_over_town() {
    let item = json!({
        "addresstype": "city",
        "lat": "52.5",
        "lon": "13.4",
        "address": {
            "city": "Berlin",
            "town": "Wannsee",
            "country": "Germany",
            "country_code": "de"
        }
    });
    let result = to_city_country(&item).expect("should return Some");
    assert_eq!(display(&result), "Berlin, Germany");
}

// ---------------------------------------------------------------------------
// 3. Country-level result (no city field)
// ---------------------------------------------------------------------------

#[test]
fn country_level_result() {
    let item = json!({
        "addresstype": "country",
        "address": {
            "country": "Germany",
            "country_code": "de"
        }
    });
    let result = to_city_country(&item).expect("should return Some for country-level");

    assert_eq!(display(&result), "Germany");
    assert_eq!(country_code(&result), Some("DE"));
    // lat/lon absent → null
    assert!(result.get("lat").and_then(|v| v.as_f64()).is_none());
    assert!(result.get("lon").and_then(|v| v.as_f64()).is_none());
}

// ---------------------------------------------------------------------------
// 4. Rejected types — road / house / postcode / POI / state
// ---------------------------------------------------------------------------

#[test]
fn road_rejected() {
    let item = json!({
        "addresstype": "road",
        "lat": "52.5",
        "lon": "13.4",
        "address": {
            "road": "Main St",
            "country": "Germany",
            "country_code": "de"
        }
    });
    assert!(to_city_country(&item).is_none(), "road should be rejected");
}

#[test]
fn postcode_rejected() {
    let item = json!({
        "addresstype": "postcode",
        "lat": "52.5",
        "lon": "13.4",
        "address": {
            "postcode": "10115",
            "country": "Germany",
            "country_code": "de"
        }
    });
    assert!(
        to_city_country(&item).is_none(),
        "postcode should be rejected"
    );
}

#[test]
fn house_number_rejected() {
    let item = json!({
        "addresstype": "house",
        "lat": "52.5",
        "lon": "13.4",
        "address": {
            "house_number": "42",
            "country": "Germany",
            "country_code": "de"
        }
    });
    assert!(to_city_country(&item).is_none(), "house should be rejected");
}

#[test]
fn poi_rejected() {
    let item = json!({
        "addresstype": "amenity",
        "lat": "52.5",
        "lon": "13.4",
        "address": {
            "amenity": "Brandenburg Gate",
            "country": "Germany",
            "country_code": "de"
        }
    });
    assert!(
        to_city_country(&item).is_none(),
        "POI/amenity should be rejected"
    );
}

#[test]
fn state_level_rejected() {
    let item = json!({
        "addresstype": "state",
        "lat": "52.0",
        "lon": "12.0",
        "address": {
            "state": "Brandenburg",
            "country": "Germany",
            "country_code": "de"
        }
    });
    assert!(
        to_city_country(&item).is_none(),
        "state/region should be rejected"
    );
}

// ---------------------------------------------------------------------------
// 5. No country — city present, country absent
// ---------------------------------------------------------------------------

#[test]
fn city_without_country() {
    let item = json!({
        "addresstype": "city",
        "lat": "34.0",
        "lon": "36.0",
        "address": {
            "city": "Homs"
            // no country, no country_code
        }
    });
    let result =
        to_city_country(&item).expect("should return Some when city present without country");

    assert_eq!(
        display(&result),
        "Homs",
        "no trailing comma when country absent"
    );
    assert!(
        result
            .get("countryCode")
            .map(|v| v.is_null())
            .unwrap_or(true),
        "countryCode should be null/absent"
    );
}

// ---------------------------------------------------------------------------
// 6. Missing or non-numeric lat/lon → null, suggestion still returned
// ---------------------------------------------------------------------------

#[test]
fn missing_lat_lon_still_returns_suggestion() {
    let item = json!({
        "addresstype": "city",
        "address": {
            "city": "Oslo",
            "country": "Norway",
            "country_code": "no"
        }
        // lat / lon keys absent
    });
    let result = to_city_country(&item).expect("should return Some even without lat/lon");

    assert_eq!(display(&result), "Oslo, Norway");
    assert!(lat(&result).is_none(), "lat should be null when absent");
    assert!(lon(&result).is_none(), "lon should be null when absent");
}

#[test]
fn non_numeric_lat_lon_returns_null_fields() {
    let item = json!({
        "addresstype": "city",
        "lat": "not-a-number",
        "lon": "also-bad",
        "address": {
            "city": "Atlantis",
            "country": "Mythica",
            "country_code": "my"
        }
    });
    let result = to_city_country(&item).expect("should return Some with non-numeric lat/lon");

    assert_eq!(display(&result), "Atlantis, Mythica");
    assert!(lat(&result).is_none(), "non-numeric lat should be null");
    assert!(lon(&result).is_none(), "non-numeric lon should be null");
}

// ---------------------------------------------------------------------------
// 7. country_code casing — always upper-cased
// ---------------------------------------------------------------------------

#[test]
fn country_code_uppercased_from_lowercase() {
    let item = json!({
        "addresstype": "city",
        "lat": "51.5",
        "lon": "-0.1",
        "address": {
            "city": "London",
            "country": "United Kingdom",
            "country_code": "gb"
        }
    });
    let result = to_city_country(&item).expect("should return Some");

    assert_eq!(
        country_code(&result),
        Some("GB"),
        "country_code must be upper-cased"
    );
}

#[test]
fn country_code_already_upper_stays_upper() {
    let item = json!({
        "addresstype": "city",
        "lat": "48.9",
        "lon": "2.3",
        "address": {
            "city": "Paris",
            "country": "France",
            "country_code": "FR"
        }
    });
    let result = to_city_country(&item).expect("should return Some");

    assert_eq!(country_code(&result), Some("FR"));
}

// ---------------------------------------------------------------------------
// 8. Empty string city fields are treated as absent (use next fallback)
// ---------------------------------------------------------------------------

#[test]
fn empty_city_falls_through_to_town() {
    let item = json!({
        "addresstype": "town",
        "lat": "53.0",
        "lon": "9.0",
        "address": {
            "city": "",
            "town": "Buxtehude",
            "country": "Germany",
            "country_code": "de"
        }
    });
    let result = to_city_country(&item).expect("should return Some using town fallback");

    assert_eq!(display(&result), "Buxtehude, Germany");
}

// ---------------------------------------------------------------------------
// 9. Country-level with only country_code (no country name)
//    Branch: (None, None) => country_code.clone()?  →  yields raw uppercased code.
// ---------------------------------------------------------------------------

#[test]
fn country_level_code_only_no_country_name() {
    let item = json!({
        "addresstype": "country",
        "address": {
            "country_code": "de"
            // "country" name intentionally absent
        }
    });
    let result = to_city_country(&item)
        .expect("should return Some: country_code fallback covers the (None,None) branch");

    assert_eq!(
        display(&result),
        "DE",
        "display must be the raw uppercased country_code when country name is absent"
    );
    assert_eq!(
        country_code(&result),
        Some("DE"),
        "countryCode must also be the uppercased code"
    );
    // lat/lon absent from the input → the JSON value must be explicitly null,
    // not merely a missing key — assert directly on the Value variant.
    assert!(
        result.get("lat").map(|v| v.is_null()).unwrap_or(false),
        "lat must be an explicit JSON null (not a missing key)"
    );
    assert!(
        result.get("lon").map(|v| v.is_null()).unwrap_or(false),
        "lon must be an explicit JSON null (not a missing key)"
    );
}

// ---------------------------------------------------------------------------
// 10. Country addresstype with empty address → returns None
//     The `?` on `country_code.clone()?` in the (None, None) branch propagates
//     None out of the helper when both country name and country_code are absent.
// ---------------------------------------------------------------------------

#[test]
fn country_level_empty_address_returns_none() {
    let item = json!({
        "addresstype": "country",
        "address": {}
        // no country, no country_code, no city
    });
    assert!(
        to_city_country(&item).is_none(),
        "country addresstype with an empty address must return None: \
         the ? on country_code propagates None out of the helper"
    );
}

// ---------------------------------------------------------------------------
// 11. Road result that carries a parent `city` field (Nominatim contextual address)
//
//     Nominatim sometimes returns `addresstype:"road"` but includes a `city`
//     field in the address object because it encodes the administrative context
//     of the road.  The keep-rule in `to_city_country` is:
//       "city present OR is_country_level" → accept.
//     Because `city` is present the road hit is accepted, and `display` is built
//     from the city (not the road name).  This is intentional: a road match
//     collapses to its containing city so the UI shows "Berlin, Germany" rather
//     than a street name.
// ---------------------------------------------------------------------------

#[test]
fn road_with_parent_city_collapses_to_city() {
    let item = json!({
        "addresstype": "road",
        "lat": "52.5",
        "lon": "13.4",
        "address": {
            "road": "Unter den Linden",
            "city": "Berlin",
            "country": "Germany",
            "country_code": "de"
        }
    });
    // Road hits are normally rejected, but this one carries a `city` field in
    // the address context, so the keep-rule (`city.is_some()`) accepts it and
    // the display collapses to the city — never the road name.
    let result = to_city_country(&item)
        .expect("road with a parent city field must be accepted and collapsed to its city");

    assert_eq!(
        display(&result),
        "Berlin, Germany",
        "display must be the city, not the road name"
    );
    assert_eq!(
        country_code(&result),
        Some("DE"),
        "countryCode must be uppercased"
    );
    assert_eq!(lat(&result), Some(52.5), "lat must be preserved");
    assert_eq!(lon(&result), Some(13.4), "lon must be preserved");
}

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
