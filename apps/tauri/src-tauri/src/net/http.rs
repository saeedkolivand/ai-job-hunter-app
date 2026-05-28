//! Centralized HTTP client infrastructure.
//!
//! This module is the **sole** caller of `reqwest::Client::builder()` /
//! `reqwest::Client::new()` — a CI guardrail enforces this. Every subsystem
//! (AI providers, scrapers, geocoding, research, profile import) composes
//! [`shared`] (or [`build_client`] for stateful variations) instead of building
//! its own client.
//!
//! Design:
//! * **One pooled client** ([`shared`]), built once and cheaply cloned. It has
//!   **no global timeout** — callers set per-request timeouts via
//!   `RequestBuilder::timeout`, so the 5s…3600s spread across the app reuses the
//!   same connection pool.
//! * **One TLS backend** (rustls). Unified deliberately so there is a single
//!   network behavior to reason about.
//! * [`build_client`] for the one stateful case: a per-session cookie jar
//!   (board login). Same rustls/pool/UA base.

use std::sync::Arc;
use std::time::Duration;

/// Default desktop user-agent. Individual requests may override the `User-Agent`
/// header (e.g. geocoding identifies itself to Nominatim).
pub const DEFAULT_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

fn base_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .use_rustls_tls()
        .user_agent(DEFAULT_UA)
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(60))
}

/// The single pooled HTTP client. Built on first use, then cloned (cheap — a
/// `reqwest::Client` is internally reference-counted). No global timeout: set one
/// per request with `.timeout(..)`.
pub fn shared() -> reqwest::Client {
    static SHARED: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    SHARED
        .get_or_init(|| base_builder().build().expect("failed to build shared HTTP client"))
        .clone()
}

/// Per-client configuration for stateful variations that cannot share the global
/// pool (currently: a per-session cookie jar).
#[derive(Default)]
pub struct ClientConfig {
    pub timeout: Option<Duration>,
    pub cookie_jar: Option<Arc<reqwest::cookie::Jar>>,
}

/// Build a dedicated client from [`ClientConfig`], on the same rustls/pool/UA
/// base as [`shared`]. Use only when a request-scoped client cannot reuse the
/// shared pool (e.g. it needs its own cookie jar).
pub fn build_client(cfg: ClientConfig) -> reqwest::Result<reqwest::Client> {
    let mut builder = base_builder();
    if let Some(timeout) = cfg.timeout {
        builder = builder.timeout(timeout);
    }
    if let Some(jar) = cfg.cookie_jar {
        builder = builder.cookie_provider(jar);
    }
    builder.build()
}
