/// HTTP client for scrapers.
///
/// - sane default headers (modern desktop UA)
/// - per-request abort signal honoured
/// - light retry on transient failures
/// - opt-in JSON / HTML helpers with size caps
use std::time::Duration;

use futures::StreamExt as _;

use crate::error::{AppError, AppResult};
use crate::net::http::DEFAULT_UA;

const MAX_BYTES: usize = 8 * 1024 * 1024; // 8 MB default guard — per-request override via FetchOptions::max_bytes

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

// Detects whether a string contains at least one HTML tag.
// ponytail: simple `<tag` / `</tag` scan — good enough for the aggregator inputs
// that are either pure HTML or plain-text/already-markdown; won't catch every
// hand-crafted edge case (e.g. lone `<` in code), but correct for our use-case.
static HTML_TAG_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"<[a-zA-Z/][^>]*>").expect("static HTML-tag detection regex is valid")
});

#[derive(Debug, Clone)]
pub struct FetchOptions {
    pub headers: Option<Vec<(String, String)>>,
    pub method: Option<reqwest::Method>,
    pub body: Option<String>,
    pub retries: u32,
    /// Per-request byte cap. `None` falls back to the shared `MAX_BYTES` (8 MB).
    /// Use only for feeds known to exceed 8 MB (e.g. the GTJ RSS feed at ~10 MB).
    pub max_bytes: Option<usize>,
    /// Per-request wall-clock timeout. `None` preserves the shared client's
    /// no-global-timeout contract (callers rely on the retry loop for liveness).
    /// Set it for third-party egresses that need a hard ceiling (e.g. the GitHub
    /// repo import) so a slow/stalled connection can't hang indefinitely.
    pub timeout: Option<Duration>,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            headers: None,
            method: None,
            body: None,
            retries: 2,
            max_bytes: None,
            timeout: None,
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
    let cap = opts.max_bytes.unwrap_or(MAX_BYTES);
    let mut last_err: Option<AppError> = None;

    // Extract host once — used for per-host rate limiting on every attempt.
    let host_opt: Option<String> = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()));

    for attempt in 0..=retries {
        if signal.is_cancelled() {
            return Err(AppError::Cancelled);
        }

        // Per-host rate limiting: wait for a free slot before every request
        // attempt, including retries after 429/503. Gating only the first send
        // let a short Retry-After bypass per-host pacing on retry attempts.
        if let Some(ref host) = host_opt {
            let rl = crate::scraping::rate_limiter::for_host(host).await;
            rl.wait_for_slot().await;
        }

        let client = crate::net::http::shared();
        let mut request = match opts.method.clone().unwrap_or(reqwest::Method::GET) {
            reqwest::Method::GET => client.get(url),
            reqwest::Method::POST => client.post(url),
            _ => client.get(url),
        };

        request = request.header("user-agent", DEFAULT_UA);

        // Only add the broad HTML accept when the caller hasn't already set one.
        // This prevents a double `accept` header when fetch_json (which prepends
        // `accept: application/json` into opts.headers) calls fetch_text.
        let caller_has_accept = opts
            .headers
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("accept"));
        if !caller_has_accept {
            request = request.header(
                "accept",
                "text/html,application/xhtml+xml,application/xml,application/json;q=0.9,*/*;q=0.8",
            );
        }
        request = request.header("accept-language", "en-US,en;q=0.9,de;q=0.8");

        if let Some(headers) = &opts.headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }

        if let Some(body) = &opts.body {
            request = request.body(body.clone());
        }

        // Optional per-request timeout. The shared client has no global timeout
        // (net/http.rs), so this is the only hard wall-clock ceiling when set.
        if let Some(timeout) = opts.timeout {
            request = request.timeout(timeout);
        }

        match request.send().await {
            Ok(response) => {
                let status_code = response.status().as_u16();
                let headers = response.headers().clone();

                // Record every completed send — including 429/503 retries — so the
                // per-host window reflects actual traffic, not just final successes.
                if let Some(ref host) = host_opt {
                    let rl = crate::scraping::rate_limiter::for_host(host).await;
                    rl.record_request().await;
                }

                // 429 / 503 — rate-limited or temporarily unavailable. If we have
                // retries left, back off then loop; otherwise fall through and
                // return the response as-is so the caller can observe the status.
                if (status_code == 429 || status_code == 503) && attempt < retries {
                    let wait_ms = retry_after_ms(&headers).unwrap_or_else(|| {
                        // Exponential backoff with ±25% jitter: base * 2^attempt + jitter.
                        let base: u64 = 1_000 * (1u64 << attempt);
                        let jitter = rand::random::<u64>() % (base / 4 + 1);
                        (base + jitter).min(30_000)
                    });
                    tokio::time::sleep(Duration::from_millis(wait_ms)).await;
                    continue;
                }

                // Cheap pre-check: honest servers that declare content-length
                // let us abort before reading a single byte.
                if let Some(content_length) = response.content_length() {
                    if content_length > cap as u64 {
                        return Err(AppError::Validation("Response too large".to_string()));
                    }
                }

                // Determine charset from Content-Type (mirrors what reqwest::Response::text()
                // does internally) so German umlauts / € decode correctly regardless of cap.
                let encoding = headers
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|ct| ct.to_str().ok())
                    .and_then(|ct| {
                        // Extract charset=... from e.g. "text/html; Charset="ISO-8859-1""
                        // Key match is case-insensitive; strip surrounding quotes from value.
                        ct.split(';').find_map(|part| {
                            let p = part.trim();
                            let eq = p.find('=')?;
                            if !p[..eq].trim().eq_ignore_ascii_case("charset") {
                                return None;
                            }
                            let cs = p[eq + 1..].trim().trim_matches(|c| c == '"' || c == '\'');
                            Some(cs.to_ascii_lowercase())
                        })
                    })
                    .and_then(|cs| encoding_rs::Encoding::for_label(cs.as_bytes()))
                    .unwrap_or(encoding_rs::UTF_8);

                // Stream the body, accumulating bytes and aborting as soon as the
                // running total exceeds the effective cap — prevents OOM from a
                // server that lies about or omits Content-Length.
                let mut stream = response.bytes_stream();
                let mut buf: Vec<u8> = Vec::new();
                while let Some(chunk) = stream.next().await {
                    // .without_url() — reqwest::Error's Display embeds the full
                    // request URL (incl. query string), which can carry secrets
                    // like an API token/key; strip it before it reaches
                    // BoardScrapeSummary.error → IPC → renderer + logs.
                    let chunk =
                        chunk.map_err(|e| AppError::Network(e.without_url().to_string()))?;
                    if buf.len().saturating_add(chunk.len()) > cap {
                        return Err(AppError::Validation("Response too large".to_string()));
                    }
                    buf.extend_from_slice(&chunk);
                }

                // Decode using the charset we extracted above.
                let (cow, _enc, _had_errors) = encoding.decode(&buf);
                let text = cow.into_owned();

                return Ok(FetchResult {
                    status_code,
                    headers,
                    text,
                });
            }
            Err(e) => {
                // .without_url() — see the identical comment above; a
                // connection-level failure (e.g. connection refused, TLS
                // error, DNS failure) is the transport-failure branch where
                // reqwest's Display appends " for url (<full-url>)", which
                // can carry a secret query string (Adzuna app_key, Comeet
                // token, ...). Strip it before it becomes AppError::Network.
                last_err = Some(AppError::Network(e.without_url().to_string()));
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

/// Parse a `Retry-After` header into milliseconds to wait.
///
/// Handles the integer-seconds form: `Retry-After: 5`.
/// HTTP-date form is not parsed (no extra dependency) — callers fall back to
/// exponential backoff when the header is absent or non-numeric.
///
/// Returns `None` when the header is absent or non-numeric.
/// The returned value is clamped to 30 000 ms.
fn retry_after_ms(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let value = headers.get("retry-after").and_then(|v| v.to_str().ok())?;

    // Integer seconds only — the format almost all APIs use in practice.
    let secs = value.trim().parse::<u64>().ok()?;
    // saturating_mul prevents debug-mode panic (and release-mode wrap) when a
    // hostile board sends a huge value; .min(30_000) caps the actual wait.
    Some(secs.saturating_mul(1_000).min(30_000))
}

/// Fetch a URL and deserialize a 2xx JSON body into `T`.
///
/// Failures are **representable** — a caller can tell these three apart:
/// - **(a) HTTP failure** — a non-2xx response returns `Err(AppError::Provider("HTTP <status>"))`,
///   carrying the status code.
/// - **(b) schema drift** — a 2xx body that does not deserialize into `T` returns
///   `Err(AppError::Parse(_))` (serde detail is logged, not returned, so no body value crosses IPC).
/// - **(c) success (incl. empty)** — a 2xx body that deserializes returns `Ok(T)`; a genuinely empty
///   payload (e.g. `[]`/`{}`) is `Ok(empty)`, never conflated with a failure.
///
/// Transport/cancellation faults from [`fetch_text`] (`AppError::Cancelled`/`Network`/`Validation`)
/// propagate unchanged via `?`.
pub async fn fetch_json<T: for<'de> serde::Deserialize<'de>>(
    url: &str,
    opts: FetchOptions,
    signal: tokio_util::sync::CancellationToken,
) -> AppResult<T> {
    // Merge `accept: application/json` into caller headers.
    // Caller-supplied headers win — if the caller already set an `accept` header we
    // leave it as-is; otherwise we prepend the JSON accept so fetch_text's broader
    // HTML accept doesn't take precedence. This preserves auth headers like X-API-Key.
    // Clone headers so we can use `..opts` below — avoids silently dropping any
    // future FetchOptions field that would otherwise be missed by field-by-field copy.
    let mut merged_headers = opts.headers.clone().unwrap_or_default();
    let has_accept = merged_headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("accept"));
    if !has_accept {
        merged_headers.insert(0, ("accept".to_string(), "application/json".to_string()));
    }
    let res = fetch_text(
        url,
        FetchOptions {
            headers: Some(merged_headers),
            ..opts
        },
        signal,
    )
    .await?;

    // Log scheme+host+port+path only — never query (may contain API
    // secrets like Adzuna app_key) or userinfo/fragment.
    let safe_url = reqwest::Url::parse(url)
        .map(|u| {
            let port = u.port().map(|p| format!(":{p}")).unwrap_or_default();
            match u.host_str() {
                Some(host) => format!("{}://{}{}{}", u.scheme(), host, port, u.path()),
                None => format!("{}:{}", u.scheme(), u.path()),
            }
        })
        .unwrap_or_else(|_| "<unparseable-url>".to_string());

    if res.status_code < 200 || res.status_code >= 300 {
        log::warn!(
            "[scraping::http] non-2xx HTTP {} for {safe_url}",
            res.status_code
        );
        // (a) Upstream non-2xx — carry the status code so a blocked/rotted/mistyped
        // board is distinguishable from a genuine empty result, not a silent `Ok`.
        return Err(AppError::Provider(format!("HTTP {}", res.status_code)));
    }

    serde_json::from_str::<T>(&res.text).map_err(|e| {
        log::warn!(
            "[scraping::http] fetch_json parse failure for {safe_url} ({e}); body_len={}",
            res.text.len()
        );
        // (b) Schema drift. The serde detail is logged above but kept out of the
        // returned message so no body value can cross IPC → renderer.
        AppError::Parse("response body did not match the expected schema".to_string())
    })
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

/// Convert HTML to Markdown for job description fields.
///
/// When the input contains HTML tags (detected by a simple `<tag>` scan) it is
/// passed through `htmd` for faithful structure preservation — headings, lists,
/// bold, links — so the frontend can render the description with react-markdown
/// instead of a flat text blob.
///
/// When the input has NO HTML tags (plain text or already-markdown, which is
/// common for Adzuna/JSearch descriptions) it is returned as-is (trimmed).
/// Running htmd on already-markdown text is destructive: htmd escapes markdown
/// special chars (`*`, `_`, etc.) in text nodes, turning `**bold**` into
/// `\*\*bold\*\*`, which react-markdown renders as literal `**`.
///
/// Falls back to [`html_to_text`] on any htmd error so we never regress to
/// `strip_html`'s whitespace-collapsing behaviour.
pub fn html_to_markdown(html: &str) -> String {
    let trimmed_input = html.trim();
    if trimmed_input.is_empty() {
        return String::new();
    }

    // If the input has no HTML tags, treat it as plain text / already-markdown
    // and return it as-is — skipping htmd avoids escaping existing ** markers.
    if !HTML_TAG_RE.is_match(trimmed_input) {
        return trimmed_input.to_string();
    }

    match htmd::convert(trimmed_input) {
        Ok(md) => {
            let trimmed = md.trim().to_string();
            if trimmed.is_empty() {
                // htmd produced nothing; fall through to the text fallback.
                html_to_text(trimmed_input)
            } else {
                trimmed
            }
        }
        Err(e) => {
            log::warn!(
                "[scraping::http] htmd conversion failed ({e}); falling back to html_to_text"
            );
            html_to_text(trimmed_input)
        }
    }
}

#[cfg(test)]
mod test;
