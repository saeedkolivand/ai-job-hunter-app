use serde_json::{json, Value};

#[tauri::command]
pub async fn geocode_suggest(query: String) -> Value {
    if query.trim().is_empty() {
        return json!([]);
    }

    let client = reqwest::Client::builder()
        .user_agent("ai-job-hunter/1.0")
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => return json!([]),
    };

    let url = format!(
        "https://nominatim.openstreetmap.org/search?format=json&q={}&limit=5&addressdetails=1",
        urlencoding::encode(&query)
    );

    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return json!([]),
    };

    let results = match response.json::<Vec<serde_json::Value>>().await {
        Ok(r) => r,
        Err(_) => return json!([]),
    };

    let suggestions: Vec<Value> = results
        .into_iter()
        .filter_map(|item| {
            let display = item.get("display_name")?.as_str()?;
            Some(json!({ "display": display }))
        })
        .collect();

    json!(suggestions)
}
