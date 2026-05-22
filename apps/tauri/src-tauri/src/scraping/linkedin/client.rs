#![allow(dead_code)]
use super::session::LinkedInSessionData;
use super::rate_limiter::RateLimiter;
use anyhow::Result;
use flate2::read::GzDecoder;
use reqwest::Client;
use std::io::Read;
use std::time::Duration;

pub struct LinkedInHttpClient {
    session_data: Option<LinkedInSessionData>,
    user_agent: String,
    client: Client,
    rate_limiter: RateLimiter,
}

impl LinkedInHttpClient {
    pub fn new(session_data: Option<LinkedInSessionData>) -> Self {
        Self {
            session_data,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36".to_string(),
            client: Client::builder()
                .pool_max_idle_per_host(10)
                .pool_idle_timeout(Duration::from_secs(60))
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            rate_limiter: super::rate_limiter::linkedin_rate_limiter(),
        }
    }

    pub fn update_session(&mut self, session_data: LinkedInSessionData) {
        self.session_data = Some(session_data);
    }

    fn get_default_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            self.user_agent.parse().unwrap(),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8"
                .parse()
                .unwrap(),
        );
        headers.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            "en-US,en;q=0.9,de;q=0.8".parse().unwrap(),
        );
        headers.insert(
            reqwest::header::ACCEPT_ENCODING,
            "gzip, deflate, br".parse().unwrap(),
        );
        headers.insert("DNT", "1".parse().unwrap());
        headers.insert(
            reqwest::header::CONNECTION,
            "keep-alive".parse().unwrap(),
        );
        headers.insert("Upgrade-Insecure-Requests", "1".parse().unwrap());
        headers.insert("Sec-Fetch-Dest", "document".parse().unwrap());
        headers.insert("Sec-Fetch-Mode", "navigate".parse().unwrap());
        headers.insert("Sec-Fetch-Site", "none".parse().unwrap());
        headers.insert("Sec-Fetch-User", "?1".parse().unwrap());
        headers.insert(
            reqwest::header::CACHE_CONTROL,
            "max-age=0".parse().unwrap(),
        );

        // Add session cookies if available
        if let Some(ref session) = self.session_data {
            let cookie_value = if let Some(ref jsession) = session.jsession_id {
                format!("li_at={}; JSESSIONID={}", session.li_at, jsession)
            } else {
                format!("li_at={}", session.li_at)
            };
            headers.insert(
                reqwest::header::COOKIE,
                cookie_value.parse().unwrap(),
            );

            // Add CSRF token if available
            if let Some(ref csrf) = session.csrf_token {
                headers.insert("X-CSRF-Token", csrf.parse().unwrap());
                headers.insert("csrf-token", csrf.parse().unwrap());
            }
        }

        headers
    }

    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        signal: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<T> {
        if let Some(signal) = signal {
            if signal.is_cancelled() {
                return Err(anyhow::anyhow!("Request aborted"));
            }
        }

        self.rate_limiter.wait_for_slot().await;

        let headers = self.get_default_headers();
        let response = self.client.get(url).headers(headers).send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP {}: Request failed", status));
        }

        let bytes = response.bytes().await?;
        let body = if bytes.starts_with(&[0x1f, 0x8b]) {
            // Gzip magic number
            let mut decoder = GzDecoder::new(&bytes[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            String::from_utf8(decompressed)?
        } else {
            String::from_utf8(bytes.to_vec())?
        };

        self.rate_limiter.record_request().await;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn get_html(
        &self,
        url: &str,
        signal: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<String> {
        if let Some(signal) = signal {
            if signal.is_cancelled() {
                return Err(anyhow::anyhow!("Request aborted"));
            }
        }

        eprintln!("[LinkedIn] GET {}", url);
        eprintln!("[LinkedIn] Has session: {}", self.session_data.is_some());

        self.rate_limiter.wait_for_slot().await;

        let headers = self.get_default_headers();
        let response = self.client.get(url).headers(headers).send().await?;

        let status = response.status();
        let content_type = response.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown");
        
        eprintln!("[LinkedIn] Response: {} {}", status, content_type);

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|_| String::from("<no body>"));
            eprintln!("[LinkedIn] Error response body (first 500 chars): {}", &error_body[..error_body.len().min(500)]);
            return Err(anyhow::anyhow!("HTTP {}: Request failed", status));
        }

        let bytes = response.bytes().await?;
        let body = if bytes.starts_with(&[0x1f, 0x8b]) {
            // Gzip magic number
            eprintln!("[LinkedIn] Decompressing gzip response");
            let mut decoder = GzDecoder::new(&bytes[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            String::from_utf8(decompressed)?
        } else {
            String::from_utf8(bytes.to_vec())?
        };

        eprintln!("[LinkedIn] Response body length: {} bytes", body.len());
        eprintln!("[LinkedIn] Response preview (first 200 chars): {}", &body[..body.len().min(200)]);

        self.rate_limiter.record_request().await;
        Ok(body)
    }

    pub fn has_session(&self) -> bool {
        self.session_data.is_some()
    }
}
