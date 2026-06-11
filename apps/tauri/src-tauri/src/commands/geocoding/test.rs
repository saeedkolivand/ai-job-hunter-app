//! Unit tests for `to_city_country`.
//!
//! This child module can reach the private parent helper via
//! `super::to_city_country`. Tests are authored by the test-author stage.

use serde_json::json;

use super::to_city_country;

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
