//! Extension-bridge authorization helpers — the origin allowlist and the
//! URL/SSRF host guard. Pure functions with unit tests; no I/O, no app state.
//!
//! The IP/host SSRF classifier itself lives in [`crate::net::ssrf`] (L0) so the
//! IP-pinned guarded fetch ([`crate::net::http::get_guarded`]) can share it;
//! [`is_safe_import_url`] is the thin URL-parsing wrapper the bridge calls.

/// Allowed published **Chrome** extension ids. The Chrome `chrome-extension://`
/// host IS the stable Chrome Web Store id, so a Chrome origin is pinned by
/// exact id match against this list.
///
/// Firefox is **not** pinned here: Firefox assigns every install a random,
/// per-profile internal UUID (anti-fingerprinting) and uses that — never the
/// AMO/gecko id — in `moz-extension://` URLs, so the id is unknowable in
/// advance. Firefox origins are therefore accepted by UUID **shape** instead
/// (see [`is_allowed_origin`] / [`is_extension_uuid`]); the gecko id
/// (`job-importer@aijobhunter.app`, in `apps/extension/src/manifest.ts`) never
/// appears as an origin and is intentionally absent from this list.
///
/// The published Chrome Web Store id is now pinned below. The Chrome
/// `chrome-extension://` host IS the stable Web Store id, so the production
/// origin matches by exact id; the dev override
/// (`platform::config::extension_dev_origins`) still admits a local Chrome
/// extension during development.
/// Each id is matched as `chrome-extension://<id>` (Chrome only).
pub const ALLOWED_EXTENSION_IDS: &[&str] = &[
    // Published Chrome Web Store id (32 lowercase a–p chars).
    "oaoekkgkhmgdfnpmfkpphgiikliaicll",
];

/// Whether a handshake `Origin` is an allowed extension origin.
///
/// This check is **defense-in-depth, not the primary boundary**. The real
/// authentication is the per-frame 256-bit pairing token enforced in
/// [`super::classify_frame`] (every envelope token must equal `state.token()`),
/// over a loopback-only (`127.0.0.1`) listener. The token is copied by the user
/// from the app Settings and a sibling extension cannot read it, so even a local
/// extension that opens a socket cannot import anything without it.
///
/// Acceptance:
/// - **Dev override** (checked first): any exact-match `dev_origins` entry (a
///   developer locally-loaded extension, supplied via the dev env override).
/// - **Chrome**: `chrome-extension://<id>` where `<id>` is in
///   [`ALLOWED_EXTENSION_IDS`] (the stable Chrome Web Store id).
/// - **Firefox**: `moz-extension://<uuid>` where `<uuid>` is a well-formed
///   extension UUID ([`is_extension_uuid`]). The Firefox per-install UUID is
///   unknowable in advance, so the origin check can only assert scheme + UUID
///   shape; the pairing token is what actually authenticates.
///
/// In all cases the origin must be scheme + host only — a trailing path or any
/// extra slash segment is rejected.
pub fn is_allowed_origin(origin: &str, dev_origins: &[String]) -> bool {
    let origin = origin.trim();
    if origin.is_empty() {
        return false;
    }
    // Dev override: exact-match the full origin string (checked first).
    if dev_origins.iter().any(|d| d == origin) {
        return true;
    }
    // Firefox: a WebSocket/fetch initiated from an extension BACKGROUND script
    // sends `Origin: null` — Firefox deliberately strips the `moz-extension://`
    // UUID rather than leak it (Bugzilla 1607936 / 1257989). So the real Firefox
    // bridge handshake arrives as `null`, NOT `moz-extension://<uuid>`. Accept
    // it: the origin gate is defense-in-depth only — the actual boundary is the
    // per-frame 256-bit pairing token over a loopback-only (`127.0.0.1`)
    // listener, which a null-origin page cannot satisfy without the token the
    // user copied from the app's Settings.
    if origin == "null" {
        return true;
    }
    // Chrome: scheme + known store id. An origin is just scheme + host, so a
    // clean id has no slash (reject a trailing path / extra segment).
    if let Some(id) = origin.strip_prefix("chrome-extension://") {
        return !id.contains('/') && ALLOWED_EXTENSION_IDS.contains(&id);
    }
    // Firefox, non-background contexts (e.g. a content/popup-initiated socket):
    // `moz-extension://<uuid>`. The id is random per profile and unknowable in
    // advance, so accept any well-formed UUID host (no trailing path —
    // `is_extension_uuid` rejects a slash, which is not a hex/dash char). The
    // common background path is handled by the `null` case above.
    if let Some(host) = origin.strip_prefix("moz-extension://") {
        return is_extension_uuid(host);
    }
    false
}

/// Whether `s` is a well-formed Firefox extension UUID: the standard
/// `8-4-4-4-12` form, lowercase hex, dashes in the canonical positions, and
/// nothing else (no trailing path, no extra segment). Hand-written hex/dash
/// check so the bridge takes no new dependency (no `uuid` crate).
///
/// Example accepted: `12345678-90ab-cdef-1234-567890abcdef`.
fn is_extension_uuid(s: &str) -> bool {
    // 32 hex digits + 4 dashes = 36 chars.
    if s.len() != 36 {
        return false;
    }
    for (i, b) in s.bytes().enumerate() {
        let ok = match i {
            // Canonical dash positions in 8-4-4-4-12.
            8 | 13 | 18 | 23 => b == b'-',
            // Everything else must be a lowercase hex digit (0-9 or a-f).
            _ => b.is_ascii_digit() || matches!(b, b'a'..=b'f'),
        };
        if !ok {
            return false;
        }
    }
    true
}

/// SSRF guard for an import target URL: parse the host and reject loopback,
/// private, link-local, and `*.local` hosts so the bridge can not be used to
/// probe the user internal network. Returns `true` only for a host that is a
/// public hostname or a public IP. A URL that fails to parse a host is rejected.
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

    // A well-formed Firefox per-install UUID (8-4-4-4-12 lowercase hex).
    const FIREFOX_UUID: &str = "12345678-90ab-cdef-1234-567890abcdef";

    fn dev() -> Vec<String> {
        vec!["chrome-extension://devdevdevdevdevdevdevdevdevdevde".to_string()]
    }

    #[test]
    fn allows_known_chrome_id() {
        // The published Chrome Web Store id must be an allowed origin.
        let origin = "chrome-extension://oaoekkgkhmgdfnpmfkpphgiikliaicll";
        assert!(is_allowed_origin(origin, &[]));
    }

    #[test]
    fn allows_well_formed_firefox_uuid() {
        let origin = format!("moz-extension://{FIREFOX_UUID}");
        assert!(is_allowed_origin(&origin, &[]));
    }

    #[test]
    fn allows_firefox_null_origin() {
        // Firefox sends `Origin: null` for a WebSocket initiated from an
        // extension background script — it strips the moz-extension UUID rather
        // than leak it (Bugzilla 1607936 / 1257989). This is the REAL Firefox
        // bridge handshake (NOT `moz-extension://<uuid>`). The origin gate is
        // defense-in-depth only; the per-frame 256-bit pairing token over a
        // loopback-only listener is the actual boundary.
        assert!(is_allowed_origin("null", &[]));
        // Leading/trailing whitespace is trimmed before the check.
        assert!(is_allowed_origin("  null  ", &[]));
    }

    #[test]
    fn rejects_origins_that_merely_contain_null() {
        // Only the exact `null` token is accepted — not arbitrary strings that
        // happen to contain it.
        assert!(!is_allowed_origin("nullish", &[]));
        assert!(!is_allowed_origin("https://null.example.com", &[]));
    }

    #[test]
    fn rejects_firefox_gecko_id() {
        // The AMO/gecko id never appears as a real `moz-extension://` origin —
        // Firefox uses a random per-install UUID instead — so it is not a UUID
        // and must be rejected.
        assert!(!is_allowed_origin(
            "moz-extension://job-importer@aijobhunter.app",
            &[]
        ));
    }

    #[test]
    fn rejects_firefox_uuid_with_trailing_path() {
        let origin = format!("moz-extension://{FIREFOX_UUID}/popup.html");
        assert!(!is_allowed_origin(&origin, &[]));
    }

    #[test]
    fn rejects_malformed_firefox_uuid() {
        // Too short.
        assert!(!is_allowed_origin("moz-extension://12345678-90ab", &[]));
        // Non-hex char in a hex position.
        assert!(!is_allowed_origin(
            "moz-extension://g2345678-90ab-cdef-1234-567890abcdef",
            &[]
        ));
        // Uppercase hex (origins are lowercase).
        assert!(!is_allowed_origin(
            "moz-extension://12345678-90AB-cdef-1234-567890abcdef",
            &[]
        ));
        // Dash in the wrong position (right length, bad shape).
        assert!(!is_allowed_origin(
            "moz-extension://123456789-0ab-cdef-1234-567890abcdef",
            &[]
        ));
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

    #[test]
    fn is_extension_uuid_shape() {
        assert!(is_extension_uuid(FIREFOX_UUID));
        assert!(!is_extension_uuid(""));
        assert!(!is_extension_uuid("not-a-uuid"));
        // Trailing path can never be a UUID (contains a slash, wrong length).
        assert!(!is_extension_uuid("12345678-90ab-cdef-1234-567890abcdef/x"));
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
