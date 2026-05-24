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

        let client = reqwest::Client::new();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_options_default() {
        let opts = FetchOptions::default();
        assert!(opts.headers.is_none());
        assert!(opts.method.is_none());
        assert!(opts.body.is_none());
        assert_eq!(opts.retries, 1);
    }

    #[test]
    fn test_strip_html_basic() {
        let html = "<p>Hello <b>World</b></p>";
        let result = strip_html(html);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_strip_html_script() {
        let html = "<script>alert('xss')</script><p>Content</p>";
        let result = strip_html(html);
        assert_eq!(result, "Content");
    }

    #[test]
    fn test_strip_html_style() {
        let html = "<style>body { color: red; }</style><p>Content</p>";
        let result = strip_html(html);
        assert_eq!(result, "Content");
    }

    #[test]
    fn test_strip_html_entities() {
        let html = "Hello &amp; World &lt;3";
        let result = strip_html(html);
        assert_eq!(result, "Hello & World <3");
    }

    #[test]
    fn test_strip_html_nbsp() {
        let html = "Hello&nbsp;&nbsp;World";
        let result = strip_html(html);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_strip_html_quotes() {
        let html = "&quot;Test&quot; and &#39;quote&#39;";
        let result = strip_html(html);
        assert_eq!(result, "\"Test\" and 'quote'");
    }

    #[test]
    fn test_strip_html_whitespace() {
        let html = "<p>Hello</p>   <p>World</p>";
        let result = strip_html(html);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_strip_html_empty() {
        let html = "";
        let result = strip_html(html);
        assert_eq!(result, "");
    }

    #[test]
    fn test_strip_html_nested_tags() {
        let html = "<div><span><b>Bold</b></span></div>";
        let result = strip_html(html);
        assert_eq!(result, "Bold");
    }

    #[tokio::test]
    async fn test_fetch_text_success() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use wiremock::matchers::method;

        let mock_server = MockServer::start().await;
        let signal = tokio_util::sync::CancellationToken::new();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Hello World"))
            .mount(&mock_server)
            .await;

        let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
        assert!(result.is_ok());
        let fetch_result = result.unwrap();
        assert_eq!(fetch_result.status_code, 200);
        assert_eq!(fetch_result.text, "Hello World");
    }

    #[tokio::test]
    async fn test_fetch_text_404() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use wiremock::matchers::method;

        let mock_server = MockServer::start().await;
        let signal = tokio_util::sync::CancellationToken::new();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
        assert!(result.is_ok());
        let fetch_result = result.unwrap();
        assert_eq!(fetch_result.status_code, 404);
    }

    #[tokio::test]
    async fn test_fetch_json_success() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use wiremock::matchers::method;
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct TestResponse {
            message: String,
        }

        let mock_server = MockServer::start().await;
        let signal = tokio_util::sync::CancellationToken::new();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"message":"test"}"#))
            .mount(&mock_server)
            .await;

        let result = fetch_json::<TestResponse>(&mock_server.uri(), FetchOptions::default(), signal).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_fetch_json_invalid() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use wiremock::matchers::method;

        let mock_server = MockServer::start().await;
        let signal = tokio_util::sync::CancellationToken::new();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid json"))
            .mount(&mock_server)
            .await;

        let result: anyhow::Result<Option<serde_json::Value>> = fetch_json(&mock_server.uri(), FetchOptions::default(), signal).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_fetch_text_cancelled() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use wiremock::matchers::method;

        let mock_server = MockServer::start().await;
        let signal = tokio_util::sync::CancellationToken::new();
        signal.cancel();

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Hello"))
            .mount(&mock_server)
            .await;

        let result = fetch_text(&mock_server.uri(), FetchOptions::default(), signal).await;
        assert!(result.is_err());
    }
}
