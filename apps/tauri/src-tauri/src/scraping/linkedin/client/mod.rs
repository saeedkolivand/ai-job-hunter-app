use super::rate_limiter::RateLimiter;
use super::session::LinkedInSessionData;
use crate::error::{AppError, AppResult};
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
            user_agent: crate::net::http::DEFAULT_UA.to_string(),
            client: crate::net::http::build_client(crate::net::http::ClientConfig {
                timeout: Some(Duration::from_secs(30)),
                ..Default::default()
            })
            .expect("failed to build LinkedIn HTTP client"),
            rate_limiter: super::rate_limiter::linkedin_rate_limiter(),
        }
    }

    pub fn update_session(&mut self, session_data: LinkedInSessionData) {
        self.session_data = Some(session_data);
    }

    fn get_default_headers(&self) -> AppResult<reqwest::header::HeaderMap> {
        use reqwest::header::HeaderValue;

        let mut headers = reqwest::header::HeaderMap::new();
        // Runtime UA value: propagate a parse failure instead of panicking.
        headers.insert(
            reqwest::header::USER_AGENT,
            self.user_agent
                .parse()
                .map_err(|_| AppError::Config("invalid user-agent header".to_string()))?,
        );
        // Static literals cannot fail — `from_static` is infallible.
        headers.insert(
            reqwest::header::ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            ),
        );
        headers.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("en-US,en;q=0.9,de;q=0.8"),
        );
        headers.insert(
            reqwest::header::ACCEPT_ENCODING,
            HeaderValue::from_static("gzip, deflate, br"),
        );
        headers.insert("DNT", HeaderValue::from_static("1"));
        headers.insert(
            reqwest::header::CONNECTION,
            HeaderValue::from_static("keep-alive"),
        );
        headers.insert(
            "Upgrade-Insecure-Requests",
            HeaderValue::from_static("1"),
        );
        headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("document"));
        headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
        headers.insert("Sec-Fetch-Site", HeaderValue::from_static("none"));
        headers.insert("Sec-Fetch-User", HeaderValue::from_static("?1"));
        headers.insert(
            reqwest::header::CACHE_CONTROL,
            HeaderValue::from_static("max-age=0"),
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
                cookie_value
                    .parse()
                    .map_err(|_| AppError::Config("invalid session cookie header".to_string()))?,
            );

            // Add CSRF token if available
            if let Some(ref csrf) = session.csrf_token {
                let csrf_value: HeaderValue = csrf
                    .parse()
                    .map_err(|_| AppError::Config("invalid CSRF token header".to_string()))?;
                headers.insert("X-CSRF-Token", csrf_value.clone());
                headers.insert("csrf-token", csrf_value);
            }
        }

        Ok(headers)
    }

    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        signal: Option<&tokio_util::sync::CancellationToken>,
    ) -> AppResult<T> {
        if let Some(signal) = signal {
            if signal.is_cancelled() {
                return Err(AppError::Cancelled);
            }
        }

        self.rate_limiter.wait_for_slot().await;

        let headers = self.get_default_headers()?;
        let response = self.client.get(url).headers(headers).send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Network(format!("HTTP {status}: Request failed")));
        }

        let bytes = response.bytes().await?;
        let body = if bytes.starts_with(&[0x1f, 0x8b]) {
            // Gzip magic number
            let mut decoder = GzDecoder::new(&bytes[..]);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| AppError::Parse(format!("gzip decode failed: {e}")))?;
            String::from_utf8(decompressed)
                .map_err(|e| AppError::Parse(format!("response was not valid UTF-8: {e}")))?
        } else {
            String::from_utf8(bytes.to_vec())
                .map_err(|e| AppError::Parse(format!("response was not valid UTF-8: {e}")))?
        };

        self.rate_limiter.record_request().await;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn get_html(
        &self,
        url: &str,
        signal: Option<&tokio_util::sync::CancellationToken>,
    ) -> AppResult<String> {
        if let Some(signal) = signal {
            if signal.is_cancelled() {
                return Err(AppError::Cancelled);
            }
        }

        self.rate_limiter.wait_for_slot().await;

        let headers = self.get_default_headers()?;
        let response = self.client.get(url).headers(headers).send().await?;

        let status = response.status();

        if !status.is_success() {
            let _error_body = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("<no body>"));
            return Err(AppError::Network(format!("HTTP {status}: Request failed")));
        }

        let bytes = response.bytes().await?;
        let body = if bytes.starts_with(&[0x1f, 0x8b]) {
            // Gzip magic number
            let mut decoder = GzDecoder::new(&bytes[..]);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| AppError::Parse(format!("gzip decode failed: {e}")))?;
            String::from_utf8(decompressed)
                .map_err(|e| AppError::Parse(format!("response was not valid UTF-8: {e}")))?
        } else {
            String::from_utf8(bytes.to_vec())
                .map_err(|e| AppError::Parse(format!("response was not valid UTF-8: {e}")))?
        };

        self.rate_limiter.record_request().await;
        Ok(body)
    }

    pub fn has_session(&self) -> bool {
        self.session_data.is_some()
    }
}

#[cfg(test)]
mod test;
