//! `finish` score-clamping + level-threshold tests, `attach`'s JSON shape,
//! and `matches_domain_list`/`company_matches_host` anchoring.

use super::super::*;
use super::support::posting;

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
