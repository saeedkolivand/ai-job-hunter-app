//! `is_implausible_company` table-driven cases, the description-unavailable
//! flag, and the `build_found_job` trust/salary integration.

use super::super::*;
use super::support::{posting, posting_without_description};
use crate::commands::autopilot::build_found_job;

// is_implausible_company — table-driven (A1 hardening plan)
// ---------------------------------------------------------------------------

/// Real-world garbage this predicate exists to catch, one row per category
/// from the hardening plan. `"Apply now | LinkedIn"` is the literal PR #960
/// report, kept first.
#[test]
fn is_implausible_company_rejects_real_world_garbage() {
    let long_paragraph = "Acme Corp Global Holdings ".repeat(6);
    let cases: Vec<(&str, &str)> = vec![
        ("Apply now | LinkedIn", "the literal PR #960 report"),
        ("Apply Now", "bare CTA debris"),
        ("View Job", "bare CTA debris"),
        ("See more", "bare CTA debris"),
        ("Easy Apply", "bare CTA debris"),
        ("LinkedIn", "job-board brand standing in for the employer"),
        ("Indeed", "job-board brand standing in for the employer"),
        ("Glassdoor", "job-board brand standing in for the employer"),
        ("Xing", "job-board brand standing in for the employer"),
        ("StepStone", "job-board brand standing in for the employer"),
        ("Monster", "job-board brand standing in for the employer"),
        (
            "ZipRecruiter",
            "job-board brand standing in for the employer",
        ),
        ("Acme <script>alert(1)</script>", "HTML/markup debris"),
        ("Acme &amp; Co", "HTML-entity debris"),
        ("***", "mostly/all punctuation"),
        ("---", "mostly/all punctuation"),
        ("", "empty"),
        ("   ", "whitespace-only"),
        ("n/a", "placeholder"),
        ("N/A", "placeholder, case-insensitive"),
        ("None", "placeholder"),
        ("Unknown", "placeholder"),
        ("Company", "placeholder"),
        ("Unternehmen", "placeholder"),
        (
            "X",
            "single-character shape — see the doc comment's X decision",
        ),
        ("A", "single-character shape"),
        (long_paragraph.as_str(), "absurdly long"),
    ];
    for (input, reason) in cases {
        assert!(
            is_implausible_company(input),
            "expected {input:?} to be rejected ({reason})"
        );
    }
}

/// Real employer names — punctuation-bearing ones especially — that must
/// survive every rule above.
#[test]
fn is_implausible_company_accepts_legitimate_names_with_punctuation() {
    let legit = [
        "Johnson & Johnson",
        "Ben & Jerry's",
        "Yahoo!",
        "Booking.com",
        "37signals",
        "Acme Corp",
        "Stripe",
        "CHECK24 Vergleichsportal für Versicherungen GmbH",
        "Boxing Studio",
        "Müller & Söhne GmbH",
    ];
    for name in legit {
        assert!(
            !is_implausible_company(name),
            "expected {name:?} to be accepted as a plausible employer name"
        );
    }
}

/// Regression for a HIGH finding on commit ff87e93f: the pre-fix
/// `JOB_BOARD_NAMES` (word-boundary) and `CTA_PHRASES` (substring) checks
/// silently deleted real employers from letters, because each one either
/// shares a board's name as one word among several or opens with the same
/// prefix as a CTA phrase. Whole-string matching (see both consts' doc
/// comments) fixes every one of these without reopening the PR #960 hole —
/// see [`apply_now_pipe_linkedin_is_caught_only_by_the_separator_rule`]
/// below for proof that report is still caught.
#[test]
fn is_implausible_company_accepts_real_employers_sharing_a_board_word_or_cta_prefix() {
    let real_employers = [
        "Monster Worldwide",
        "Xing SE",
        "Indeed Inc",
        "Glassdoor Inc",
        "Apply On Demand Inc",
    ];
    for name in real_employers {
        assert!(
            !is_implausible_company(name),
            "expected {name:?} to be accepted as a plausible employer name"
        );
    }
}

/// The literal PR #960 report, isolated from the big garbage table above so
/// the coverage is provable rather than accidental: after the whole-string
/// fix, this input is no longer caught by `JOB_BOARD_NAMES`
/// (`"apply now | linkedin"` != `"linkedin"`) or `CTA_PHRASES`
/// (`"apply now | linkedin"` != `"apply now"`) — the separator rule
/// (`trimmed.contains(['|', …])`) is the only remaining rule that fires.
/// Mutation-tested by temporarily deleting that check (see the PR handoff
/// for the observed red/green result).
#[test]
fn apply_now_pipe_linkedin_is_caught_only_by_the_separator_rule() {
    assert!(is_implausible_company("Apply now | LinkedIn"));
}

/// Accepted trade-off, not a bug: making `CTA_PHRASES` an exact whole-string
/// match (to stop false-positiving "Apply On Demand Inc") also means a CTA
/// phrase concatenated with a board name — but without a separator
/// character — no longer matches either list, so this shape now reads as
/// plausible. Pinned here so a future reader sees the gap was a deliberate
/// choice, not a rediscovered regression.
#[test]
fn cta_plus_board_name_without_a_separator_is_an_accepted_false_negative() {
    assert!(!is_implausible_company("Apply on Indeed"));
}

// ---------------------------------------------------------------------------
// assess_trust — implausible company (A1 hardening plan)
// ---------------------------------------------------------------------------

#[test]
fn implausible_company_flagged_and_penalized_instead_of_mismatch() {
    // The host is neither allowlisted nor a match for the company, so a
    // pre-A1 build would have flagged `CompanyDomainMismatch` (-15) here —
    // this asserts the implausible-company branch takes priority instead
    // (-20, exactly one flag), never both.
    let a = assess_trust(
        "https://boards.example/jobs/1",
        "Apply now | LinkedIn",
        "A real description.",
    );
    assert_eq!(a.score, 80);
    assert_eq!(a.level, TrustLevel::Medium);
    assert_eq!(a.flags, vec![TrustFlag::ImplausibleCompany]);
}

// ---------------------------------------------------------------------------
// assess_trust — description unavailable (issue #1105, trust half)
// ---------------------------------------------------------------------------

/// LinkedIn's free/guest board sets `description: Some(String::new())` for
/// every posting — a title-only stub must never come back `TrustLevel::High`.
/// Mutation-checked: deleting the `description.trim().is_empty()` firing
/// condition turns this red (flag missing, level stays `High`).
#[test]
fn empty_description_flags_and_caps_below_high() {
    let a = assess_trust("https://stripe.com/jobs/1", "Stripe", "");
    assert_eq!(a.level, TrustLevel::Medium);
    assert!(a.level != TrustLevel::High, "must be capped below High");
    assert_eq!(a.flags, vec![TrustFlag::DescriptionUnavailable]);
}

/// Same as above with a whitespace-only description — trimmed before the
/// emptiness check, so this must fire identically to a truly-empty string.
#[test]
fn whitespace_only_description_flags_and_caps_below_high() {
    let a = assess_trust("https://stripe.com/jobs/1", "Stripe", "   \n\t");
    assert_eq!(a.level, TrustLevel::Medium);
    assert_eq!(a.flags, vec![TrustFlag::DescriptionUnavailable]);
}

/// The regression guard that matters most for a signature change touching
/// every call site: a REAL, non-empty description must leave every existing
/// check's output byte-for-byte unaffected — same score, same level, same
/// (empty) flag list as before this parameter existed.
#[test]
fn non_empty_description_does_not_affect_the_happy_path() {
    let a = assess_trust(
        "https://stripe.com/jobs/1",
        "Stripe",
        "We build payments infrastructure for the internet.",
    );
    assert_eq!(a.score, 100);
    assert_eq!(a.level, TrustLevel::High);
    assert!(a.flags.is_empty(), "expected no flags, got {:?}", a.flags);
}

/// Regression for CodeRabbit's finding on this PR: `assess_trust` previously
/// used a raw `.trim().is_empty()` check, weaker than
/// `crate::documents::keywords::description_is_blank` — the SAME
/// "is there usable scoring text" concept `commands::autopilot`'s
/// `no_jd_text` already uses. A description that's just a bare URL has no
/// markdown noise to strip, but `markdown_to_plain` still reduces it to
/// nothing usable once run through the shared predicate. Mutation-checked:
/// reverting to the raw `.trim().is_empty()` check turns this red (the flag
/// stops firing, level stays `High`).
#[test]
fn bare_url_description_flags_and_caps_below_high() {
    let a = assess_trust(
        "https://stripe.com/jobs/1",
        "Stripe",
        "http://example.com/careers",
    );
    assert_ne!(a.level, TrustLevel::High, "must be capped below High");
    assert_eq!(a.flags, vec![TrustFlag::DescriptionUnavailable]);
}

/// Same regression, markdown-only noise instead of a bare URL — both inputs
/// read as "non-empty" under the old raw check but reduce to nothing under
/// `markdown_to_plain`.
#[test]
fn markdown_only_noise_description_flags_and_caps_below_high() {
    for description in ["***", "[](url)"] {
        let a = assess_trust("https://stripe.com/jobs/1", "Stripe", description);
        assert_ne!(
            a.level,
            TrustLevel::High,
            "description={description:?} — must be capped below High"
        );
        assert_eq!(
            a.flags,
            vec![TrustFlag::DescriptionUnavailable],
            "description={description:?}"
        );
    }
}

/// Stacks with an independent flag (`CompanyDomainMismatch`) rather than
/// replacing it — a LinkedIn-shaped stub AND a mismatched host are two
/// separate problems, both worth surfacing.
#[test]
fn empty_description_stacks_with_company_domain_mismatch() {
    let a = assess_trust("https://randomhost.xyz/j", "Acme", "");
    assert_eq!(
        a.flags,
        vec![
            TrustFlag::CompanyDomainMismatch,
            TrustFlag::DescriptionUnavailable
        ]
    );
    assert_eq!(a.score, 70);
    assert_eq!(a.level, TrustLevel::Medium);
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
    let expected = assess_trust(&p.url, &p.company, p.description.as_deref().unwrap_or(""));

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

/// Regression for issue #1105 (trust half): LinkedIn's free/guest board
/// leaves every posting's `description` empty/`None` (a dead "will be filled
/// in background" promise). `build_found_job` is the real call site
/// (`commands/autopilot.rs`) threading the posting's description into
/// `assess_trust` — this proves that wiring, not a hand-retyped mirror.
#[test]
fn found_job_flags_description_unavailable_for_stubbed_posting() {
    let p = posting_without_description("https://linkedin.com/jobs/view/1", "Acme");

    let found = build_found_job(&p, "", 0);
    let trust = found
        .trust
        .expect("build_found_job must always set Some(..)");

    assert!(
        trust.flags.contains(&TrustFlag::DescriptionUnavailable),
        "expected DescriptionUnavailable, got {:?}",
        trust.flags
    );
    assert_ne!(trust.level, TrustLevel::High, "must be capped below High");
}

/// `build_found_job` pulls scraped salary out of `JobPosting.extra` (Adzuna's
/// shape) so it survives into the persisted `FoundJob`.
#[test]
fn build_found_job_extracts_salary_from_extra() {
    let mut p = posting("https://example.com/j", "Acme");
    p.extra
        .insert("salaryMin".to_string(), serde_json::json!(70_000.0));
    p.extra
        .insert("salaryMax".to_string(), serde_json::json!(90_000.0));
    p.extra
        .insert("salaryCurrency".to_string(), serde_json::json!("EUR"));

    let found = build_found_job(&p, "", 0);
    assert_eq!(found.salary_min, Some(70_000.0));
    assert_eq!(found.salary_max, Some(90_000.0));
    assert_eq!(found.salary_currency, Some("EUR".to_string()));
}

/// A posting with no salary keys in `extra` (every non-Adzuna board today) must
/// carry `None`, not panic or default to 0.
#[test]
fn build_found_job_missing_salary_keys_yield_none() {
    let p = posting("https://example.com/j", "Acme");
    let found = build_found_job(&p, "", 0);
    assert_eq!(found.salary_min, None);
    assert_eq!(found.salary_max, None);
    assert_eq!(found.salary_currency, None);
}
