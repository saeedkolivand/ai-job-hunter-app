#![allow(dead_code)]

/// HTTP client for scrapers.
///
/// - sane default headers (modern desktop UA)
/// - per-request abort signal honoured
/// - light retry on transient failures
/// - opt-in JSON / HTML helpers with size caps
use std::time::Duration;

const DEFAULT_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

const MAX_BYTES: usize = 8 * 1024 * 1024; // 8 MB per response

#[derive(Debug, Clone)]
pub struct FetchOptions {
    pub headers: Option<Vec<(String, String)>>,
    pub method: Option<reqwest::Method>,
    pub body: Option<String>,
    pub retries: u32,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            headers: None,
            method: None,
            body: None,
            retries: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchResult {
    pub status_code: u16,
    pub headers: reqwest::header::HeaderMap,
    pub text: String,
}

pub async fn fetch_text(
    url: &str,
    opts: FetchOptions,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<FetchResult> {
    let retries = opts.retries;
    let mut last_err: Option<anyhow::Error> = None;

    for attempt in 0..=retries {
        if signal.is_cancelled() {
            return Err(anyhow::anyhow!("Request cancelled"));
        }

        let client = crate::net::http::shared();
        let mut request = match opts.method.clone().unwrap_or(reqwest::Method::GET) {
            reqwest::Method::GET => client.get(url),
            reqwest::Method::POST => client.post(url),
            _ => client.get(url),
        };

        request = request.header("user-agent", DEFAULT_UA);
        request = request.header(
            "accept",
            "text/html,application/xhtml+xml,application/xml,application/json;q=0.9,*/*;q=0.8",
        );
        request = request.header("accept-language", "en-US,en;q=0.9,de;q=0.8");

        if let Some(headers) = &opts.headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        if let Some(body) = &opts.body {
            request = request.body(body.clone());
        }

        match request.send().await {
            Ok(response) => {
                let status_code = response.status().as_u16();
                let headers = response.headers().clone();
                
                // Check content length if available
                if let Some(content_length) = response.content_length() {
                    if content_length > MAX_BYTES as u64 {
                        return Err(anyhow::anyhow!("Response too large"));
                    }
                }

                let text = response.text().await?;
                
                if text.len() > MAX_BYTES {
                    return Err(anyhow::anyhow!("Response too large"));
                }

                return Ok(FetchResult {
                    status_code,
                    headers,
                    text,
                });
            }
            Err(e) => {
                last_err = Some(anyhow::anyhow!(e));
                if signal.is_cancelled() {
                    return Err(anyhow::anyhow!("Request cancelled"));
                }
                if attempt < retries {
                    tokio::time::sleep(Duration::from_millis(300 * (attempt + 1) as u64)).await;
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fetch_text failed")))
}

pub async fn fetch_json<T: for<'de> serde::Deserialize<'de>>(
    url: &str,
    opts: FetchOptions,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Option<T>> {
    let res = fetch_text(
        url,
        FetchOptions {
            headers: Some(vec![("accept".to_string(), "application/json".to_string())]),
            ..opts
        },
        signal,
    )
    .await?;

    if res.status_code < 200 || res.status_code >= 300 {
        return Ok(None);
    }

    match serde_json::from_str::<T>(&res.text) {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

pub fn strip_html(html: &str) -> String {
    let mut result = html.to_string();
    
    // Remove script and style tags
    result = regex::Regex::new(r"(?i)<script[\s\S]*?</script>").unwrap().replace_all(&result, " ").to_string();
    result = regex::Regex::new(r"(?i)<style[\s\S]*?</style>").unwrap().replace_all(&result, " ").to_string();
    
    // Remove all HTML tags
    result = regex::Regex::new(r"<[^>]+>").unwrap().replace_all(&result, " ").to_string();
    
    // Decode HTML entities
    result = result.replace("&nbsp;", " ");
    result = result.replace("&amp;", "&");
    result = result.replace("&lt;", "<");
    result = result.replace("&gt;", ">");
    result = result.replace("&quot;", "\"");
    result = result.replace("&#39;", "'");
    
    // Collapse whitespace
    result = regex::Regex::new(r"\s+").unwrap().replace_all(&result, " ").to_string();

    result.trim().to_string()
}

/// Like `strip_html`, but preserves document structure: block elements become
/// line breaks and `<li>` items become bullet lines, so job descriptions stay
/// readable instead of collapsing into one paragraph.
pub fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();

    // Drop script/style blocks entirely.
    s = regex::Regex::new(r"(?is)<(script|style)[\s\S]*?</(script|style)>")
        .unwrap()
        .replace_all(&s, " ")
        .to_string();

    // List items → bullet on their own line.
    s = regex::Regex::new(r"(?i)<li[^>]*>").unwrap().replace_all(&s, "\n• ").to_string();
    // Explicit line breaks.
    s = regex::Regex::new(r"(?i)<br\s*/?>").unwrap().replace_all(&s, "\n").to_string();
    // Block-level closers → newline (li excluded; its opener already broke the line).
    s = regex::Regex::new(r"(?i)</(p|div|ul|ol|h[1-6]|tr|section|header|article)>")
        .unwrap()
        .replace_all(&s, "\n")
        .to_string();

    // Strip any remaining tags.
    s = regex::Regex::new(r"<[^>]+>").unwrap().replace_all(&s, "").to_string();

    // Decode common entities.
    s = s
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Collapse runs of spaces/tabs, trim each line, cap consecutive blank lines.
    s = regex::Regex::new(r"[ \t]+").unwrap().replace_all(&s, " ").to_string();
    s = regex::Regex::new(r" *\n *").unwrap().replace_all(&s, "\n").to_string();
    s = regex::Regex::new(r"\n{3,}").unwrap().replace_all(&s, "\n\n").to_string();
    // Keep bullets tight under their heading — no blank line before a bullet.
    s = regex::Regex::new(r"\n{2,}•").unwrap().replace_all(&s, "\n•").to_string();

    s.trim().to_string()
}

#[cfg(test)]
mod test;
