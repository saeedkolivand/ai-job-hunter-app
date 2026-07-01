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
        .get_or_init(|| {
            base_builder()
                .build()
                .expect("failed to build shared HTTP client")
        })
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

/// Build a one-off client for [`get_guarded`]: same rustls/pool/UA base as
/// [`shared`] plus a 20s timeout. When `pin` is `Some((host, ips))`, the client
/// pins DNS for `host` to exactly those validated IPs via reqwest's
/// `resolve_to_addrs`, so the actual GET cannot rebind to a different (e.g.
/// freshly-rebound) address between the validation lookup and connect. The
/// `SocketAddr` port carries the real destination port from `lookup_host`.
///
/// Redirects are **disabled** ([`reqwest::redirect::Policy::none`]) on this
/// client only — without it reqwest would follow up to 10 redirects, and a
/// `301 Location: http://169.254.169.254/` (or a rebinding host) on the first
/// hop would bypass the IP pin entirely. A 3xx is surfaced as the response
/// status; `get_guarded`'s callers treat a non-2xx as "no scraper matched".
fn build_guarded_client(
    pin: Option<(String, Vec<std::net::SocketAddr>)>,
) -> crate::error::AppResult<reqwest::Client> {
    use crate::error::AppError;
    let mut builder = base_builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20));
    if let Some((host, addrs)) = pin {
        builder = builder.resolve_to_addrs(&host, &addrs);
    }
    builder.build().map_err(AppError::from)
}

/// IP-validated, IP-pinned GET for fetching an **attacker-controlled** URL (the
/// generic-HTML scrape fallback). Closes the DNS-rebinding TOCTOU on the only
/// egress that fetches a raw user URL:
///
/// 1. Reject non-`http(s)` schemes.
/// 2. If the host is an IP literal, validate it directly ([`crate::net::ssrf::is_safe_ip`])
///    and fetch — hermetic, no DNS (reqwest resolves a literal itself, so pinning
///    is a no-op there).
/// 3. Otherwise resolve the host once, validate **every** returned address, then
///    pin the client to those exact IPs so the connect cannot rebind to a
///    private/loopback address after the check.
///
/// Returns [`crate::error::AppError::Validation`] for an unsafe/rejected host.
pub async fn get_guarded(url: &str) -> crate::error::AppResult<reqwest::Response> {
    use crate::error::AppError;
    use std::net::IpAddr;

    let u =
        reqwest::Url::parse(url).map_err(|e| AppError::Validation(format!("invalid url: {e}")))?;
    match u.scheme() {
        "http" | "https" => {}
        s => return Err(AppError::Validation(format!("unsupported scheme: {s}"))),
    }
    let host = u
        .host_str()
        .ok_or_else(|| AppError::Validation("url has no host".into()))?
        .to_string();
    let port = u
        .port_or_known_default()
        .unwrap_or(if u.scheme() == "https" { 443 } else { 80 });

    let client = if let Ok(ip) = host.parse::<IpAddr>() {
        // IP literal: validate directly, no DNS, no pin (reqwest won't re-resolve
        // a literal so it cannot rebind).
        if !crate::net::ssrf::is_safe_ip(ip) {
            return Err(AppError::Validation(
                "url host resolves to a private/loopback address".into(),
            ));
        }
        build_guarded_client(None)?
    } else {
        let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
            .await
            .map_err(|e| AppError::Validation(format!("dns resolution failed: {e}")))?
            .collect();
        validate_resolved_addrs(&addrs)?;
        build_guarded_client(Some((host.clone(), addrs)))?
    };

    client.get(u).send().await.map_err(AppError::from)
}

/// Like [`get_guarded`] but manually follows up to `max_hops` redirects (so at
/// most `max_hops + 1` total requests: the initial fetch plus one per hop),
/// **re-validating every hop** through [`get_guarded`] — so each redirect target
/// is IP-validated + pinned and an attacker can't bounce us onto a private /
/// loopback / metadata address via a `Location` header (the exact hole
/// [`build_guarded_client`] disables reqwest's own redirect following to avoid).
/// This matches reqwest's own `Policy::limited(n)` semantics (`n` = redirects
/// followed, not total requests). Relative `Location` values resolve against the
/// current URL. Returns the first non-redirect response, or — if the hop budget
/// is exhausted while still redirecting, or a `Location` can't be safely followed
/// — the last 3xx (which callers treat as non-2xx → "no match").
///
/// Used by the generic-HTML resolver so an aggregator `redirect_url` (an Adzuna
/// 30x that bounces to the real posting) reaches the destination ad instead of
/// dying on the first non-2xx hop.
pub async fn get_guarded_following_redirects(
    url: &str,
    max_hops: u8,
) -> crate::error::AppResult<reqwest::Response> {
    let mut current = url.to_string();
    // `max_hops` redirects followed → at most `max_hops + 1` fetches: the initial
    // one below, plus one per loop iteration.
    let mut res = get_guarded(&current).await?;
    for _ in 0..max_hops {
        if !res.status().is_redirection() {
            return Ok(res);
        }
        let location = match res.headers().get(reqwest::header::LOCATION) {
            // non-UTF-8 / absent Location → nothing safe to follow; return the 3xx
            // (caller treats non-2xx as "no match").
            Some(v) => match v.to_str() {
                Ok(s) => s.to_string(),
                Err(_) => return Ok(res),
            },
            None => return Ok(res),
        };
        // Resolve a possibly-relative Location against the current URL. An
        // unparseable/un-joinable Location is also "can't follow safely" → return
        // the 3xx, symmetric with the non-UTF-8 / absent cases above. The next
        // get_guarded re-validates this URL, so no IP check is skipped.
        let next = match reqwest::Url::parse(&current)
            .ok()
            .and_then(|base| base.join(&location).ok())
        {
            Some(u) => u.to_string(),
            None => return Ok(res),
        };
        current = next;
        res = get_guarded(&current).await?;
    }
    // Hop budget exhausted: return the last response (a 3xx here → "no match").
    Ok(res)
}

/// Validate the set of addresses a hostname resolved to. This is the security
/// core of the hostname branch of [`get_guarded`] — the check that actually
/// closes the DNS-rebinding TOCTOU. Rejects the whole set (with
/// [`crate::error::AppError::Validation`]) if it is empty or if **any** resolved
/// address is unsafe ([`crate::net::ssrf::is_safe_ip`]); returns `Ok(())` only
/// when every address is a safe public IP. Behaviorally identical to the prior
/// inline loop in `get_guarded` (same empty-set and any-unsafe rejections, same
/// error variant/message).
fn validate_resolved_addrs(addrs: &[std::net::SocketAddr]) -> crate::error::AppResult<()> {
    use crate::error::AppError;
    if addrs.is_empty() {
        return Err(AppError::Validation("url host did not resolve".into()));
    }
    for sa in addrs {
        if !crate::net::ssrf::is_safe_ip(sa.ip()) {
            return Err(AppError::Validation(
                "url host resolves to a private/loopback address".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    // IP-literal unsafe URLs must be rejected WITHOUT any network/DNS — the
    // literal path is hermetic.
    #[tokio::test]
    async fn get_guarded_rejects_metadata_literal() {
        let err = get_guarded("http://169.254.169.254/").await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn get_guarded_rejects_loopback_v4_literal() {
        let err = get_guarded("http://127.0.0.1/").await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn get_guarded_rejects_private_literal() {
        let err = get_guarded("http://10.0.0.1/").await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn get_guarded_rejects_loopback_v6_literal() {
        let err = get_guarded("http://[::1]/").await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn get_guarded_rejects_non_http_scheme() {
        let err = get_guarded("ftp://1.1.1.1/").await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    // ── get_guarded: literal-branch validation actually runs ──────────────────

    // A private IP literal must be rejected by get_guarded's OWN validation
    // (not merely by the ssrf classifier in isolation) — this exercises the
    // literal branch's `is_safe_ip` gate and the Validation error it returns.
    #[tokio::test]
    async fn get_guarded_private_literal_returns_validation_error() {
        let err = get_guarded("http://192.168.1.1/job").await.unwrap_err();
        assert!(
            matches!(err, AppError::Validation(_)),
            "private IP literal must be a Validation error, got {err:?}"
        );
    }

    // A public IP literal passes get_guarded's validation step. We use TEST-NET-3
    // (203.0.113.0/24, RFC 5737 — documentation/example range, guaranteed
    // unroutable) so the connect cannot reach a real host; whatever the send
    // resolves to, the ONE thing we assert is that get_guarded did NOT reject it
    // at the validation gate. A subsequent connection/timeout error is expected
    // and acceptable — it proves we got *past* validation.
    #[tokio::test]
    async fn get_guarded_public_literal_passes_validation() {
        let result = get_guarded("http://203.0.113.1/job").await;
        if let Err(AppError::Validation(msg)) = &result {
            panic!("public IP literal must pass validation, but was rejected: {msg}");
        }
        // Ok(_) (unlikely against TEST-NET-3) or a non-Validation transport error
        // both confirm validation was passed.
    }

    // ── validate_resolved_addrs: the hostname-branch security core ─────────────
    // This is the post-`lookup_host` gate that closes the DNS-rebinding TOCTOU on
    // the hostname path. Tested hermetically with synthetic resolved sets — no
    // real DNS — so the security logic is asserted independent of the network.

    fn sa(s: &str) -> std::net::SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn validate_resolved_addrs_rejects_empty_set() {
        let err = validate_resolved_addrs(&[]).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[test]
    fn validate_resolved_addrs_rejects_any_private_addr() {
        // A host that resolves to a public AND a private address (a classic
        // rebinding payload) must be rejected because ANY unsafe addr fails.
        let addrs = [sa("1.1.1.1:80"), sa("192.168.1.10:80")];
        let err = validate_resolved_addrs(&addrs).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[test]
    fn validate_resolved_addrs_rejects_loopback_addr() {
        let err = validate_resolved_addrs(&[sa("127.0.0.1:80")]).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[test]
    fn validate_resolved_addrs_rejects_metadata_addr() {
        // 169.254.169.254 — cloud metadata endpoint reached via a rebinding host.
        let err = validate_resolved_addrs(&[sa("169.254.169.254:80")]).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[test]
    fn validate_resolved_addrs_accepts_all_public_set() {
        let addrs = [
            sa("1.1.1.1:443"),
            sa("8.8.8.8:443"),
            sa("[2606:4700:4700::1111]:443"),
        ];
        assert!(
            validate_resolved_addrs(&addrs).is_ok(),
            "an all-public resolved set must pass"
        );
    }

    // The redirect-follower must apply get_guarded's IP validation on the FIRST
    // hop too — an unsafe literal is rejected before any redirect could be followed.
    #[tokio::test]
    async fn following_redirects_rejects_unsafe_first_hop() {
        let err = get_guarded_following_redirects("http://127.0.0.1/", 5)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }
}
