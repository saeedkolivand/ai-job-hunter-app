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
    // defense-in-depth only; the v2 mutual HMAC handshake over a
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
fn allows_native_host_sentinel_origin() {
    // Our native-messaging host relays to the loopback bridge with this
    // sentinel Origin (it has no extension origin of its own). Accepted
    // (defense-in-depth); the v2 mutual HMAC handshake is the real boundary.
    assert!(is_allowed_origin(NATIVE_HOST_ORIGIN, &[]));
    assert!(is_allowed_origin("ajh-native-host", &[]));
    // Leading/trailing whitespace is trimmed before the check.
    assert!(is_allowed_origin("  ajh-native-host  ", &[]));
}

#[test]
fn rejects_origins_that_merely_contain_native_sentinel() {
    // Only the exact sentinel is accepted — not strings that contain it.
    assert!(!is_allowed_origin("ajh-native-host.evil.com", &[]));
    assert!(!is_allowed_origin("xajh-native-host", &[]));
}

#[test]
fn allows_agent_cli_sentinel_origin() {
    assert!(is_allowed_origin(AGENT_CLI_ORIGIN, &[]));
    assert!(is_allowed_origin("ajh-agent-cli", &[]));
    assert!(is_allowed_origin("  ajh-agent-cli  ", &[]));
}

#[test]
fn rejects_origins_that_merely_contain_agent_cli_sentinel() {
    assert!(!is_allowed_origin("ajh-agent-cli.evil.com", &[]));
    assert!(!is_allowed_origin("xajh-agent-cli", &[]));
}

#[test]
fn agent_cli_and_native_host_sentinels_are_distinct() {
    // The whole point of a SEPARATE sentinel (finding #5, security
    // review) is that the two are distinguishable — one must never
    // satisfy the other's exact-match check.
    assert_ne!(AGENT_CLI_ORIGIN, NATIVE_HOST_ORIGIN);
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

// ── is_extension_origin (PR1 — extension read tier caller-class label) ───

#[test]
fn is_extension_origin_accepts_chrome_and_firefox_shapes() {
    assert!(is_extension_origin(
        "chrome-extension://oaoekkgkhmgdfnpmfkpphgiikliaicll",
        &[]
    ));
    let firefox = format!("moz-extension://{FIREFOX_UUID}");
    assert!(is_extension_origin(&firefox, &[]));
    // The REAL Firefox background-script origin — see the doc.
    assert!(is_extension_origin("null", &[]));
}

#[test]
fn is_extension_origin_accepts_dev_override() {
    let origins = dev();
    assert!(is_extension_origin(&origins[0], &origins));
}

#[test]
fn is_extension_origin_accepts_the_native_host_relay_but_rejects_the_cli() {
    // The native-messaging relay forwards the paired extension's frames 1:1, so it
    // counts as the extension; only the CLI sentinel is carved out.
    assert!(is_extension_origin(NATIVE_HOST_ORIGIN, &[]));
    assert!(!is_extension_origin(AGENT_CLI_ORIGIN, &[]));
}

#[test]
fn is_extension_origin_rejects_unknown_origins() {
    assert!(!is_extension_origin("https://evil.example.com", &[]));
    assert!(!is_extension_origin("", &[]));
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
