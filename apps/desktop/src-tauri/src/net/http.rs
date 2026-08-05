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
//! * [`read_text_capped`]/[`read_bytes_capped`]/[`read_json_capped`] bound every
//!   response read in the fleet — a hostile/misconfigured endpoint can't drive
//!   the process into OOM by streaming an unbounded body.

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;

/// Default desktop user-agent. Individual requests may override the `User-Agent`
/// header (e.g. geocoding identifies itself to Photon).
pub const DEFAULT_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

fn base_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .use_rustls_tls()
        .user_agent(DEFAULT_UA)
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(60))
        .redirect(redirect_policy())
}

/// Mirrors `reqwest::redirect::Policy`'s own built-in default (`limited(10)`)
/// so wrapping it in [`redirect_policy`]'s SSRF guard doesn't silently drop
/// that ceiling — a custom policy does not get the default cap for free.
const MAX_REDIRECTS: usize = 10;

/// Fleet-wide redirect-target SSRF guard. Every client built from
/// [`base_builder`] — [`shared`], [`build_client`], and therefore every
/// subsystem that composes them (scrapers, AI providers, geocoding, profile
/// import) — rejects a redirect hop whose target is a non-`http(s)` scheme or
/// a private/loopback/link-local/unique-local IP literal, at this one
/// chokepoint. [`get_guarded`]/[`get_guarded_following_redirects`] keep their
/// own stricter `Policy::none()` + IP-pinned hop-by-hop validation for
/// attacker-controlled URLs — this guard is the equivalent floor for the
/// pooled client every named-board scraper uses via `fetch_json`/`fetch_text`.
///
/// ponytail: `redirect::Policy::custom` takes a **synchronous** closure, so
/// it can only check the redirect target's literal scheme/host — it cannot
/// perform an async DNS lookup the way `get_guarded`'s IP-pinned flow does.
/// A *hostname* redirect target that itself resolves to a private IP (DNS
/// rebinding) is therefore NOT caught here, only IP-literal targets (the
/// common SSRF payload, e.g. `Location: http://169.254.169.254/…`) and
/// non-http(s) schemes are. Closing the DNS-rebinding gap fully would mean
/// routing every scraper request through the guarded IP-pinned path, which
/// is a much larger change than this fleet-wide hardening pass; also caps
/// the hop count so a custom policy doesn't lose reqwest's own redirect-loop
/// protection.
fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }
        if is_allowed_redirect_target(attempt.url()) {
            attempt.follow()
        } else {
            attempt.error(
                "blocked redirect to a private/loopback/link-local host or non-http(s) scheme",
            )
        }
    })
}

/// Pure predicate behind [`redirect_policy`] — extracted so the SSRF decision
/// is unit-testable without constructing a `reqwest::redirect::Attempt`
/// (its fields are private to `reqwest`, so the policy closure itself can't
/// be exercised directly from this crate's tests).
fn is_allowed_redirect_target(url: &reqwest::Url) -> bool {
    if url.scheme() != "http" && url.scheme() != "https" {
        return false;
    }
    match url.host_str() {
        Some(host) => crate::net::ssrf::is_safe_public_host(host),
        None => false,
    }
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

/// Default byte cap for [`read_text_capped`]/[`read_bytes_capped`]/
/// [`read_json_capped`] when a caller has no budget of its own. Matches
/// `scraping::http::MAX_BYTES` (kept separately there — that module predates
/// this helper and layers its own per-request `FetchOptions::max_bytes`
/// override on top).
pub(crate) const DEFAULT_MAX_BODY_BYTES: usize = 8 * 1024 * 1024; // 8 MB

/// Read a response body as text, refusing to buffer more than `cap` bytes.
///
/// Two guards: a cheap `Content-Length` pre-check for honest servers, then a
/// streamed accumulation that aborts the moment the running total exceeds `cap`
/// — so a server that lies about or omits `Content-Length` still can't drive us
/// into OOM. The charset comes from `Content-Type`, mirroring what
/// `reqwest::Response::text()` does internally, so German umlauts / € decode
/// correctly regardless of the cap.
///
/// The fleet-wide chokepoint for bounded body reads — `pub(crate)` so every
/// subsystem holding a `reqwest::Response` (scrapers via
/// [`fetch_text`](crate::scraping::http::fetch_text), the SSRF-guarded
/// [`get_guarded`]/[`get_guarded_following_redirects`] for attacker-influenced
/// URLs, geocoding, profile import, the LinkedIn client, …) can bound its read
/// without buffering the whole thing first.
pub(crate) async fn read_text_capped(
    response: reqwest::Response,
    cap: usize,
) -> crate::error::AppResult<String> {
    use crate::error::AppError;

    if let Some(content_length) = response.content_length() {
        if content_length > cap as u64 {
            return Err(AppError::Validation("Response too large".to_string()));
        }
    }

    let encoding = response
        .headers()
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

    let mut stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        // .without_url() — reqwest::Error's Display embeds the full request URL
        // (incl. query string), which can carry secrets like an API token/key;
        // strip it before it reaches an AppError that may cross IPC → renderer.
        let chunk = chunk.map_err(|e| AppError::Network(e.without_url().to_string()))?;
        if buf.len().saturating_add(chunk.len()) > cap {
            return Err(AppError::Validation("Response too large".to_string()));
        }
        buf.extend_from_slice(&chunk);
    }

    let (cow, _enc, _had_errors) = encoding.decode(&buf);
    Ok(cow.into_owned())
}

/// Read a response body as raw bytes, refusing to buffer more than `cap` bytes.
/// Same two guards as [`read_text_capped`] (a `Content-Length` pre-check plus a
/// streamed accumulation that aborts past `cap`) but skips charset decoding —
/// for callers that decode the body themselves (e.g. a manual gzip-then-UTF-8
/// decode, which is not the `Content-Type`-driven text decode `read_text_capped`
/// performs).
pub(crate) async fn read_bytes_capped(
    response: reqwest::Response,
    cap: usize,
) -> crate::error::AppResult<Vec<u8>> {
    use crate::error::AppError;

    if let Some(content_length) = response.content_length() {
        if content_length > cap as u64 {
            return Err(AppError::Validation("Response too large".to_string()));
        }
    }

    let mut stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Network(e.without_url().to_string()))?;
        if buf.len().saturating_add(chunk.len()) > cap {
            return Err(AppError::Validation("Response too large".to_string()));
        }
        buf.extend_from_slice(&chunk);
    }

    Ok(buf)
}

/// Read + parse a JSON body via [`read_text_capped`]. On a schema/parse
/// failure, the serde detail — and the body itself — never reach the returned
/// error, only a generic message does; the detail is logged instead. Mirrors
/// `scraping::http::fetch_json`'s schema-drift handling, generalized to any
/// caller holding a `reqwest::Response` (not just the scraper `fetch_text`
/// path).
pub(crate) async fn read_json_capped<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    cap: usize,
) -> crate::error::AppResult<T> {
    use crate::error::AppError;

    let text = read_text_capped(response, cap).await?;
    serde_json::from_str::<T>(&text).map_err(|e| {
        log::warn!(
            "[net::http] read_json_capped: response did not match the expected schema ({e}); body_len={}",
            text.len()
        );
        AppError::Parse("response body did not match the expected schema".to_string())
    })
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

    // ── is_allowed_redirect_target: fleet-wide redirect SSRF guard ─────────────
    // Hermetic — pure URL parsing + the `net::ssrf` classifier, no network.

    fn url(s: &str) -> reqwest::Url {
        reqwest::Url::parse(s).unwrap()
    }

    #[test]
    fn is_allowed_redirect_target_accepts_public_https() {
        assert!(is_allowed_redirect_target(&url(
            "https://boards.greenhouse.io/acme/jobs/1"
        )));
    }

    #[test]
    fn is_allowed_redirect_target_rejects_loopback_literal() {
        assert!(!is_allowed_redirect_target(&url("http://127.0.0.1/steal")));
        assert!(!is_allowed_redirect_target(&url("http://[::1]/steal")));
    }

    #[test]
    fn is_allowed_redirect_target_rejects_cloud_metadata_link_local() {
        assert!(!is_allowed_redirect_target(&url(
            "http://169.254.169.254/latest/meta-data/"
        )));
    }

    #[test]
    fn is_allowed_redirect_target_rejects_rfc1918_private_ranges() {
        for u in [
            "http://10.0.0.1/",
            "http://172.16.0.1/",
            "http://192.168.1.1/",
        ] {
            assert!(!is_allowed_redirect_target(&url(u)), "{u} must be blocked");
        }
    }

    #[test]
    fn is_allowed_redirect_target_rejects_ipv6_unique_local() {
        assert!(!is_allowed_redirect_target(&url("http://[fc00::1]/")));
    }

    #[test]
    fn is_allowed_redirect_target_rejects_non_http_scheme() {
        assert!(!is_allowed_redirect_target(&url("file:///etc/passwd")));
        assert!(!is_allowed_redirect_target(&url(
            "ftp://files.example.com/x"
        )));
    }

    #[test]
    fn is_allowed_redirect_target_allows_arbitrary_public_hostname() {
        // A public-looking hostname is allowed here even though it could still
        // resolve to a private IP via DNS rebinding — this synchronous policy
        // cannot resolve DNS. See the `redirect_policy` doc comment.
        assert!(is_allowed_redirect_target(&url("https://example.com/x")));
    }

    // ── redirect_policy: actually wired onto the pooled client ─────────────────
    // The tests above only prove `is_allowed_redirect_target` is correct in
    // isolation (it has to be extracted — `reqwest::redirect::Attempt` has no
    // public constructor). This test closes the gap: a real round-trip through
    // `shared()` proves `redirect_policy()` is the policy the pooled client
    // actually uses, not merely a well-tested function nobody wired up.
    // NOTE: the redirect target must be REACHABLE (the same MockServer's own
    // `/landed` route) rather than a refusing port like 127.0.0.1:9. A
    // refusing port makes this test tautological — the default (unwired)
    // reqwest redirect policy would ALSO follow the hop, fail to connect, and
    // return `Err`, so the test would pass even if `redirect_policy()` were
    // never wired onto `shared()`. By pointing at a route that genuinely
    // returns 200, an `Err` here can only mean the SSRF guard blocked the
    // hop (loopback is unsafe regardless of reachability) — an unwired
    // client would instead SUCCEED (200, body "landed").
    #[tokio::test]
    async fn shared_client_blocks_redirect_to_loopback_target() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/landed"))
            .respond_with(ResponseTemplate::new(200).set_body_string("landed"))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/start"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("location", format!("{}/landed", mock_server.uri())),
            )
            .mount(&mock_server)
            .await;

        let result = shared()
            .get(format!("{}/start", mock_server.uri()))
            .send()
            .await;

        let err = result.expect_err(
            "shared() must error on a redirect to a loopback target — the target IS \
             reachable (same MockServer's /landed route), so a follow would have \
             succeeded (200) if redirect_policy() were never wired onto the real \
             pooled client",
        );
        // reqwest::Error's Display only prints "error following redirect for url
        // (...)" — the custom message passed to `attempt.error(...)` lives on the
        // wrapped `source`, which Debug (not Display) surfaces. Assert on Debug so
        // this actually proves *why* it failed (our guard) rather than merely that
        // it failed (which a connect error would also satisfy).
        let debug_msg = format!("{err:?}");
        assert!(
            debug_msg.contains("blocked redirect"),
            "error must reflect the redirect-policy block specifically, got: {debug_msg}"
        );
    }

    // ── read_text_capped / read_bytes_capped / read_json_capped ────────────────
    // Moved here from `scraping::http` (this is now the fleet-wide chokepoint).

    #[tokio::test]
    async fn read_text_capped_returns_the_body_under_the_cap() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Hello World"))
            .mount(&mock_server)
            .await;

        let response = reqwest::get(mock_server.uri()).await.unwrap();
        assert_eq!(
            read_text_capped(response, 1024).await.unwrap(),
            "Hello World"
        );
    }

    /// Proves the STREAMING guard (not just the cheap `Content-Length`
    /// pre-check) actually fires: this response carries no `Content-Length`
    /// (asserted below), so the only thing that can catch an oversized body is
    /// the streamed accumulation inside `read_text_capped` aborting once the
    /// running total exceeds `cap`.
    #[tokio::test]
    async fn read_text_capped_rejects_a_body_over_the_cap_without_content_length() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                // A manually-set `Transfer-Encoding: chunked` header stops hyper's
                // H1 encoder from auto-computing `Content-Length` (the two are
                // mutually exclusive per RFC 9112) — the response arrives with NO
                // declared length, forcing reqwest's `content_length()` to `None`
                // and putting the entire burden of catching an oversized body on
                // the streamed accumulation below, not the cheap pre-check.
                ResponseTemplate::new(200)
                    .insert_header("transfer-encoding", "chunked")
                    .set_body_string("x".repeat(4096)),
            )
            .mount(&mock_server)
            .await;

        let response = reqwest::get(mock_server.uri()).await.unwrap();
        assert!(
            response.content_length().is_none(),
            "this test only proves the streaming guard if wiremock sent no \
             Content-Length — otherwise the cheap pre-check would catch it first"
        );
        let err = read_text_capped(response, 64)
            .await
            .expect_err("a body over the cap must not be buffered");
        assert!(
            format!("{err}").contains("too large"),
            "expected a size error, got: {err}"
        );
    }

    #[tokio::test]
    async fn read_bytes_capped_returns_the_body_under_the_cap() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1, 2, 3, 4]))
            .mount(&mock_server)
            .await;

        let response = reqwest::get(mock_server.uri()).await.unwrap();
        let bytes = read_bytes_capped(response, 64).await.unwrap();
        assert_eq!(bytes, vec![1, 2, 3, 4]);
    }

    /// Same streaming-guard proof as `read_text_capped_rejects_a_body_over_the_
    /// cap_without_content_length`, for the bytes variant used by the LinkedIn
    /// client's manual gzip decode path.
    #[tokio::test]
    async fn read_bytes_capped_rejects_a_body_over_the_cap_without_content_length() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                // See the identical `read_text_capped` variant of this test for why
                // `Transfer-Encoding: chunked` is required to actually suppress
                // `Content-Length` here.
                ResponseTemplate::new(200)
                    .insert_header("transfer-encoding", "chunked")
                    .set_body_bytes(vec![0u8; 4096]),
            )
            .mount(&mock_server)
            .await;

        let response = reqwest::get(mock_server.uri()).await.unwrap();
        assert!(
            response.content_length().is_none(),
            "this test only proves the streaming guard if wiremock sent no Content-Length"
        );
        let err = read_bytes_capped(response, 64)
            .await
            .expect_err("a body over the cap must not be buffered");
        assert!(
            format!("{err}").contains("too large"),
            "expected a size error, got: {err}"
        );
    }

    /// Mirrors `scraping::http::test::test_fetch_json_invalid` — a body that
    /// doesn't deserialize into the target type returns the static no-leak
    /// `AppError::Parse` message, never the serde detail or the body itself.
    #[tokio::test]
    async fn read_json_capped_returns_generic_error_on_parse_failure() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&mock_server)
            .await;

        let response = reqwest::get(mock_server.uri()).await.unwrap();
        let err = read_json_capped::<serde_json::Value>(response, DEFAULT_MAX_BODY_BYTES)
            .await
            .expect_err("invalid json must fail to parse");
        match err {
            AppError::Parse(msg) => {
                assert_eq!(msg, "response body did not match the expected schema");
                assert!(
                    !msg.contains("not json"),
                    "Parse message must not contain the mock response body: {msg:?}"
                );
            }
            other => panic!("expected a Parse error on schema drift, got {other:?}"),
        }
    }
}
