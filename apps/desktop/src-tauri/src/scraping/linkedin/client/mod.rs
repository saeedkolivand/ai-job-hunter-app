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
    rate_limiter: &'static RateLimiter,
}

impl LinkedInHttpClient {
    pub fn new(session_data: Option<LinkedInSessionData>) -> AppResult<Self> {
        Ok(Self {
            session_data,
            user_agent: crate::net::http::DEFAULT_UA.to_string(),
            client: crate::net::http::build_client(crate::net::http::ClientConfig {
                timeout: Some(Duration::from_secs(30)),
                ..Default::default()
            })
            .map_err(|e| AppError::Network(format!("failed to build LinkedIn HTTP client: {e}")))?,
            rate_limiter: super::rate_limiter::linkedin_rate_limiter(),
        })
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
        // Advertise ONLY what `get_html` can actually decode. `reqwest` is built
        // with `default-features = false` and no gzip/brotli/deflate feature, so
        // it never auto-decompresses; the manual decode below handles the gzip
        // magic bytes and nothing else. Asking for `br`/`deflate` let the edge
        // answer with a body we then fed to `String::from_utf8`, failing the
        // whole board with a misleading "response was not valid UTF-8".
        headers.insert(
            reqwest::header::ACCEPT_ENCODING,
            HeaderValue::from_static("gzip"),
        );
        headers.insert("DNT", HeaderValue::from_static("1"));
        headers.insert(
            reqwest::header::CONNECTION,
            HeaderValue::from_static("keep-alive"),
        );
        headers.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
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

        let cap = crate::net::http::DEFAULT_MAX_BODY_BYTES;
        let bytes = crate::net::http::read_bytes_capped(response, cap).await?;
        let body = decode_body(bytes, cap)?;

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
            return Err(AppError::Network(format!("HTTP {status}: Request failed")));
        }

        let cap = crate::net::http::DEFAULT_MAX_BODY_BYTES;
        let bytes = crate::net::http::read_bytes_capped(response, cap).await?;
        let body = decode_body(bytes, cap)?;

        self.rate_limiter.record_request().await;
        Ok(body)
    }

    pub fn has_session(&self) -> bool {
        self.session_data.is_some()
    }
}

/// Decode a raw response body: gzip-decompress it first if the gzip magic
/// prefix (`1f 8b`) is present (this client disables `reqwest`'s built-in
/// decompression and requests `gzip` itself — see the `Accept-Encoding`
/// comment in [`LinkedInHttpClient::get_default_headers`] — so it must decode
/// gzip by hand), then UTF-8 decode.
///
/// `bytes` already went through [`crate::net::http::read_bytes_capped`],
/// which bounds only the COMPRESSED wire size at `cap`. DEFLATE can expand up
/// to ~1032:1, so an 8 MB compressed cap could still inflate to gigabytes
/// resident if the decompressed side were left unbounded — `Read::take(cap)`
/// bounds that side too. A decompressed read that lands EXACTLY on `cap` is
/// treated as truncation (rejected) rather than a legitimately cap-sized
/// body, since the two are indistinguishable without decoding further.
fn decode_body(bytes: Vec<u8>, cap: usize) -> AppResult<String> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = GzDecoder::new(&bytes[..]);
        let mut decompressed = Vec::new();
        let read = (&mut decoder)
            .take(cap as u64)
            .read_to_end(&mut decompressed)
            .map_err(|e| AppError::Parse(format!("gzip decode failed: {e}")))?;
        if read == cap {
            return Err(AppError::Validation(
                "decompressed response too large".to_string(),
            ));
        }
        String::from_utf8(decompressed)
            .map_err(|e| AppError::Parse(format!("response was not valid UTF-8: {e}")))
    } else {
        String::from_utf8(bytes)
            .map_err(|e| AppError::Parse(format!("response was not valid UTF-8: {e}")))
    }
}

#[cfg(test)]
mod test;
