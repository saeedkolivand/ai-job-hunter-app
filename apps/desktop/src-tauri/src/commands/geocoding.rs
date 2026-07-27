use std::collections::HashSet;

use serde_json::{json, Value};

#[cfg(test)]
mod test;

/// Reduce a Nominatim result to a city- or country-level suggestion.
/// Returns None for anything more detailed (road/house/postcode/POI) or a
/// region/state-level match, so the UI only ever shows "City, Country".
fn to_city_country(item: &Value) -> Option<Value> {
    let address = item.get("address");

    // First present, non-empty city-equivalent field.
    let city = address.and_then(|a| {
        ["city", "town", "village", "municipality", "hamlet"]
            .iter()
            .find_map(|key| {
                a.get(*key)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
    });

    let country = address
        .and_then(|a| a.get("country"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let country_code = address
        .and_then(|a| a.get("country_code"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_uppercase());

    let lat = item
        .get("lat")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok());
    let lon = item
        .get("lon")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok());

    let addresstype = item.get("addresstype").and_then(|v| v.as_str());
    let is_country_level = addresstype == Some("country");

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

/// Core suggestion lookup shared by the `geocode_suggest` IPC command AND any
/// server-side caller (e.g. autopilot's save-time `country_code` derivation —
/// the autopilot aggregator zero-jobs fix). Empty query → empty result, no
/// network call. Never errors: a network/parse failure degrades to `vec![]`.
pub(crate) async fn suggest(query: &str) -> Vec<Value> {
    if query.trim().is_empty() {
        return vec![];
    }

    let url = format!(
        "https://nominatim.openstreetmap.org/search?format=json&q={}&limit=10&addressdetails=1",
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

    let results = match response.json::<Vec<Value>>().await {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    // Reduce every Nominatim hit to a city- or country-level suggestion
    // (`to_city_country`), drop everything more detailed, dedupe by the visible
    // label, preserve order, and cap at 5. `countryCode`/`lat`/`lon` survive so
    // ScrapeForm keeps its country + radius filtering (#49/#40).
    let mut seen: HashSet<String> = HashSet::new();
    results
        .iter()
        .filter_map(to_city_country)
        .filter(|s| {
            s.get("display")
                .and_then(|d| d.as_str())
                .map(|d| seen.insert(d.to_string()))
                .unwrap_or(false)
        })
        .take(5)
        .collect()
}

/// Whether `location` is non-empty after trimming — the only precondition for
/// attempting a geocode lookup. Pure (no network) so it's unit-tested directly.
fn should_derive_country_code(location: Option<&str>) -> bool {
    location.map(str::trim).is_some_and(|s| !s.is_empty())
}

/// Pick a country code out of the geocode service's ranked suggestions: the
/// first hit that actually CARRIES a VALID one wins (not just the first hit —
/// an earlier suggestion with an absent/null/malformed `countryCode` must not
/// block a usable later one), lower-cased to match
/// `BoardSearchInput::country_code`'s convention. Each candidate is validated
/// against the SAME 2-ASCII-letter shape `AutopilotTargetSchema.countryCode`
/// enforces (`^[A-Za-z]{2}$`) — a geocoded value written server-side bypasses
/// the IPC schema, so a `"USA"` / `"1a"`-style value is skipped, not accepted.
/// Pure — no network — so this is unit-tested directly; the HTTP round trip
/// inside [`suggest`] is not (no fixture for it).
fn country_code_from_suggestions(suggestions: &[Value]) -> Option<String> {
    suggestions
        .iter()
        .find_map(|s| {
            s.get("countryCode")
                .and_then(|v| v.as_str())
                .filter(|cc| cc.len() == 2 && cc.bytes().all(|b| b.is_ascii_alphabetic()))
        })
        .map(str::to_lowercase)
}

/// Cap a server-side backfill lookup tighter than [`suggest`]'s own 5s reqwest
/// timeout: that 5s is SHARED with the interactive `geocode_suggest` picker
/// (where the user is waiting on the result and a longer wait is acceptable), so
/// it can't be lowered there. On a backfill path this lookup is best-effort — an
/// autopilot save / a scrape run should not stall for up to 5s on a slow
/// geocode. On timeout we fall through to no suggestion (`None`), and the
/// aggregator's guessed-market guard still covers the residual case.
const BACKFILL_GEOCODE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Best-effort: when a search target has a real `location` but no
/// `country_code` (a prefilled or typed-freehand location that never went
/// through the picker — the aggregator zero-jobs bug), look one up via the SAME
/// geocode service the manual picker uses and return it for backfill.
///
/// Shared by both entry points that can receive a location without a picked
/// suggestion: autopilot save (`commands::autopilot`) and manual board scrape
/// (`commands::scrape`). Without it the aggregator hardcodes a `'de'` market
/// guess AND suppresses its sparse-city broadening, so e.g. a typed "Germany"
/// or "Amsterdam" search silently under-returns.
///
/// Never blocks or fails the caller: a network error / no match / timeout just
/// leaves `country_code` absent, exactly as it would without this fix — the
/// aggregator's own guessed-market guard (`scraping::boards::aggregator`)
/// covers that residual case too.
pub(crate) async fn derive_country_code(location: Option<&str>) -> Option<String> {
    if !should_derive_country_code(location) {
        return None;
    }
    // Safe: `should_derive_country_code` just proved this is `Some`.
    let location = location.unwrap_or_default().trim();
    // Cap the lookup at BACKFILL_GEOCODE_TIMEOUT (see const above): a timeout
    // yields an empty Vec -> None (best-effort), never a stalled caller.
    let suggestions = tokio::time::timeout(BACKFILL_GEOCODE_TIMEOUT, suggest(location))
        .await
        .unwrap_or_default();
    country_code_from_suggestions(&suggestions)
}

#[tauri::command]
pub async fn geocode_suggest(query: String) -> Value {
    json!(suggest(&query).await)
}
