//! Extension-bridge authorization helpers — the origin allowlist and the
//! URL/SSRF host guard. Pure functions with unit tests; no I/O, no app state.
//!
//! The IP/host SSRF classifier itself lives in [`crate::net::ssrf`] (L0) so the
//! IP-pinned guarded fetch ([`crate::net::http::get_guarded`]) can share it;
//! [`is_safe_import_url`] is the thin URL-parsing wrapper the bridge calls.

/// Placeholder published-extension ids. **TODO(bridge): replace these with the
/// real Chrome Web Store / AMO ids before store submission** — until then no
/// production extension origin matches, so only the dev override
/// (`platform::config::extension_dev_origins`) admits a local extension. Each id
/// is matched as `chrome-extension://<id>` AND `moz-extension://<id>`.
pub const ALLOWED_EXTENSION_IDS: &[&str] = &[
    // Chrome Web Store id (32 lowercase a–p chars) — PLACEHOLDER.
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    // Firefox AMO extension id (UUID form) — PLACEHOLDER.
    "00000000-0000-0000-0000-000000000000",
];

/// Whether a handshake `Origin` is an allowed extension origin. Accepts only
/// `chrome-extension://<id>` / `moz-extension://<id>` where `<id>` is in
/// [`ALLOWED_EXTENSION_IDS`], plus any exact-match `dev_origins` entry (a
/// developer's locally-loaded extension, supplied via the dev env override).
pub fn is_allowed_origin(origin: &str, dev_origins: &[String]) -> bool {
    let origin = origin.trim();
    if origin.is_empty() {
        return false;
    }
    // Dev override: exact-match the full origin string.
    if dev_origins.iter().any(|d| d == origin) {
        return true;
    }
    // Production: scheme + known id (no path/extra segments).
    let id = if let Some(rest) = origin.strip_prefix("chrome-extension://") {
        rest
    } else if let Some(rest) = origin.strip_prefix("moz-extension://") {
        rest
    } else {
        return false;
    };
    // Reject anything past the id (e.g. a trailing path) — an origin is just the
    // scheme + host, so a clean id has no `/`.
    if id.contains('/') {
        return false;
    }
    ALLOWED_EXTENSION_IDS.contains(&id)
}

/// SSRF guard for an import target URL: parse the host and reject loopback,
/// private, link-local, and `*.local` hosts so the bridge can't be used to probe
/// the user's internal network. Returns `true` only for a host that is a public
/// hostname or a public IP. A URL that fails to parse a host is rejected.
///
/// This is a fast-fail by host **name**; the actual fetch
/// ([`crate::net::http::get_guarded`]) additionally IP-validates and IP-pins the
/// resolved address to close the DNS-rebinding TOCTOU. The host classifier lives
/// in [`crate::net::ssrf`].
pub fn is_safe_import_url(url: &str) -> bool {
    let parsed = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return false,
    }
    match parsed.host_str() {
        Some(host) => crate::net::ssrf::is_safe_public_host(host),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev() -> Vec<String> {
        vec!["chrome-extension://devdevdevdevdevdevdevdevdevdevde".to_string()]
    }

    #[test]
    fn allows_known_chrome_id() {
        let origin = format!("chrome-extension://{}", ALLOWED_EXTENSION_IDS[0]);
        assert!(is_allowed_origin(&origin, &[]));
    }

    #[test]
    fn allows_known_firefox_id() {
        let origin = format!("moz-extension://{}", ALLOWED_EXTENSION_IDS[1]);
        assert!(is_allowed_origin(&origin, &[]));
    }

    #[test]
    fn rejects_unknown_id() {
        assert!(!is_allowed_origin(
            "chrome-extension://unknownunknownunknownunknownunkn",
            &[]
        ));
    }

    #[test]
    fn rejects_non_extension_scheme() {
        assert!(!is_allowed_origin("https://evil.example.com", &[]));
        assert!(!is_allowed_origin("http://localhost", &[]));
        assert!(!is_allowed_origin("", &[]));
    }

    #[test]
    fn rejects_id_with_trailing_path() {
        let origin = format!("chrome-extension://{}/popup.html", ALLOWED_EXTENSION_IDS[0]);
        assert!(!is_allowed_origin(&origin, &[]));
    }

    #[test]
    fn dev_override_exact_match() {
        let origins = dev();
        assert!(is_allowed_origin(&origins[0], &origins));
        // A different id not in the dev list is still rejected.
        assert!(!is_allowed_origin(
            "chrome-extension://otherotherotherotherotherotherot",
            &origins
        ));
    }

    // ── SSRF URL guard (host classifier itself is tested in crate::net::ssrf) ──

    #[test]
    fn is_safe_import_url_rejects_non_http_and_private() {
        assert!(!is_safe_import_url("file:///etc/passwd"));
        assert!(!is_safe_import_url("ftp://example.com/x"));
        assert!(!is_safe_import_url("http://127.0.0.1/admin"));
        assert!(!is_safe_import_url(
            "http://169.254.169.254/latest/meta-data"
        ));
        assert!(!is_safe_import_url("not a url"));
    }

    #[test]
    fn is_safe_import_url_allows_public_http() {
        assert!(is_safe_import_url(
            "https://boards.greenhouse.io/acme/jobs/1"
        ));
        assert!(is_safe_import_url("http://jobs.example.com/posting/42"));
    }
}
