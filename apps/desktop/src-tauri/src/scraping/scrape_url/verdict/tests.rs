use super::*;

fn p<'a>(title: &'a str, company: &'a str, url: &'a str) -> PostingIdentity<'a> {
    PostingIdentity {
        title,
        company,
        url,
    }
}

#[test]
fn same_linkedin_id_across_slug_and_regional_variants_is_same() {
    let stored = p(
        "Rust Engineer",
        "Acme",
        "https://www.linkedin.com/jobs/view/4464018189",
    );
    // Different title/company text must not matter once the board id matches.
    let resolved = p(
        "Totally Other",
        "Other Corp",
        "https://de.linkedin.com/jobs/view/rust-engineer-at-acme-4464018189",
    );
    assert_eq!(identity_verdict(stored, resolved), IdentityVerdict::Same);
}

#[test]
fn different_id_on_the_same_board_is_drift_even_with_identical_text() {
    let stored = p(
        "Rust Engineer",
        "Acme",
        "https://www.linkedin.com/jobs/view/111",
    );
    let resolved = p(
        "Rust Engineer",
        "Acme",
        "https://www.linkedin.com/jobs/view/222",
    );
    assert_eq!(
        identity_verdict(stored, resolved),
        IdentityVerdict::Drift(vec![DriftField::Id])
    );
}

#[test]
fn adzuna_redirect_to_greenhouse_with_noisy_title_and_suffix_is_same() {
    let stored = p(
        "Senior Rust Developer (m/w/d)",
        "Acme GmbH",
        "https://www.adzuna.de/land/ad/123456",
    );
    let resolved = p(
        "Senior Rust Developer",
        "Acme",
        "https://boards.greenhouse.io/acme/jobs/987",
    );
    assert_eq!(identity_verdict(stored, resolved), IdentityVerdict::Same);
}

#[test]
fn changed_company_is_drift() {
    let stored = p("Rust Engineer", "Acme", "https://www.adzuna.de/land/ad/1");
    let resolved = p(
        "Rust Engineer",
        "Globex",
        "https://boards.greenhouse.io/globex/jobs/1",
    );
    assert_eq!(
        identity_verdict(stored, resolved),
        IdentityVerdict::Drift(vec![DriftField::Company])
    );
}

#[test]
fn changed_role_is_drift() {
    let stored = p("Rust Engineer", "Acme", "https://www.adzuna.de/land/ad/1");
    let resolved = p(
        "Marketing Manager",
        "Acme",
        "https://boards.greenhouse.io/acme/jobs/2",
    );
    assert_eq!(
        identity_verdict(stored, resolved),
        IdentityVerdict::Drift(vec![DriftField::Title])
    );
}

#[test]
fn blank_resolved_fields_are_no_evidence_not_drift() {
    let stored = p("Rust Engineer", "Acme", "https://www.adzuna.de/land/ad/1");
    let resolved = p("", "", "https://example.com/careers/1");
    assert_eq!(identity_verdict(stored, resolved), IdentityVerdict::Same);
}

#[test]
fn id_on_one_side_only_falls_back_to_company_and_title() {
    let stored = p(
        "Rust Engineer",
        "Acme",
        "https://www.linkedin.com/jobs/view/111",
    );
    let resolved = p(
        "Rust Engineer",
        "Acme",
        "https://boards.greenhouse.io/acme/jobs/9",
    );
    assert_eq!(identity_verdict(stored, resolved), IdentityVerdict::Same);
}

#[test]
fn fingerprint_ignores_title_noise_but_not_the_url() {
    let a = identity_fingerprint(p("Dev (m/w/d)", "Acme GmbH", "https://x.example.com/a"));
    let b = identity_fingerprint(p("Dev", "Acme", "https://x.example.com/a"));
    let c = identity_fingerprint(p("Dev", "Acme", "https://x.example.com/b"));
    assert_eq!(a, b);
    assert_ne!(a, c);
}
