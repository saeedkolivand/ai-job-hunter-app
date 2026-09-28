//! `assess_trust` domain-signal tests: clean/missing/invalid URL, suspicious
//! domains, company/domain mismatch, the ATS + Adzuna-market allowlists, and
//! combined penalties.

use super::super::*;

// ---------------------------------------------------------------------------
// assess_trust — clean job
// ---------------------------------------------------------------------------

#[test]
fn clean_job_scores_100_high_no_flags() {
    let a = assess_trust(
        "https://stripe.com/jobs/1",
        "Stripe",
        "We build payments infra.",
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty(), "expected no flags, got {:?}", a.flags);
}

// ---------------------------------------------------------------------------
// assess_trust — missing url (early return)
// ---------------------------------------------------------------------------

#[test]
fn missing_url_flags_and_early_returns() {
    for url in ["", "   ", "\t\n"] {
        // Company deliberately set to something that would ALSO mismatch, to
        // prove the empty-url branch early-returns before the mismatch check
        // ever runs.
        let a = assess_trust(url, "Suspicious Co", "Some job description.");
        assert_eq!(a.score, 60, "url={url:?}");
        assert_eq!(a.level, TrustLevel::Medium, "url={url:?}");
        assert_eq!(
            a.flags,
            vec![TrustFlag::MissingApplyUrl],
            "url={url:?} — early return must produce exactly one flag"
        );
    }
}

// ---------------------------------------------------------------------------
// assess_trust — invalid url (early return)
// ---------------------------------------------------------------------------

#[test]
fn invalid_url_flags_and_early_returns() {
    for url in [
        "javascript:alert(1)",             // parseable, non-http(s) scheme
        "data:text/plain;base64,aGVsbG8=", // parseable, non-http(s) scheme
        "ftp://files.example.com/x",       // parseable, non-http(s) scheme
        "not-a-url-at-all",                // non-parseable (no scheme)
    ] {
        let a = assess_trust(url, "Suspicious Co", "Some job description.");
        assert_eq!(a.score, 50, "url={url:?}");
        assert_eq!(a.level, TrustLevel::Low, "url={url:?}");
        assert_eq!(
            a.flags,
            vec![TrustFlag::InvalidUrl],
            "url={url:?} — early return must produce exactly one flag"
        );
    }
}

// ---------------------------------------------------------------------------
// assess_trust — suspicious domain
// ---------------------------------------------------------------------------

#[test]
fn suspicious_domain_flagged_and_penalized() {
    // Company left empty so only the suspicious-domain check is exercised.
    let a = assess_trust("https://bit.ly/x", "", "A real description.");
    assert_eq!(a.score, 75);
    assert_eq!(a.level, TrustLevel::Medium);
    assert_eq!(a.flags, vec![TrustFlag::SuspiciousDomain]);
}

#[test]
fn suspicious_domain_subdomain_still_flagged() {
    let a = assess_trust("https://sub.bit.ly/x", "", "A real description.");
    assert_eq!(a.score, 75);
    assert_eq!(a.level, TrustLevel::Medium);
    assert_eq!(a.flags, vec![TrustFlag::SuspiciousDomain]);
}

// ---------------------------------------------------------------------------
// assess_trust — company/domain mismatch
// ---------------------------------------------------------------------------

#[test]
fn company_domain_mismatch_flagged_and_penalized() {
    let a = assess_trust("https://randomhost.xyz/j", "Acme", "A real description.");
    assert_eq!(a.score, 85);
    assert_eq!(a.level, TrustLevel::Medium);
    assert_eq!(a.flags, vec![TrustFlag::CompanyDomainMismatch]);
}

// ---------------------------------------------------------------------------
// assess_trust — allowlist suppresses mismatch
// ---------------------------------------------------------------------------

#[test]
fn allowlisted_ats_host_suppresses_mismatch() {
    // Company is deliberately unrelated to the host so this would flag
    // `CompanyDomainMismatch` if the host weren't allowlisted.
    for url in [
        "https://boards.greenhouse.io/other-corp/jobs/55",
        "https://greenhouse.io/jobs/99",
    ] {
        let a = assess_trust(url, "Weyland-Yutani", "A real description.");
        assert_eq!(a.score, 100, "url={url}");
        assert_eq!(a.level, TrustLevel::High, "url={url}");
        assert!(a.flags.is_empty(), "url={url} flags={:?}", a.flags);
    }
}

/// The Adzuna aggregator host — the country code is a path segment (e.g.
/// `/v1/api/jobs/de/redirects/…`), not a subdomain, so `api.adzuna.com` alone
/// must cover every market's `redirect_url` without flagging every posting.
#[test]
fn adzuna_aggregator_host_suppresses_mismatch() {
    let a = assess_trust(
        "https://api.adzuna.com/v1/api/jobs/de/redirects/123",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty());
}

/// Companion to `adzuna_aggregator_host_suppresses_mismatch` above: the
/// aggregator's OTHER host shape — its own per-market website
/// (`www.adzuna.<tld>`), not the `api.adzuna.com` API host — must also
/// suppress `CompanyDomainMismatch`. Regression for issue #1107: the
/// allowlist covered only `api.adzuna.com`, so every posting whose
/// `redirect_url` came back in this shape (confirmed in this repo's own
/// aggregator fixture, `scraping/boards/aggregator/test.rs`, `"redirect_url":
/// "https://www.adzuna.de/details/…"`) was false-flagged regardless of
/// employer legitimacy.
///
/// Mutation-checked: reverting the `is_adzuna_market_host` allowlist rule
/// turns this red (`CompanyDomainMismatch` fires) before the fix.
#[test]
fn adzuna_market_website_host_suppresses_mismatch() {
    let a = assess_trust(
        "https://www.adzuna.de/details/4172839571",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty());
}

/// A lookalike host must NOT be treated as Adzuna's own site — `adzuna` has
/// to be immediately followed by a bare TLD, not by more labels belonging to
/// an unrelated (attacker-controlled) domain.
#[test]
fn adzuna_market_host_match_does_not_match_lookalike_domain() {
    let a = assess_trust(
        "https://www.adzuna.evil.com/j",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.flags, vec![TrustFlag::CompanyDomainMismatch]);
}

/// Regression for the follow-up to issue #1107: `is_adzuna_market_host` only
/// matched a single-label TLD after `adzuna` (`www.adzuna.de`), so compound
/// ccTLD markets — including `gb`, Adzuna's HOME market — still false-flagged
/// every posting. Mutation-checked: reverting the compound-TLD branch turns
/// this red.
#[test]
fn adzuna_market_website_compound_tld_uk_suppresses_mismatch() {
    let a = assess_trust(
        "https://www.adzuna.co.uk/details/4172839571",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty());
}

/// Same compound-TLD branch, a second market (`au`) — guards against a fix
/// that only special-cased `co.uk` literally instead of matching the general
/// two-label-suffix shape.
#[test]
fn adzuna_market_website_compound_tld_au_suppresses_mismatch() {
    let a = assess_trust(
        "https://www.adzuna.com.au/details/4172839571",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty());
}

/// Third compound-TLD market (`za`) — South Africa's `co.za` shares the
/// `co.<tld>` shape with `co.uk`/`co.in`/`co.nz`, so this guards that all
/// four aren't collapsed into one accidentally-loose "co.*" rule.
#[test]
fn adzuna_market_website_compound_tld_za_suppresses_mismatch() {
    let a = assess_trust(
        "https://www.adzuna.co.za/details/4172839571",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty());
}

/// A compound-shaped lookalike must NOT match — `adzuna` followed by
/// `evil.co.uk` (three trailing labels once counted from `adzuna`) is not a
/// known Adzuna compound suffix.
#[test]
fn adzuna_compound_lookalike_subdomain_does_not_match() {
    let a = assess_trust(
        "https://www.adzuna.evil.co.uk/j",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.flags, vec![TrustFlag::CompanyDomainMismatch]);
}

/// A compound-shaped lookalike where the real attacker domain follows a
/// legitimate-looking `adzuna.co.uk` prefix — must NOT match either.
#[test]
fn adzuna_compound_lookalike_suffix_does_not_match() {
    let a = assess_trust(
        "https://adzuna.co.uk.evil.com/j",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.flags, vec![TrustFlag::CompanyDomainMismatch]);
}

/// Two more curated single-label markets beyond the pre-existing `.de`
/// coverage — guards against a fix that only special-cased the one TLD
/// already under test rather than actually curating the list.
#[test]
fn adzuna_market_website_single_label_fr_suppresses_mismatch() {
    let a = assess_trust(
        "https://www.adzuna.fr/details/4172839571",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty());
}

/// The `us` market's real domain is `adzuna.com`, not `adzuna.us` — the one
/// exception to the ccTLD-matches-country-code pattern the other 11
/// single-label markets follow. Regression for a curated list that dropped
/// (or mis-mapped) the US entry.
#[test]
fn adzuna_market_website_us_special_case_com_suppresses_mismatch() {
    let a = assess_trust(
        "https://www.adzuna.com/details/4172839571",
        "Weyland-Yutani",
        "A real description.",
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty());
}

/// Security regression: before curating `ADZUNA_SINGLE_LABEL_TLDS`,
/// `is_adzuna_market_host`'s single-label branch accepted `adzuna.<any
/// TLD>`, so a squatted lookalike domain (an attacker registering
/// `adzuna.xyz`, `adzuna.top`, etc. — cheap SLD-squatting under an unrelated
/// TLD) suppressed `CompanyDomainMismatch` for a spoofed posting exactly
/// like a real Adzuna market host would. This is the attack case the curated
/// list closes. Mutation-checked: reverting to the old "any single-label
/// TLD" branch turns this red (the flag stops firing).
#[test]
fn adzuna_lookalike_unrecognized_tld_triggers_mismatch() {
    for url in ["https://www.adzuna.xyz/j", "https://adzuna.top/j"] {
        let a = assess_trust(url, "Weyland-Yutani", "A real description.");
        assert_eq!(
            a.flags,
            vec![TrustFlag::CompanyDomainMismatch],
            "url={url} — unrecognized-TLD lookalike must not suppress the mismatch flag"
        );
    }
}

// ---------------------------------------------------------------------------
// assess_trust — combined penalties
// ---------------------------------------------------------------------------

#[test]
fn suspicious_and_mismatch_combine() {
    let a = assess_trust("https://bit.ly/xyz", "Acme", "A real description.");
    assert_eq!(a.score, 60);
    assert_eq!(a.level, TrustLevel::Medium);
    assert_eq!(
        a.flags,
        vec![
            TrustFlag::SuspiciousDomain,
            TrustFlag::CompanyDomainMismatch
        ]
    );
}

/// `assess_trust`'s two combinable penalties (-25, -15) can't drive a real
/// call below 0, so this exercises the private `finish` clamp directly to
/// prove the floor holds regardless of how many flags/penalties pile up.
#[test]
fn finish_clamps_score_to_zero_never_negative() {
    let a = finish(
        -1000,
        vec![
            TrustFlag::SuspiciousDomain,
            TrustFlag::CompanyDomainMismatch,
        ],
    );
    assert_eq!(a.score, 0);
    assert_eq!(a.level, TrustLevel::Low);
}
