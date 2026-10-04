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

/// Shared accumulation guard behind [`read_bytes_capped`] (and therefore
/// [`read_text_capped`]/[`read_json_capped`], both built on it): a cheap
/// `Content-Length` pre-check for honest servers, then a streamed
/// accumulation that aborts the moment the RUNNING TOTAL exceeds `cap` — not
/// merely a single chunk's length — so a server that lies about or omits
/// `Content-Length`, or that simply dribbles an oversized body out in chunks
/// each individually under `cap`, still can't drive us into OOM. Generic over
/// the chunk type (`B: AsRef<[u8]>`) so it is unit-testable against a
/// synthetic `futures::stream::iter` of plain `Vec<u8>` chunks without
/// constructing a real `reqwest::Response`.
async fn accumulate_capped<S, B>(
    mut stream: S,
    content_length: Option<u64>,
    cap: usize,
) -> crate::error::AppResult<Vec<u8>>
where
    S: futures::Stream<Item = reqwest::Result<B>> + Unpin,
    B: AsRef<[u8]>,
{
    use crate::error::AppError;

    if let Some(content_length) = content_length {
        if content_length > cap as u64 {
            return Err(AppError::Validation("Response too large".to_string()));
        }
    }

    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        // .without_url() — reqwest::Error's Display embeds the full request URL
        // (incl. query string), which can carry secrets like an API token/key;
        // strip it before it reaches an AppError that may cross IPC → renderer.
        let chunk = chunk.map_err(|e| AppError::Network(e.without_url().to_string()))?;
        let chunk = chunk.as_ref();
        if buf.len().saturating_add(chunk.len()) > cap {
            return Err(AppError::Validation("Response too large".to_string()));
        }
        buf.extend_from_slice(chunk);
    }

    Ok(buf)
}

/// Read a response body as raw bytes, refusing to buffer more than `cap`
/// bytes. The fleet-wide chokepoint for bounded body reads — `pub(crate)` so
/// every subsystem holding a `reqwest::Response` (scrapers via
/// [`fetch_text`](crate::scraping::http::fetch_text), the SSRF-guarded
/// [`get_guarded`]/[`get_guarded_following_redirects`] for attacker-influenced
/// URLs, geocoding, profile import, the LinkedIn client's manual gzip decode,
/// …) can bound its read without buffering the whole thing first. See
/// [`accumulate_capped`] for the guard itself.
pub(crate) async fn read_bytes_capped(
    response: reqwest::Response,
    cap: usize,
) -> crate::error::AppResult<Vec<u8>> {
    let content_length = response.content_length();
    accumulate_capped(response.bytes_stream(), content_length, cap).await
}

/// Read a response body as text, refusing to buffer more than `cap` bytes
/// (via [`read_bytes_capped`]). The charset comes from `Content-Type`,
/// mirroring what `reqwest::Response::text()` does internally, so German
/// umlauts / € decode correctly regardless of the cap.
pub(crate) async fn read_text_capped(
    response: reqwest::Response,
    cap: usize,
) -> crate::error::AppResult<String> {
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

    let buf = read_bytes_capped(response, cap).await?;
    let (cow, _enc, _had_errors) = encoding.decode(&buf);
    Ok(cow.into_owned())
}

/// Read + parse a JSON body via [`read_bytes_capped`] + `serde_json::from_slice`
/// — matching `reqwest::Response::json()`'s own UTF-8-only semantics (RFC 8259
/// §8.1 requires JSON-for-interchange to be UTF-8) instead of sniffing
/// `Content-Type`'s charset the way [`read_text_capped`] does for HTML/text.
/// A gateway mislabelling a UTF-8 JSON body's charset must not corrupt it via
/// the wrong decode, and a charset `encoding_rs` maps to the replacement
/// encoding (`iso-2022-jp`, `hz-gb-2312`, …) must not blank an otherwise-valid
/// body out to a single U+FFFD.
///
/// On a schema/parse failure, the serde detail — and the body itself — never
/// reach the returned error, only a generic message does; the detail (line,
/// column, kind — never the body content) is logged instead. Mirrors
/// `scraping::http::fetch_json`'s schema-drift handling, generalized to any
/// caller holding a `reqwest::Response` (not just the scraper `fetch_text`
/// path).
pub(crate) async fn read_json_capped<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    cap: usize,
) -> crate::error::AppResult<T> {
    use crate::error::AppError;

    let bytes = read_bytes_capped(response, cap).await?;
    serde_json::from_slice::<T>(&bytes).map_err(|e| {
        log::warn!(
            "[net::http] read_json_capped: response did not match the expected schema \
             (line={}, column={}, kind={:?}); body_len={}",
            e.line(),
            e.column(),
            e.classify(),
            bytes.len()
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
mod tests;
