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

/// Sentinel `Origin` the native-messaging host
/// ([`super::native_host`]) sends when it relays the browser's frames to this
/// loopback bridge. The host is our OWN native process (spawned by the browser,
/// our exe in `--native-host` mode) bridging stdio → `ws://127.0.0.1`, so it has
/// no `chrome-extension://`/`moz-extension://` origin of its own. Accepting this
/// sentinel is defense-in-depth only: the real boundary is the v2 mutual HMAC
/// handshake ([`super::handshake::verify_client_proof`]) over the loopback-only
/// listener, which the host relays through unchanged.
pub const NATIVE_HOST_ORIGIN: &str = "ajh-native-host";

/// Sentinel `Origin` [`super::agent_cli::attempt_port`] sends (MEDIUM fix —
/// security review). The CLI used to reuse [`NATIVE_HOST_ORIGIN`], so the
/// server could not distinguish "the CLI" from "the browser extension
/// arriving via the native-host relay" — `msg::AGENT_QUERY`'s doc claimed
/// `agent.query` is CLI-agent-only, but that was true of the extension's OWN
/// code, not of anything the server itself enforced.
///
/// **Be honest about what this is**: a plain string a local process types
/// into a WebSocket handshake header, trivially spoofable by anything that
/// can open a loopback socket — NOT a security boundary. The v2 mutual HMAC
/// handshake ([`super::handshake::verify_client_proof`]) remains the ONLY
/// boundary; this origin gate stays defense-in-depth exactly like every
/// other entry in [`is_allowed_origin`]. What it DOES do: make the
/// documented "CLI-agent only" restriction on `agent.query` actually
/// enforced against every NON-COLLUDING case — a future bug in the
/// extension's own code, or a compromised extension update that starts
/// sending frames it was never meant to — none of which know to spoof this
/// origin. It does nothing against a deliberate local attacker, who can
/// simply set this header (the same limitation every entry in
/// [`is_allowed_origin`] already carries).
pub const AGENT_CLI_ORIGIN: &str = "ajh-agent-cli";

/// Whether a handshake `Origin` is an allowed extension origin.
///
/// This check is **defense-in-depth, not the primary boundary**. The real
/// authentication is the v2 mutual HMAC challenge-response
/// ([`super::advance_frame`] drives it; [`super::handshake::verify_client_proof`]
/// does the **constant-time** proof check) over a loopback-only (`127.0.0.1`)
/// listener — the pairing token is used only as an HMAC key and is NEVER sent on
/// the wire. The token is copied by the user from the app Settings and a sibling
/// extension cannot read it, so even a local extension that opens a socket
/// cannot import anything without proving it knows the token.
///
/// Acceptance:
/// - **Dev override** (checked first): any exact-match `dev_origins` entry (a
///   developer locally-loaded extension, supplied via the dev env override).
/// - **Chrome**: `chrome-extension://<id>` where `<id>` is in
///   [`ALLOWED_EXTENSION_IDS`] (the stable Chrome Web Store id).
/// - **Firefox**: `moz-extension://<uuid>` where `<uuid>` is a well-formed
///   extension UUID ([`is_extension_uuid`]). The Firefox per-install UUID is
///   unknowable in advance, so the origin check can only assert scheme + UUID
///   shape; the mutual handshake is what actually authenticates.
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
    // v2 mutual HMAC handshake over a loopback-only (`127.0.0.1`) listener, which
    // a null-origin page cannot satisfy without proving it knows the token the
    // user copied from the app's Settings.
    if origin == "null" {
        return true;
    }
    // Native-messaging host (our own native process relaying to the loopback
    // bridge — see `NATIVE_HOST_ORIGIN`). Exact match only; the mutual handshake
    // + loopback binding remain the real boundary.
    if origin == NATIVE_HOST_ORIGIN {
        return true;
    }
    // The `ajh-tauri agent` CLI — see `AGENT_CLI_ORIGIN`'s doc for why this
    // is a label, not a boundary.
    if origin == AGENT_CLI_ORIGIN {
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

/// Whether a handshake `Origin` is the paired browser EXTENSION itself — the label
/// `handle_connection` resolves once (PR1, extension read tier) and threads alongside
/// [`AGENT_CLI_ORIGIN`]'s own label into [`super::CallerClass`]. Same defense-in-depth caveat as
/// every other check in this file: a label, not a boundary — the v2 mutual HMAC handshake remains
/// the only real one.
///
/// Reuses [`is_allowed_origin`] (never a second origin-matching implementation) and excludes ONLY
/// the one sentinel that is never the extension: [`AGENT_CLI_ORIGIN`]. [`NATIVE_HOST_ORIGIN`] IS
/// the extension here — the native-messaging host ([`super::native_host`], the Firefox
/// HTTPS-Only-Mode fallback) has no origin of its own and relays the paired extension's frames 1:1,
/// and the native-messaging manifests pin which extension may launch it, so a relayed frame
/// genuinely came from that extension; `Origin` is only a label either way, and the mutual HMAC
/// handshake stays the real boundary. Everything else [`is_allowed_origin`] accepts genuinely IS
/// the extension too: the dev override, a known Chrome id, a well-formed Firefox UUID, and `null` —
/// the real origin a Firefox extension's BACKGROUND script sends (see [`is_allowed_origin`]'s own
/// doc), which is the shape production connections actually arrive as, since the bridge client
/// runs in the extension's background/service-worker context on both browsers.
pub fn is_extension_origin(origin: &str, dev_origins: &[String]) -> bool {
    let origin = origin.trim();
    if origin == AGENT_CLI_ORIGIN {
        return false;
    }
    is_allowed_origin(origin, dev_origins)
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
mod tests;
