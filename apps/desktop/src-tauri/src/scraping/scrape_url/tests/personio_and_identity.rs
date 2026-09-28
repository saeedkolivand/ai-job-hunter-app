//! Personio id-composition consistency (resolver == board-scrape path) and
//! `job_identity`'s `(board, id)` extraction, issue #1166.

// ── Personio id consistency: resolver == board-scrape path ───────────────────
//
// Both ingestion paths (board scrape + URL resolve) must produce the same
// JobPosting.id for the same posting. Before this fix the resolver emitted
// `personio:{id}` while the board emitted `personio:{company}:{id}`.

/// `personio_company_from_url` correctly extracts the company slug from the
/// first host label and lowercases it.  This drives the extraction fn from
/// *real URL strings*, so the assertions fail if the fn stops parsing the host
/// or stops lowercasing — a hardcoded-literal test cannot catch those regressions.
#[test]
fn personio_company_from_url_extracts_slug() {
    use super::super::personio_company_from_url;

    // Standard `.de` subdomain → lowercase company slug.
    assert_eq!(
        personio_company_from_url("https://acme.jobs.personio.de/?id=42"),
        Some("acme".to_string()),
        "standard .de URL must yield company slug"
    );

    // `.com` variant must also work.
    assert_eq!(
        personio_company_from_url("https://globex.jobs.personio.com/job/99"),
        Some("globex".to_string()),
        ".com host variant must yield company slug"
    );

    // Uppercase in host: reqwest::Url normalises ASCII hosts to lowercase
    // before we even split — verify the fn handles that chain end-to-end.
    assert_eq!(
        personio_company_from_url("https://ACME.jobs.personio.de/?id=1"),
        Some("acme".to_string()),
        "uppercase host label must be normalised to lowercase"
    );

    // Bare root (no company subdomain) → None.
    assert_eq!(
        personio_company_from_url("https://jobs.personio.de/?id=5"),
        None,
        "bare personio root has no company subdomain"
    );

    // Non-Personio host → None.
    assert_eq!(
        personio_company_from_url("https://acme.example.com/jobs/42"),
        None,
        "non-Personio host must return None"
    );

    // Look-alike (suffix-evading) host → None.
    assert_eq!(
        personio_company_from_url("https://jobs.personio.de.evil.tld/?id=1"),
        None,
        "look-alike host must be rejected"
    );

    // Garbage / unparseable URL → None.
    assert_eq!(
        personio_company_from_url("not a url at all"),
        None,
        "unparseable URL must return None"
    );
}

/// Assert the full resolver id for a known URL+pos_id equals
/// `make_job_id(extracted_company, pos_id)` — i.e. the test fails if the
/// resolver stops extracting the company from the URL or stops using
/// `make_job_id`.  Unlike the previous test, both sides are NOT identical
/// expressions: one side drives `personio_company_from_url` from the URL
/// string; the other is the expected literal.
#[test]
fn personio_resolver_id_composition_is_non_tautological() {
    use super::super::personio_company_from_url;

    let url = "https://acme.jobs.personio.de/?id=42";
    let pos_id = "42";

    // Drive extraction from the URL — NOT a hardcoded company string.
    let extracted =
        personio_company_from_url(url).expect("well-formed Personio URL must yield a company slug");

    // If personio_company_from_url returns the wrong thing (e.g. "jobs" instead
    // of "acme", or the id instead of the slug), this assertion catches it.
    assert_eq!(
        extracted, "acme",
        "extracted slug must be the subdomain label"
    );

    // Compose the id the same way try_personio does.
    let resolver_id = crate::scraping::boards::personio::make_job_id(&extracted, pos_id);

    // If make_job_id format ever changes (e.g. drops the company), this fails.
    assert_eq!(
        resolver_id, "personio:acme:42",
        "resolver id must be personio:<company>:<pos_id>"
    );

    // Cross-check: the board-scrape path for the same company+id must be byte-identical.
    let board_id = crate::scraping::boards::personio::make_job_id("acme", pos_id);
    assert_eq!(
        resolver_id, board_id,
        "resolver and board-scrape must produce byte-identical ids for the same posting"
    );
}

// ── job_identity: (board, id) identity for lookup, issue #1166 ────────────

#[test]
fn job_identity_linkedin_numeric_view_form() {
    assert_eq!(
        super::super::job_identity("https://www.linkedin.com/jobs/view/4185657072"),
        Some(("linkedin", "4185657072".to_string()))
    );
}

#[test]
fn job_identity_linkedin_slugged_view_form_extracts_trailing_digits() {
    assert_eq!(
        super::super::job_identity(
            "https://de.linkedin.com/jobs/view/ai-software-engineer-at-hyra-4464018189"
        ),
        Some(("linkedin", "4464018189".to_string()))
    );
}

#[test]
fn job_identity_linkedin_current_job_id_form_matches_the_view_form() {
    assert_eq!(
        super::super::job_identity("https://www.linkedin.com/jobs/search/?currentJobId=4185657072"),
        super::super::job_identity("https://www.linkedin.com/jobs/view/4185657072")
    );
}

#[test]
fn job_identity_linkedin_folds_regional_and_apex_hosts() {
    let want = Some(("linkedin", "4185657072".to_string()));
    for host in [
        "linkedin.com",
        "www.linkedin.com",
        "de.linkedin.com",
        "uk.linkedin.com",
    ] {
        assert_eq!(
            super::super::job_identity(&format!("https://{host}/jobs/view/4185657072")),
            want,
            "host {host} must fold to the same identity"
        );
    }
}

#[test]
fn job_identity_linkedin_ignores_scheme_and_a_missing_scheme() {
    let want = Some(("linkedin", "4185657072".to_string()));
    for url in [
        "http://www.linkedin.com/jobs/view/4185657072",
        "https://www.linkedin.com/jobs/view/4185657072",
        "www.linkedin.com/jobs/view/4185657072",
        "linkedin.com/jobs/view/4185657072",
    ] {
        assert_eq!(
            super::super::job_identity(url),
            want,
            "{url} must resolve to {want:?}"
        );
    }
}

#[test]
fn job_identity_linkedin_rejects_lookalike_host() {
    assert_eq!(
        super::super::job_identity("https://linkedin.com.attacker.tld/jobs/view/123"),
        None
    );
}

#[test]
fn job_identity_indeed_vjk_and_jk_forms_match() {
    assert_eq!(
        super::super::job_identity("https://www.indeed.com/jobs?q=x&vjk=9b6647ed6c731326"),
        super::super::job_identity("https://de.indeed.com/viewjob?jk=9b6647ed6c731326")
    );
}

#[test]
fn job_identity_unknown_host_is_none() {
    assert_eq!(
        super::super::job_identity("https://boards.example.com/jobs/42"),
        None
    );
}

// PR 7 verify-live gate: Xing and StepStone were live-probed via browser
// automation (public, no-login search); in those sessions neither host left the
// current tab on a list-shell URL with the selected job only in a query param —
// clicking a job title fully navigated straight to the canonical detail URL (id
// already in the path). These four tests are regression guards against a
// careless future match arm "fixing" a shape that was never broken: they pin
// `canonical_job_url` returning `None` for the exact captured list-search and
// detail URL shapes (tracking query params included, to prove the function
// ignores them) for both hosts. See the TODO above `canonical_job_url` for the
// StepStone login-gated caveat this observation doesn't cover.

#[test]
fn canonical_xing_list_search_url_is_none() {
    assert_eq!(
        super::super::canonical_job_url(
            "https://www.xing.com/jobs/search?keywords=software+engineer&location=Berlin"
        ),
        None
    );
}

#[test]
fn canonical_xing_detail_url_is_none() {
    assert_eq!(
        super::super::canonical_job_url(
            "https://www.xing.com/jobs/berlin-senior-software-engineer-155853218?ijt=jb_55"
        ),
        None
    );
}

#[test]
fn canonical_stepstone_list_search_url_is_none() {
    assert_eq!(
        super::super::canonical_job_url(
            "https://www.stepstone.de/jobs/software-engineer/in-berlin"
        ),
        None
    );
}

#[test]
fn canonical_stepstone_detail_url_is_none() {
    assert_eq!(
        super::super::canonical_job_url(
            "https://www.stepstone.de/stellenangebote--Software-Engineer-m-w-d-Distribution-Berlin-GEMA-Gesellschaft-fuer-musik-Auffuehrungs-und-mechan-Vervielfaeltigungsrechte--14009455-inline.html?rltr=1_1_25_seorl_m_0_0_0_0_0_0"
        ),
        None
    );
}
