use super::*;

#[test]
fn sanitize_log_origin_strips_control_chars_but_keeps_the_rest() {
    // Newline/CR/ESC are exactly the shapes that forge a second log line
    // (CR/LF) or an ANSI escape-sequence trick in a log file support
    // bundles ship verbatim — this is what a hostile `Origin` header can
    // never smuggle through.
    let hostile = "chrome-extension://evil\nWARN fake line\r\x1b[31mred\x1b[0m\0end";
    let cleaned = sanitize_log_origin(hostile);
    assert!(
        cleaned.chars().all(|c| !c.is_control()),
        "control character survived: {cleaned:?}"
    );
    assert!(!cleaned.contains('\n'));
    assert!(!cleaned.contains('\r'));
    assert!(!cleaned.contains('\x1b'));
    assert!(!cleaned.contains('\0'));
    // The diagnostic value must survive: the developer still needs to
    // read the real origin back to add it to `extensionDevOrigins`.
    assert!(cleaned.contains("chrome-extension://evil"));
    assert!(cleaned.contains("WARN fake line"));
    assert!(cleaned.contains("red"));
    assert!(cleaned.contains("end"));
}

#[test]
fn sanitize_log_origin_caps_length() {
    // The `Origin` header is attacker-supplied and unbounded — this is
    // the live protection (see the doc comment: a raw newline can't reach
    // this call site through the current HTTP-header path, but nothing
    // bounds the header's length).
    let huge = "a".repeat(MAX_LOGGED_ORIGIN_LEN + 500);
    let cleaned = sanitize_log_origin(&huge);
    assert_eq!(cleaned.chars().count(), MAX_LOGGED_ORIGIN_LEN + 1); // + truncation marker
    assert!(cleaned.ends_with('…'));
}

#[test]
fn sanitize_log_origin_leaves_a_short_clean_origin_untouched() {
    // A real Origin never gets mangled — length-cap and control-char
    // stripping are both no-ops on the legitimate shapes this bridge
    // actually sees.
    let origin = "chrome-extension://oaoekkgkhmgdfnpmfkpphgiikliaicll";
    assert_eq!(sanitize_log_origin(origin), origin);
}
