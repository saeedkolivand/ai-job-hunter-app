use serde_json::Value;

const BRAVE_SEARCH_URL: &str = "https://api.search.brave.com/res/v1/web/search";

/// A single search result snippet.
pub struct SearchResult {
    pub title: String,
    pub snippet: String,
    #[allow(dead_code)]
    pub url: String,
}

/// Call the Brave Search API and return the top `limit` result snippets.
/// Returns an empty vec (not an error) when the API key is missing or the
/// call fails — the pipeline continues without research in that case.
pub async fn brave_search(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, String> {
    let resp = client
        .get(BRAVE_SEARCH_URL)
        .header("Accept", "application/json")
        .header("Accept-Encoding", "gzip")
        .header("X-Subscription-Token", api_key)
        .query(&[("q", query), ("count", &limit.to_string()), ("text_decorations", "0")])
        .send()
        .await
        .map_err(|e| format!("brave search request: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("brave search {status}: {body}"));
    }

    let body: Value = resp.json().await.map_err(|e| format!("brave search parse: {e}"))?;

    let results = body
        .get("web")
        .and_then(|w| w.get("results"))
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .take(limit)
                .map(|item| SearchResult {
                    title: item
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet: item
                        .get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: item
                        .get("url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(results)
}
