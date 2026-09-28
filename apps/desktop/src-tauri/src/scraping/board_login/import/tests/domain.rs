//! Pure host/domain-matching and Chromium-epoch conversion tests.

use super::super::decrypt::{chromium_time_to_unix, domain_matches};

#[test]
fn domain_matches_linkedin() {
    assert!(domain_matches("linkedin", ".linkedin.com"));
    assert!(domain_matches("linkedin", "linkedin.com"));
    assert!(domain_matches("linkedin", "www.linkedin.com"));
    assert!(!domain_matches("linkedin", "indeed.com"));
    // `.`-anchored suffix must reject lookalike hosts that merely start with
    // the board domain.
    assert!(!domain_matches("linkedin", "linkedin.com.evil.test"));
    assert!(!domain_matches("linkedin", "notlinkedin.com"));
}

#[test]
fn domain_matches_indeed_locale_tlds() {
    for host in [
        "indeed.com",
        ".de.indeed.com",
        "de.indeed.com",
        "uk.indeed.com",
        "indeed.fr",
        "ca.indeed.com",
    ] {
        assert!(domain_matches("indeed", host), "should match {host}");
    }
    assert!(!domain_matches("indeed", "linkedin.com"));
    // Must not match an unrelated host that merely contains the letters.
    assert!(!domain_matches("indeed", "notindeedreally.example"));
    assert!(domain_matches("indeed", "notindeed.indeed.com"));
}

#[test]
fn domain_matches_xing_glassdoor() {
    assert!(domain_matches("xing", "www.xing.com"));
    assert!(domain_matches("xing", "xing.com"));
    assert!(domain_matches("glassdoor", ".glassdoor.com"));
    assert!(domain_matches("glassdoor", "glassdoor.com"));
    assert!(!domain_matches("xing", "glassdoor.com"));
    // `.`-anchored suffix rejects lookalike hosts for these boards too.
    assert!(!domain_matches("xing", "xing.com.evil.test"));
    assert!(!domain_matches("glassdoor", "glassdoor.com.evil.test"));
}

#[test]
fn domain_matches_unknown_board_is_false() {
    assert!(!domain_matches("monster", "monster.com"));
}

#[test]
fn chromium_time_session_cookie_is_none() {
    assert_eq!(chromium_time_to_unix(0), None);
    assert_eq!(chromium_time_to_unix(-1), None);
}

#[test]
fn chromium_time_converts_known_epoch() {
    // 1601 epoch micros for exactly unix epoch (1970-01-01) = EPOCH_DELTA_US.
    const EPOCH_DELTA_US: i64 = 11_644_473_600_000_000;
    assert_eq!(chromium_time_to_unix(EPOCH_DELTA_US), None); // delta -> 0 -> None
                                                             // One second after unix epoch.
    let v = chromium_time_to_unix(EPOCH_DELTA_US + 1_000_000).unwrap();
    assert!((v - 1.0).abs() < 1e-6, "got {v}");
}

// ── Gap 3: domain_matches anchoring edge cases ────────────────────────────────

/// indeed locale TLDs: direct second-level domains like `indeed.de` and
/// `indeed.co.uk` must match (the `contains("indeed.")` rule fires on the
/// `.` after `indeed`).
#[test]
fn domain_matches_indeed_direct_locale_tlds() {
    for host in ["indeed.de", "indeed.co.uk", "indeed.in", "indeed.com.au"] {
        assert!(
            domain_matches("indeed", host),
            "must match indeed locale TLD: {host}"
        );
    }
}

/// The indeed rule uses `host.contains("indeed.")` (the dot is the key guard).
/// A host that does NOT contain the literal string `"indeed."` must not match.
/// NOTE: hosts like `someindeed.com` *do* contain `"indeed."` (at char offset 4)
/// and therefore DO match — this is a known, documented heuristic limitation;
/// we assert the actual behaviour here rather than an idealised one.
#[test]
fn domain_matches_indeed_rejects_substring_without_label() {
    // "indeedish.io" → "indeedish.io".contains("indeed.") == false → must not match.
    assert!(!domain_matches("indeed", "indeedish.io"));
    // "noindeedhere.net" → does not contain "indeed." → must not match.
    assert!(!domain_matches("indeed", "noindeedhere.net"));
    // Document the known heuristic: "someindeed.com" contains "indeed." so it
    // DOES match (false-positive, accepted trade-off for locale-TLD coverage).
    assert!(
        domain_matches("indeed", "someindeed.com"),
        "known heuristic: someindeed.com contains 'indeed.' and matches — document, not fix"
    );
}

/// The `.`-anchored suffix check for linkedin must accept an uppercase input
/// (the code normalises via `to_ascii_lowercase`).
#[test]
fn domain_matches_linkedin_case_insensitive() {
    assert!(domain_matches("linkedin", "WWW.LINKEDIN.COM"));
    assert!(domain_matches("linkedin", "LinkedIn.Com"));
    // Lookalike with uppercase must still be rejected.
    assert!(!domain_matches("linkedin", "LINKEDIN.COM.EVIL.TEST"));
}
