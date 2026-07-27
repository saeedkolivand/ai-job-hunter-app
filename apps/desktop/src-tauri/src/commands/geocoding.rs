//! Location autocomplete.
//!
//! **Offline first.** A bundled GeoNames index ([`geonames`]) answers virtually
//! every query with zero network traffic; only a query it cannot match at all
//! reaches [`photon`] (photon.komoot.io — OpenStreetMap data). Nominatim is no
//! longer used: its usage policy explicitly forbids autocomplete, and its rate
//! limiting degraded the picker to "no suggestions" exactly when a user typed
//! fastest.
//!
//! The suggestion shape (`{display, lat, lon, countryCode}`, city/country
//! granularity, max 5, deduped by label) is unchanged, so `geocode_suggest`,
//! the `LocationInput` picker, and autopilot's save-time `country_code`
//! backfill all keep working untouched.

use std::collections::HashSet;

use serde_json::{json, Value};

mod geonames;

#[cfg(test)]
mod test;

/// Suggestions returned to the picker. Unchanged from the Nominatim era — the
/// dropdown is sized for it and callers rely on it.
const MAX_SUGGESTIONS: usize = 5;

/// Minimum query length before the *online* fallback may run. The offline index
/// answers short queries fine; a 1–2 char miss is almost always a half-typed
/// word, and firing a request per keystroke at a free community endpoint is
/// exactly the fair-use problem this rewrite exists to fix (the renderer's
/// 300 ms debounce in `packages/ui/src/hooks/useGeocoding.ts` is the other half).
const MIN_ONLINE_QUERY_CHARS: usize = 3;

/// Reduce one Photon GeoJSON feature to a city- or country-level suggestion.
/// Returns None for anything more detailed (street/house/POI) that carries no
/// parent city, and for region/state-level hits — so the UI only ever shows
/// "City, Country" or a bare country name.
fn to_city_country(item: &Value) -> Option<Value> {
    let properties = item.get("properties");
    let string = |key: &str| {
        properties
            .and_then(|p| p.get(key))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };

    let feature_type = string("type");
    let is_country_level = feature_type.as_deref() == Some("country");

    // `city` is present on sub-city features (street/house/district) and names
    // the containing city, so those collapse to it — the same rule the previous
    // Nominatim reduction used. A feature that IS a city carries its name in
    // `name` instead.
    let city = string("city").or_else(|| {
        matches!(
            feature_type.as_deref(),
            Some("city") | Some("district") | Some("locality")
        )
        .then(|| string("name"))
        .flatten()
    });

    let country = string("country");
    let country_code = string("countrycode").map(|s| s.to_uppercase());

    let coordinate = |i: usize| {
        item.get("geometry")
            .and_then(|g| g.get("coordinates"))
            .and_then(|c| c.get(i))
            .and_then(|v| v.as_f64())
    };
    // GeoJSON coordinate order is [lon, lat].
    let (lon, lat) = (coordinate(0), coordinate(1));

    // Keep only city-level matches, or explicit country-level matches.
    if city.is_none() && !is_country_level {
        return None;
    }

    let display = match (&city, &country) {
        (Some(city), Some(country)) => format!("{city}, {country}"),
        (Some(city), None) => city.clone(),
        // Country-level result (no city): label is the country name, falling
        // back to country_code if the country name is somehow absent.
        (None, Some(country)) => country.clone(),
        (None, None) => country_code.clone()?,
    };

    Some(json!({
        "display": display,
        "lat": lat,
        "lon": lon,
        "countryCode": country_code,
    }))
}

/// Keep the first suggestion per visible label, preserve order, cap at `limit`.
/// Lazy — the iterator is only pulled until `limit` distinct labels are seen,
/// which is what lets the offline index materialize JSON for the winners only.
fn dedupe_by_display(items: impl Iterator<Item = Value>, limit: usize) -> Vec<Value> {
    let mut seen: HashSet<String> = HashSet::new();
    items
        .filter(|s| {
            s.get("display")
                .and_then(|d| d.as_str())
                .is_some_and(|d| seen.insert(d.to_string()))
        })
        .take(limit)
        .collect()
}

/// Map a Photon `FeatureCollection` body to suggestions. Pure — split out from
/// the request so the mapping is unit-testable against canned JSON.
fn photon_suggestions(body: &Value) -> Vec<Value> {
    match body.get("features").and_then(|f| f.as_array()) {
        Some(features) => {
            dedupe_by_display(features.iter().filter_map(to_city_country), MAX_SUGGESTIONS)
        }
        None => vec![],
    }
}

/// Whether an offline miss is worth a network round trip. Pure (no network) so
/// the fair-use guard is unit-tested directly, mirroring autopilot's
/// `should_derive_country_code`.
fn should_try_online(query: &str) -> bool {
    query.chars().count() >= MIN_ONLINE_QUERY_CHARS
}

/// Online fallback: Photon (komoot), purpose-built for typeahead over
/// OpenStreetMap data. Never errors — a network/parse failure degrades to
/// `vec![]`, i.e. exactly the "no suggestions" state the offline miss already
/// produced.
async fn photon(query: &str) -> Vec<Value> {
    let url = format!(
        "https://photon.komoot.io/api/?q={}&limit=10&lang=en",
        urlencoding::encode(query)
    );

    let response = match crate::net::http::shared()
        .get(&url)
        .header(reqwest::header::USER_AGENT, "ai-job-hunter/1.0")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    match response.json::<Value>().await {
        Ok(body) => photon_suggestions(&body),
        Err(_) => vec![],
    }
}

/// Core suggestion lookup shared by the `geocode_suggest` IPC command AND any
/// server-side caller (e.g. autopilot's save-time `country_code` derivation —
/// the autopilot aggregator zero-jobs fix). Empty query → empty result, no
/// lookup at all. Never errors.
///
/// The bundled index is consulted first and answers offline; the network is
/// only touched when it yields nothing for a query of at least
/// [`MIN_ONLINE_QUERY_CHARS`] characters.
pub(crate) async fn suggest(query: &str) -> Vec<Value> {
    let query = query.trim();
    if query.is_empty() {
        return vec![];
    }

    // Scanning ~34k pre-folded rows is sub-millisecond once the index is built,
    // so this stays inline rather than on a blocking pool.
    let offline = geonames::search(query, MAX_SUGGESTIONS);
    if !offline.is_empty() {
        return offline;
    }

    if !should_try_online(query) {
        return vec![];
    }
    photon(query).await
}

#[tauri::command]
pub async fn geocode_suggest(query: String) -> Value {
    json!(suggest(&query).await)
}
