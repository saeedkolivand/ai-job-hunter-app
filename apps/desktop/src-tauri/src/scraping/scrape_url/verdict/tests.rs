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
    assert_eq!(identity_verdict(stored, resolved), IdentityVerdict::Drift);
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
    assert_eq!(identity_verdict(stored, resolved), IdentityVerdict::Drift);
}

#[test]
fn changed_role_is_drift() {
    let stored = p("Rust Engineer", "Acme", "https://www.adzuna.de/land/ad/1");
    let resolved = p(
        "Marketing Manager",
        "Acme",
        "https://boards.greenhouse.io/acme/jobs/2",
    );
    assert_eq!(identity_verdict(stored, resolved), IdentityVerdict::Drift);
}

#[test]
fn nothing_comparable_is_no_evidence_but_one_blank_field_is_skipped() {
    let stored = p("Rust Engineer", "Acme", "https://www.adzuna.de/land/ad/1");
    assert_eq!(
        identity_verdict(stored, p("", "", "https://example.com/careers/1")),
        IdentityVerdict::NoEvidence
    );
    // Blank company is skipped, the matching title still proves Same.
    assert_eq!(
        identity_verdict(
            stored,
            p("Rust Engineer", "", "https://example.com/careers/1")
        ),
        IdentityVerdict::Same
    );
}

#[test]
fn title_suffix_seniority_abbreviation_and_company_prefix_are_same() {
    let url_a = "https://www.adzuna.de/land/ad/1";
    let url_b = "https://boards.greenhouse.io/x/jobs/1";
    for (st, sc, rt, rc) in [
        (
            "Software Engineer",
            "Acme",
            "Software Engineer, Payments",
            "Acme",
        ),
        (
            "Sr. Software Engineer",
            "Acme",
            "Senior Software Engineer",
            "Acme",
        ),
        ("Jr Developer", "Acme", "Junior Developer", "Acme"),
        ("Engineer", "BMW Group", "Engineer", "BMW AG"),
        ("Engineer", "Amazon", "Engineer", "Amazon Web Services"),
    ] {
        assert_eq!(
            identity_verdict(p(st, sc, url_a), p(rt, rc, url_b)),
            IdentityVerdict::Same,
            "{st}/{sc} vs {rt}/{rc}"
        );
    }
}

#[test]
fn real_world_drifts_stay_drift() {
    let url_a = "https://www.adzuna.de/land/ad/1";
    let url_b = "https://boards.greenhouse.io/x/jobs/1";
    // Clinora: same company, a different role.
    assert_eq!(
        identity_verdict(
            p("BDR", "Clinora", url_a),
            p("Customer Success", "Clinora", url_b)
        ),
        IdentityVerdict::Drift
    );
    // A different company entirely.
    assert!(matches!(
        identity_verdict(
            p("Engineer", "OMRON", url_a),
            p("Engineer", "Aratas", url_b)
        ),
        IdentityVerdict::Drift
    ));
    // The same title at a different company is always drift.
    assert_eq!(
        identity_verdict(
            p("Senior Engineer", "Acme", url_a),
            p("Senior Engineer", "Globex", url_b)
        ),
        IdentityVerdict::Drift
    );
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

#[test]
fn fingerprint_is_host_independent_where_the_board_id_is_known() {
    let de = identity_fingerprint(p("Dev", "Acme", "https://de.linkedin.com/jobs/view/42"));
    let www = identity_fingerprint(p(
        "Dev",
        "Acme",
        "https://www.linkedin.com/jobs/view/dev-at-acme-42",
    ));
    assert_eq!(de, www);
}

#[test]
fn different_roles_at_the_same_company_are_drift() {
    let (ua, ub) = (
        "https://www.adzuna.de/land/ad/1",
        "https://boards.greenhouse.io/x/jobs/1",
    );
    for (a, b) in [
        ("Product Manager", "Project Manager"),
        ("Software Engineer Frontend", "Software Engineer Backend"),
        ("Software Engineer", "Software Engineer Intern"),
        ("Software Engineer", "Senior Software Engineer"),
        ("Software Engineer", "Sr. Software Engineer"),
        ("Engineer", "Marketing Engineer"),
    ] {
        assert_eq!(
            identity_verdict(p(a, "Acme", ua), p(b, "Acme", ub)),
            IdentityVerdict::Drift,
            "{a} vs {b}"
        );
    }
}

#[test]
fn gender_noise_and_a_team_suffix_are_same() {
    let (ua, ub) = (
        "https://www.adzuna.de/land/ad/1",
        "https://boards.greenhouse.io/x/jobs/1",
    );
    assert_eq!(
        identity_verdict(
            p("Software Engineer (m/w/d)", "Acme", ua),
            p("Software Engineer, Payments", "Acme", ub)
        ),
        IdentityVerdict::Same
    );
}

#[test]
fn company_prefix_must_end_on_a_word_boundary() {
    let (ua, ub) = (
        "https://www.adzuna.de/land/ad/1",
        "https://boards.greenhouse.io/x/jobs/1",
    );
    assert_eq!(
        identity_verdict(
            p("Rust Engineer", "Acme", ua),
            p("Rust Engineer", "Acmeco", ub)
        ),
        IdentityVerdict::Drift
    );
}

#[test]
fn role_noun_synonyms_fold_but_distinct_roles_do_not() {
    let (ua, ub) = (
        "https://www.adzuna.de/land/ad/1",
        "https://boards.greenhouse.io/x/jobs/1",
    );
    for (a, b) in [
        ("Backend Developer", "Backend Engineer"),
        ("Software Dev", "Software Engineer"),
        ("Sr. Programmer", "Senior Engineer"),
    ] {
        assert_eq!(
            identity_verdict(p(a, "Acme", ua), p(b, "Acme", ub)),
            IdentityVerdict::Same,
            "{a} vs {b}"
        );
    }
    assert_eq!(
        identity_verdict(
            p("Product Manager", "Acme", ua),
            p("Project Manager", "Acme", ub)
        ),
        IdentityVerdict::Drift
    );
}
