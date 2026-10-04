use super::*;

// ── detect_platform: host match, never a substring scan of the whole URL ──

#[test]
fn rejects_hostile_lookalike_and_malformed_urls() {
    for (url, why) in [
        // Pre-fix, `lower.contains("linkedin.com/in/")` matched this because
        // the string appears in the PATH, not the host.
        (
            "https://attacker.example/linkedin.com/in/x",
            "linkedin.com in the path of a hostile host",
        ),
        (
            "http://127.0.0.1:9200/linkedin.com/in/x",
            "loopback host with linkedin in the path",
        ),
        // `evillinkedin.com` — a bare `ends_with("linkedin.com")` (no `.`
        // boundary) would wrongly accept this.
        ("https://evillinkedin.com/in/x", "lookalike suffix host"),
        (
            "https://www.linkedin.com/jobs/view/123",
            "non-profile path on the real host",
        ),
        ("not a url", "unparseable input"),
        // ── scheme is checked, not just the host ──
        // Defence in depth: get_guarded* already rejects non-HTTP(S) schemes
        // before connecting, but detect_platform must not classify this as
        // LinkedIn either.
        (
            "file://linkedin.com/in/x",
            "file scheme even with a real linkedin host",
        ),
        ("foo://linkedin.com/in/x", "an arbitrary non-http scheme"),
        // ".linkedin.com".ends_with(".linkedin.com") is true, so the old
        // `ends_with` check alone would wrongly accept this: the host is
        // effectively empty + a dot, not a real subdomain.
        (
            "https://.linkedin.com/in/x",
            "an empty leading label before the real domain",
        ),
    ] {
        assert!(detect_platform(url).is_none(), "{why}: {url}");
    }
}

#[test]
fn accepts_the_real_host_and_its_genuine_subdomains() {
    for (url, why) in [
        ("https://linkedin.com/in/foo", "bare apex host"),
        ("https://www.linkedin.com/in/foo", "www host"),
        // A real `*.linkedin.com` subdomain (e.g. a locale mirror) must still
        // match via the suffix branch.
        ("https://de.linkedin.com/in/foo", "a genuine subdomain"),
        // Sibling of the www case, pinned against the exact URL shape the
        // scheme + empty-label checks above must NOT regress.
        ("https://www.linkedin.com/in/x", "https www linkedin"),
    ] {
        assert!(
            matches!(detect_platform(url), Some(Platform::LinkedIn)),
            "{why}: {url}"
        );
    }
}
