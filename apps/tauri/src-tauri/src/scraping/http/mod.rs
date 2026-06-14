/// HTTP client for scrapers.
///
/// - sane default headers (modern desktop UA)
/// - per-request abort signal honoured
/// - light retry on transient failures
/// - opt-in JSON / HTML helpers with size caps
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::net::http::DEFAULT_UA;

const MAX_BYTES: usize = 8 * 1024 * 1024; // 8 MB per response

// ── Compiled-once regexes for HTML→text helpers ────────────────────────────────
// Promoted from per-call `Regex::new(...).unwrap()` so each `strip_html` /
// `html_to_text` call reuses the compiled automata (mirrors `extraction/html.rs`).
static STRIP_SCRIPT_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)<script[\s\S]*?</script>").unwrap());
static STRIP_STYLE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)<style[\s\S]*?</style>").unwrap());
static STRIP_TAG_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<[^>]+>").unwrap());
static STRIP_WS_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"\s+").unwrap());

static TT_SCRIPT_STYLE_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?is)<(script|style)[\s\S]*?</(script|style)>").unwrap()
});
static TT_LI_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)<li[^>]*>").unwrap());
static TT_BR_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)<br\s*/?>").unwrap());
static TT_BLOCK_CLOSE_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)</(p|div|ul|ol|h[1-6]|tr|section|header|article)>").unwrap()
});
static TT_TAG_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<[^>]+>").unwrap());
static TT_SPACES_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"[ \t]+").unwrap());
static TT_LINE_TRIM_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r" *\n *").unwrap());
static TT_BLANKS_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"\n{3,}").unwrap());
static TT_BULLET_TIGHT_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"\n{2,}•").unwrap());

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
) -> AppResult<FetchResult> {
    let retries = opts.retries;
    let mut last_err: Option<AppError> = None;

    for attempt in 0..=retries {
        if signal.is_cancelled() {
            return Err(AppError::Cancelled);
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
                        return Err(AppError::Validation("Response too large".to_string()));
                    }
                }

                let text = response.text().await?;

                if text.len() > MAX_BYTES {
                    return Err(AppError::Validation("Response too large".to_string()));
                }

                return Ok(FetchResult {
                    status_code,
                    headers,
                    text,
                });
            }
            Err(e) => {
                last_err = Some(AppError::Network(e.to_string()));
                if signal.is_cancelled() {
                    return Err(AppError::Cancelled);
                }
                if attempt < retries {
                    tokio::time::sleep(Duration::from_millis(300 * (attempt + 1) as u64)).await;
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| AppError::Network("fetch_text failed".to_string())))
}

pub async fn fetch_json<T: for<'de> serde::Deserialize<'de>>(
    url: &str,
    opts: FetchOptions,
    signal: tokio_util::sync::CancellationToken,
) -> AppResult<Option<T>> {
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
        Err(e) => {
            // Log metadata only; never the raw response body (third-party data).
            log::debug!(
                "[scraping::http] fetch_json parse failure for {url} ({e}); body_len={}",
                res.text.len()
            );
            Ok(None)
        }
    }
}

pub fn strip_html(html: &str) -> String {
    let mut result = html.to_string();

    // Remove script and style tags
    result = STRIP_SCRIPT_RE.replace_all(&result, " ").to_string();
    result = STRIP_STYLE_RE.replace_all(&result, " ").to_string();

    // Remove all HTML tags
    result = STRIP_TAG_RE.replace_all(&result, " ").to_string();

    // Decode HTML entities
    result = result.replace("&nbsp;", " ");
    result = result.replace("&amp;", "&");
    result = result.replace("&lt;", "<");
    result = result.replace("&gt;", ">");
    result = result.replace("&quot;", "\"");
    result = result.replace("&#39;", "'");

    // Collapse whitespace
    result = STRIP_WS_RE.replace_all(&result, " ").to_string();

    result.trim().to_string()
}

/// Like `strip_html`, but preserves document structure: block elements become
/// line breaks and `<li>` items become bullet lines, so job descriptions stay
/// readable instead of collapsing into one paragraph.
pub fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();

    // Drop script/style blocks entirely.
    s = TT_SCRIPT_STYLE_RE.replace_all(&s, " ").to_string();

    // List items → bullet on their own line.
    s = TT_LI_RE.replace_all(&s, "\n• ").to_string();
    // Explicit line breaks.
    s = TT_BR_RE.replace_all(&s, "\n").to_string();
    // Block-level closers → newline (li excluded; its opener already broke the line).
    s = TT_BLOCK_CLOSE_RE.replace_all(&s, "\n").to_string();

    // Strip any remaining tags.
    s = TT_TAG_RE.replace_all(&s, "").to_string();

    // Decode common entities.
    s = s
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Collapse runs of spaces/tabs, trim each line, cap consecutive blank lines.
    s = TT_SPACES_RE.replace_all(&s, " ").to_string();
    s = TT_LINE_TRIM_RE.replace_all(&s, "\n").to_string();
    s = TT_BLANKS_RE.replace_all(&s, "\n\n").to_string();
    // Keep bullets tight under their heading — no blank line before a bullet.
    s = TT_BULLET_TIGHT_RE.replace_all(&s, "\n•").to_string();

    s.trim().to_string()
}

#[cfg(test)]
mod test;
