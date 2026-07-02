use super::*;
use crate::commands::autopilot::build_found_job;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal `JobPosting` fixture — mirrors the shape a board scraper returns.
fn posting(url: &str, company: &str) -> JobPosting {
    JobPosting {
        id: "job-1".to_string(),
        external_id: None,
        title: "Backend Engineer".to_string(),
        company: company.to_string(),
        location: None,
        url: url.to_string(),
        source: "manual".to_string(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: HashMap::new(),
    }
}

// ---------------------------------------------------------------------------
// assess_trust — clean job
// ---------------------------------------------------------------------------

#[test]
fn clean_job_scores_100_high_no_flags() {
    let a = assess_trust("https://stripe.com/jobs/1", "Stripe");
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
        let a = assess_trust(url, "Suspicious Co");
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
        let a = assess_trust(url, "Suspicious Co");
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
    let a = assess_trust("https://bit.ly/x", "");
    assert_eq!(a.score, 75);
    assert_eq!(a.level, TrustLevel::Medium);
    assert_eq!(a.flags, vec![TrustFlag::SuspiciousDomain]);
}

#[test]
fn suspicious_domain_subdomain_still_flagged() {
    let a = assess_trust("https://sub.bit.ly/x", "");
    assert_eq!(a.score, 75);
    assert_eq!(a.level, TrustLevel::Medium);
    assert_eq!(a.flags, vec![TrustFlag::SuspiciousDomain]);
}

// ---------------------------------------------------------------------------
// assess_trust — company/domain mismatch
// ---------------------------------------------------------------------------

#[test]
fn company_domain_mismatch_flagged_and_penalized() {
    let a = assess_trust("https://randomhost.xyz/j", "Acme");
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
        let a = assess_trust(url, "Weyland-Yutani");
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
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty());
}

// ---------------------------------------------------------------------------
// assess_trust — combined penalties
// ---------------------------------------------------------------------------

#[test]
fn suspicious_and_mismatch_combine() {
    let a = assess_trust("https://bit.ly/xyz", "Acme");
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

// ---------------------------------------------------------------------------
// finish — level thresholds
// ---------------------------------------------------------------------------

#[test]
fn level_threshold_boundaries() {
    assert_eq!(finish(90, vec![]).level, TrustLevel::High);
    assert_eq!(finish(89, vec![]).level, TrustLevel::Medium);
    assert_eq!(finish(60, vec![]).level, TrustLevel::Medium);
    assert_eq!(finish(59, vec![]).level, TrustLevel::Low);
}

// ---------------------------------------------------------------------------
// attach() — the JSON contract the renderer badge deserializes
// ---------------------------------------------------------------------------

/// `attach` is the production glue (called from the scrape engine + manual
/// `scrape_url`) that writes `job.extra["trust"]` — this is the only test
/// proving that channel round-trips to the exact camelCase shape the
/// renderer badge expects, not just that `assess_trust` computes correctly.
#[test]
fn attach_writes_expected_json_shape() {
    let mut job = posting("https://randomhost.xyz/j", "Acme");
    attach(&mut job);

    let value = job
        .extra
        .get("trust")
        .expect("attach must insert job.extra[\"trust\"]");

    assert_eq!(value["score"], serde_json::json!(85));
    assert_eq!(value["level"], serde_json::json!("medium"));
    assert_eq!(value["flags"], serde_json::json!(["companyDomainMismatch"]));

    // Round-trip through the real type — proves the shape isn't just
    // coincidentally matching field names, it actually deserializes.
    let assessment: TrustAssessment = serde_json::from_value(value.clone())
        .expect("job.extra[\"trust\"] must deserialize back to TrustAssessment");
    assert_eq!(assessment.score, 85);
    assert_eq!(assessment.level, TrustLevel::Medium);
    assert_eq!(assessment.flags, vec![TrustFlag::CompanyDomainMismatch]);
}

// ---------------------------------------------------------------------------
// matches_domain_list — anchoring (security regression)
// ---------------------------------------------------------------------------

#[test]
fn matches_domain_list_anchors_on_label_boundary() {
    let list = ["greenhouse.io"];
    assert!(
        !matches_domain_list("evil-greenhouse.io", &list),
        "hyphen-glued lookalike must NOT match — not a real subdomain"
    );
    assert!(
        !matches_domain_list("greenhouse.io.evil.com", &list),
        "the allowlisted domain used as a prefix of an attacker host must NOT match"
    );
    assert!(
        matches_domain_list("sub.greenhouse.io", &list),
        "a real subdomain must match"
    );
    assert!(
        matches_domain_list("greenhouse.io", &list),
        "an exact host must match"
    );
}

// ---------------------------------------------------------------------------
// company_matches_host — current documented behavior
// ---------------------------------------------------------------------------

#[test]
fn company_matches_host_documented_behavior() {
    // Intentionally-deferred V1 limitation (see the doc comment on
    // `company_matches_host`): the unanchored `host.contains(slug)` check
    // matches a brand-embedding phishing host, suppressing the mismatch flag.
    // This assertion documents the REAL current behavior, not the ideal one —
    // label-boundary anchoring was deliberately deferred to avoid false
    // positives on legit brand+suffix domains (e.g. datadoghq.com/Datadog).
    assert!(
        company_matches_host("Amazon", "amazon-careers.xyz"),
        "known deferred limitation: unanchored substring match suppresses \
         the flag for a brand-embedding phishing-style host"
    );

    // Direction (b), same doc comment: a short/generic (>=3 char) company
    // word can over-match an unrelated host that merely happens to contain
    // it. "Cloud" is generic enough to appear in a host with no real
    // relation to "Bright Cloud Systems" — the word-level fallback (not the
    // full-slug check, since "brightcloudsystems" isn't a substring of the
    // host) suppresses the flag anyway. Documents the real current
    // behavior, not the ideal one.
    assert!(
        company_matches_host("Bright Cloud Systems", "cloudhosting.io"),
        "known deferred limitation: a generic company word (\"cloud\") \
         over-matches an unrelated host that merely contains the word"
    );

    // An unjudgeable (empty-after-normalize) company never raises a flag.
    assert!(company_matches_host("", "anything.example.com"));
    assert!(company_matches_host("   ", "anything.example.com"));

    // Exact brand host is the intended-to-work case.
    assert!(company_matches_host("Stripe", "stripe.com"));
}

#[test]
fn company_matches_host_skips_stop_words() {
    // "The Inc Corp" is dominated by generic legal-entity words — none of
    // them should false-match an unrelated host that merely happens to
    // contain "the"/"corp" as a substring.
    assert!(
        !company_matches_host("The Inc Corp", "the-daily-corp-news.example.com"),
        "generic legal-entity words must not false-match an unrelated host"
    );

    // A real brand word alongside a stop word still matches — the filter
    // only removes the generic words, not the whole per-word check.
    assert!(
        company_matches_host("Acme Corp", "acme.com"),
        "a real brand word must still match even when paired with a stop word"
    );
}

// ---------------------------------------------------------------------------
// FoundJob wiring — exercises the REAL `build_found_job` projection (the
// same one `autopilot_run`'s `postings.iter().map(..)` calls), not a
// hand-retyped mirror that could silently drift (dropped field, swapped args).
// ---------------------------------------------------------------------------

#[test]
fn found_job_carries_trust_from_real_build_found_job() {
    // Asymmetric on purpose: "Acme" shares no substring with "randomhost.xyz",
    // so a swapped-arg regression (`assess_trust(company, url)` instead of
    // `assess_trust(url, company)`) would try to parse "Acme" as a url —
    // InvalidUrl/50/Low — instead of the real 85/Medium/[CompanyDomainMismatch]
    // result below, and this test would fail.
    let p = posting("https://randomhost.xyz/j", "Acme");

    let found = build_found_job(&p, "", 0);
    let expected = assess_trust(&p.url, &p.company);

    let trust = found
        .trust
        .expect("build_found_job must always set Some(..)");
    assert_eq!(trust.score, expected.score);
    assert_eq!(trust.level, expected.level);
    assert_eq!(trust.flags, expected.flags);

    // Pin the concrete values too, not just equality against a second call
    // to the same function.
    assert_eq!(trust.score, 85);
    assert_eq!(trust.level, TrustLevel::Medium);
    assert_eq!(trust.flags, vec![TrustFlag::CompanyDomainMismatch]);
}
