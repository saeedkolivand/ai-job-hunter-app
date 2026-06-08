use serde_json::{json, Value};

#[tauri::command]
pub async fn geocode_suggest(query: String) -> Value {
    if query.trim().is_empty() {
        return json!([]);
    }

    let url = format!(
        "https://nominatim.openstreetmap.org/search?format=json&q={}&limit=5&addressdetails=1",
        urlencoding::encode(&query)
    );

    let response = match crate::net::http::shared()
        .get(&url)
        .header(reqwest::header::USER_AGENT, "ai-job-hunter/1.0")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return json!([]),
    };

    let results = match response.json::<Vec<serde_json::Value>>().await {
        Ok(r) => r,
        Err(_) => return json!([]),
    };

    // Surface the structured fields Nominatim already returns (addressdetails=1):
    // ISO country code + coordinates. They let the scrape pass a precise location
    // (country filter / radius) instead of a fuzzy free-text string, which is what
    // caused results to leak across countries (#49). `display` stays primary so
    // existing consumers that read only `.display` keep working.
    let suggestions: Vec<Value> = results
        .into_iter()
        .filter_map(|item| {
            let display = item.get("display_name")?.as_str()?.to_string();
            let lat = item
                .get("lat")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok());
            let lon = item
                .get("lon")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok());
            let country_code = item
                .get("address")
                .and_then(|a| a.get("country_code"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_uppercase());
            Some(json!({
                "display": display,
                "lat": lat,
                "lon": lon,
                "countryCode": country_code,
            }))
        })
        .collect();

    json!(suggestions)
}
