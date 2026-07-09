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

#[tauri::command]
pub async fn geocode_suggest(query: String) -> Value {
    json!(suggest(&query).await)
}
