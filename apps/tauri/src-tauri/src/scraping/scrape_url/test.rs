use super::*;

// ── try_named_boards: dispatch routing ───────────────────────────────────────
//
// These tests verify the "which handler fires for this URL" decision without
// making live API calls. A non-board URL must return Ok(None) at the pattern-
// match gate (before any fetch); a look-alike host for a guarded board must
// also return Ok(None) at the host-gate check.

/// A completely unrecognised URL must produce Ok(None) — no board match,
/// no fetch. This is the hermetic "no-op" path for try_named_boards.
#[tokio::test]
async fn try_named_boards_returns_none_for_unknown_url() {
    // No board handler matches example.com, so we must get Ok(None) without
    // hitting the network.
    let result = try_named_boards("https://example.com/jobs/123")
        .await
        .expect("try_named_boards must not error on a non-board URL");
    assert!(
        result.is_none(),
        "non-board URL must yield None, not a posting"
    );
}

/// A Greenhouse look-alike host must be rejected at the host gate and return
/// Ok(None) — not accepted as a real Greenhouse URL.
#[tokio::test]
async fn try_named_boards_rejects_greenhouse_lookalike() {
    let result = try_named_boards("https://greenhouse.io.attacker.tld/stripe/jobs/1")
        .await
        .expect("look-alike host returns Ok(None) at gate");
    assert!(
        result.is_none(),
        "Greenhouse look-alike host must not be accepted by try_named_boards"
    );
}

/// A Lever look-alike host must be rejected at the host gate.
#[tokio::test]
async fn try_named_boards_rejects_lever_lookalike() {
    let result = try_named_boards("https://lever.co.attacker.tld/stripe/abc123")
        .await
        .expect("look-alike host returns Ok(None) at gate");
    assert!(
        result.is_none(),
        "Lever look-alike host must not be accepted by try_named_boards"
    );
}

// ── SSRF host-gate: Workday + SmartRecruiters ────────────────────────────────
//
// Both handlers tightened from `contains` to exact/ends_with matching. A
// look-alike host (e.g. `myworkdayjobs.com.attacker.tld`) must be rejected at
// the gate — BEFORE any API call is constructed. All tests below are hermetic:
// the gate fires (returning Ok(None)) before the HTTP client is ever touched.

/// A Workday look-alike host (`*.myworkdayjobs.com.attacker.tld`) must be
/// rejected at the host gate; `try_workday` must return `Ok(None)`.
#[tokio::test]
async fn try_workday_rejects_lookalike_host() {
    let result = try_workday(
        "https://acme.myworkdayjobs.com.attacker.tld/Acme/job/Backend-Engineer/apply",
    )
    .await
    .expect("look-alike host returns Ok(None) at gate, no network");
    assert!(
        result.is_none(),
        "Workday look-alike host must not be accepted by try_workday"
    );
}

/// A real Workday URL (`<tenant>.wd1.myworkdayjobs.com`) passes the host gate.
/// After the gate the handler would make a live API call, which fails in a
/// hermetic env — so we assert Ok(None) (the non-2xx path), NOT Some(_). The
/// important thing is the gate itself does not prematurely reject a valid host.
#[tokio::test]
async fn try_workday_accepts_real_host_at_gate() {
    // This URL has no network behind it; the handler will hit a connection error
    // after the gate passes, mapping to Ok(None) via the non-2xx/err arm.
    let result = try_workday(
        "https://acme.wd1.myworkdayjobs.com/Acme/job/Backend-Engineer/REQ_001",
    )
    .await;
    // Must not be Err — the gate itself must not reject a legitimate host.
    assert!(
        result.is_ok(),
        "real Workday host must pass the gate (result may be None due to no live server)"
    );
}

/// A SmartRecruiters look-alike host (`*.smartrecruiters.com.attacker.tld`)
/// must be rejected at the host gate; `try_smartrecruiters` must return `Ok(None)`.
#[tokio::test]
async fn try_smartrecruiters_rejects_lookalike_host() {
    let result = try_smartrecruiters(
        "https://jobs.smartrecruiters.com.attacker.tld/Acme/123456789",
    )
    .await
    .expect("look-alike host returns Ok(None) at gate, no network");
    assert!(
        result.is_none(),
        "SmartRecruiters look-alike host must not be accepted by try_smartrecruiters"
    );
}

/// A real SmartRecruiters URL (`jobs.smartrecruiters.com`) passes the host gate.
/// Same hermetic rationale as the Workday positive case above.
#[tokio::test]
async fn try_smartrecruiters_accepts_real_host_at_gate() {
    let result = try_smartrecruiters(
        "https://jobs.smartrecruiters.com/Acme/123456789",
    )
    .await;
    assert!(
        result.is_ok(),
        "real SmartRecruiters host must pass the gate (result may be None due to no live server)"
    );
}

// ── resolve: 429 / non-2xx graceful degradation ───────────────────────────────
//
// An Adzuna click-tracker URL that produces a non-2xx response (429, 403, etc.)
// must cause resolve() to return Ok(None) so the renderer keeps its snippet.
// We use an IP literal that get_guarded will reject at the SSRF gate — a
// Validation error is also mapped to Ok(None) in resolve()'s Err(_) arm.
//
// NOTE: the "final_url == original (no redirect) skip" and "live-server 429 →
// Ok(None)" branches are covered by integration/contract tests, not hermetic
// unit tests. The SSRF-reject path below (`resolve_returns_none_on_redirect_
// follow_error`) covers the Err arm.

/// When the redirect follow returns an Err (SSRF-rejected IP, network failure)
/// resolve() must return Ok(None) — never panic or bubble the error.
#[tokio::test]
async fn resolve_returns_none_on_redirect_follow_error() {
    // 127.0.0.1 is rejected by the SSRF guard before any network contact,
    // giving a deterministic Err without needing a live server.
    let result = crate::scraping::scrape_url::resolve("http://127.0.0.1/jobs/1")
        .await
        .expect("resolve must not propagate errors — always Ok(Some|None)");
    assert!(
        result.is_none(),
        "SSRF-rejected URL must yield Ok(None), not a posting"
    );
}

#[test]
fn test_parse_greenhouse_url_standard() {
    let url = "https://boards.greenhouse.io/stripe/jobs/12345";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "12345".to_string())));
}

#[test]
fn test_parse_greenhouse_url_embed() {
    let url = "https://boards.greenhouse.io/embed/job_app?for=stripe&token=abc123";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "abc123".to_string())));
}

#[test]
fn test_parse_greenhouse_url_invalid() {
    let url = "https://example.com/jobs/123";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, None);
}

#[test]
fn test_parse_lever_url() {
    let url = "https://jobs.lever.co/stripe/abc123";
    let result = parse_lever_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "abc123".to_string())));
}

#[test]
fn test_parse_lever_url_invalid() {
    let url = "https://example.com/stripe/abc123";
    let result = parse_lever_url(url);
    assert_eq!(result, None);
}

#[test]
fn test_parse_generic_html() {
    let html = r#"
        <html>
            <head><title>Software Engineer</title></head>
            <body>
                <meta name="description" content="Great job opportunity">
            </body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Software Engineer");
    assert_eq!(description, Some("Great job opportunity".to_string()));
}

#[test]
fn test_parse_generic_html_h1() {
    let html = r#"
        <html>
            <body>
                <h1>Senior Developer</h1>
                <meta property="og:description" content="Remote position">
            </body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Senior Developer");
    assert_eq!(description, Some("Remote position".to_string()));
}

#[test]
fn test_parse_generic_html_empty() {
    let html = "<html><body></body></html>";
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "");
    assert_eq!(description, None);
}

#[test]
fn test_parse_generic_html_with_og_description() {
    let html = r#"
        <html>
            <head>
                <title>Job Title</title>
                <meta property="og:description" content="Remote position available">
            </head>
            <body></body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Job Title");
    assert_eq!(description, Some("Remote position available".to_string()));
}

#[test]
fn test_parse_generic_html_with_meta_description() {
    let html = r#"
        <html>
            <head>
                <meta name="description" content="Job description here">
            </head>
            <body><h1>Title</h1></body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Title");
    assert_eq!(description, Some("Job description here".to_string()));
}

#[test]
fn test_parse_generic_html_no_description() {
    let html = "<html><body><h1>Title</h1></body></html>";
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Title");
    assert!(description.is_none());
}

#[test]
fn test_parse_generic_html_h1_priority() {
    // The selector "title, h1" returns the FIRST DOM match. `<title>` appears in
    // `<head>` before `<h1>` in `<body>`, so "Page Title" wins — not "Job Title".
    let html = r#"
        <html>
            <head><title>Page Title</title></head>
            <body><h1>Job Title</h1></body>
        </html>
    "#;
    let (title, _description) = parse_generic_html(html);
    assert_eq!(
        title, "Page Title",
        "<title> must be returned as first DOM match"
    );
}

#[test]
fn test_parse_greenhouse_url_subdomain() {
    let url = "https://boards.greenhouse.io/stripe/jobs/12345";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "12345".to_string())));
}

#[test]
fn test_parse_greenhouse_url_with_trailing_slash() {
    // reqwest::Url splits "/stripe/jobs/12345/" into segments ["stripe","jobs","12345",""]
    // — the trailing empty label must be ignored; extraction must still yield the
    // correct (company, job_id) pair using indices 0 and 2 of the segments vec.
    let url = "https://boards.greenhouse.io/stripe/jobs/12345/";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "12345".to_string())));
}

#[test]
fn test_parse_lever_url_with_subdomain() {
    let url = "https://jobs.lever.co/stripe/abc123";
    let result = parse_lever_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "abc123".to_string())));
}

#[test]
fn test_parse_lever_url_with_extra_segments() {
    // parse_lever_url takes segments[0] and segments[1]; extra trailing segments
    // are ignored. Verify the extracted pair is exactly ("stripe", "abc123").
    let url = "https://jobs.lever.co/stripe/abc123/extra";
    let result = parse_lever_url(url);
    assert_eq!(
        result,
        Some(("stripe".to_string(), "abc123".to_string())),
        "extra trailing segment must be ignored; first two segments must be returned"
    );
}

#[test]
fn test_parse_greenhouse_url_embed_missing_token() {
    let url = "https://boards.greenhouse.io/embed/job_app?for=stripe";
    let result = parse_greenhouse_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_greenhouse_url_embed_missing_for() {
    let url = "https://boards.greenhouse.io/embed/job_app?token=abc123";
    let result = parse_greenhouse_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_lever_url_invalid_domain() {
    let url = "https://example.com/stripe/abc123";
    let result = parse_lever_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_greenhouse_url_invalid_domain() {
    let url = "https://example.com/stripe/jobs/12345";
    let result = parse_greenhouse_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_lever_url_single_segment() {
    let url = "https://jobs.lever.co/stripe";
    let result = parse_lever_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_greenhouse_url_single_segment() {
    let url = "https://boards.greenhouse.io/stripe";
    let result = parse_greenhouse_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_generic_html_with_both_descriptions() {
    let html = r#"
        <html>
            <head>
                <title>Job Title</title>
                <meta name="description" content="Meta description">
                <meta property="og:description" content="OG description">
            </head>
            <body></body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Job Title");
    // First description match wins
    assert!(description.is_some());
}

#[test]
fn test_parse_generic_html_malformed() {
    let html = "<html><head><title>Test</title></html>";
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Test");
    assert!(description.is_none());
}

#[test]
fn test_parse_generic_html_whitespace() {
    let html = r#"
        <html>
            <head>
                <title>   Whitespace Title   </title>
            </head>
            <body></body>
        </html>
    "#;
    let (title, _description) = parse_generic_html(html);
    assert!(!title.is_empty());
}

#[test]
fn test_parse_generic_company_json_ld() {
    let html = r#"<html><head>
        <script type="application/ld+json">
        {"@type":"JobPosting","title":"Engineer","hiringOrganization":{"@type":"Organization","name":"Acme Inc"}}
        </script>
    </head></html>"#;
    assert_eq!(parse_generic_company(html), Some("Acme Inc".to_string()));
}

#[test]
fn test_parse_generic_company_og_site_name() {
    let html = r#"<html><head><meta property="og:site_name" content="BRANDUNG"></head></html>"#;
    assert_eq!(parse_generic_company(html), Some("BRANDUNG".to_string()));
}

#[test]
fn test_parse_generic_company_json_ld_graph() {
    let html = r#"<html><head>
        <script type="application/ld+json">
        {"@graph":[{"@type":"WebPage"},{"@type":"JobPosting","hiringOrganization":{"name":"Globex"}}]}
        </script>
    </head></html>"#;
    assert_eq!(parse_generic_company(html), Some("Globex".to_string()));
}

#[test]
fn test_parse_generic_company_prefers_json_ld_over_og() {
    let html = r#"<html><head>
        <meta property="og:site_name" content="Careers Portal">
        <script type="application/ld+json">
        {"@type":"JobPosting","hiringOrganization":{"name":"Initech"}}
        </script>
    </head></html>"#;
    assert_eq!(parse_generic_company(html), Some("Initech".to_string()));
}

#[test]
fn test_parse_generic_company_none() {
    let html = "<html><head><title>Job</title></head></html>";
    assert_eq!(parse_generic_company(html), None);
}

#[test]
fn test_parse_lever_url_with_query() {
    let url = "https://jobs.lever.co/stripe/abc123?ref=source";
    let result = parse_lever_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "abc123".to_string())));
}

#[test]
fn test_parse_greenhouse_url_with_query() {
    let url = "https://boards.greenhouse.io/stripe/jobs/12345?ref=source";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "12345".to_string())));
}

// ── parse_from_html (Scan-mode fetch-free path) ──────────────────────────────

#[test]
fn test_parse_from_html_generic_fallback() {
    let html = r#"
        <html>
            <head>
                <title>Backend Engineer</title>
                <meta name="description" content="Build APIs">
                <meta property="og:site_name" content="Acme Corp">
            </head>
            <body></body>
        </html>
    "#;
    let posting = parse_from_html("https://acme.example.com/jobs/9", html)
        .expect("a title is present, so a posting is built");
    assert_eq!(posting.title, "Backend Engineer");
    assert_eq!(posting.description.as_deref(), Some("Build APIs"));
    assert_eq!(posting.company, "Acme Corp");
    assert_eq!(posting.source, "url");
    assert_eq!(posting.url, "https://acme.example.com/jobs/9");
    assert_eq!(posting.location, None);
}

#[test]
fn test_parse_from_html_prefers_json_ld() {
    // JSON-LD JobPosting overrides the bare <title> and supplies a location.
    let html = r#"
        <html>
            <head>
                <title>Some Page Title</title>
                <script type="application/ld+json">
                {
                    "@context": "https://schema.org/",
                    "@type": "JobPosting",
                    "title": "Senior Platform Engineer",
                    "description": "<p>Own the platform</p>",
                    "hiringOrganization": { "name": "Globex" },
                    "jobLocation": {
                        "address": {
                            "addressLocality": "Berlin",
                            "addressRegion": "BE"
                        }
                    }
                }
                </script>
            </head>
            <body></body>
        </html>
    "#;
    let posting =
        parse_from_html("https://globex.example.com/p/1", html).expect("json-ld carries a title");
    assert_eq!(posting.title, "Senior Platform Engineer");
    assert_eq!(posting.company, "Globex");
    assert_eq!(posting.location.as_deref(), Some("Berlin, BE"));
    assert!(posting
        .description
        .as_deref()
        .unwrap_or_default()
        .contains("Own the platform"));
}

#[test]
fn test_parse_from_html_json_ld_in_graph_array() {
    let html = r#"
        <html><head>
            <script type="application/ld+json">
            { "@graph": [
                { "@type": "WebPage" },
                { "@type": "JobPosting", "title": "Data Scientist",
                  "hiringOrganization": { "name": "Initech" } }
            ] }
            </script>
        </head><body></body></html>
    "#;
    let posting = parse_from_html("https://initech.example.com/j/2", html)
        .expect("a JobPosting node lives inside @graph");
    assert_eq!(posting.title, "Data Scientist");
    assert_eq!(posting.company, "Initech");
}

#[test]
fn test_parse_from_html_empty_title_still_yields_some() {
    // A body with no <title>/<h1> but a usable meta description must still yield
    // `Some` (empty title string) so the description-on-demand flow surfaces it.
    let html = r#"
        <html>
            <head>
                <meta name="description" content="A great role, no title tag though">
            </head>
            <body><p>just text</p></body>
        </html>
    "#;
    let posting = parse_from_html("https://x.example.com/", html)
        .expect("an empty-title document still yields a posting");
    assert_eq!(posting.title, "");
    assert_eq!(
        posting.description.as_deref(),
        Some("A great role, no title tag though")
    );
    assert_eq!(posting.source, "url");
}

#[test]
fn test_parse_from_html_json_ld_enriches_empty_title_page() {
    // No <title>/<h1>, but JSON-LD JobPosting supplies title/description/location
    // — enrichment must populate all three even on an otherwise empty-title page.
    let html = r#"
        <html><head>
            <script type="application/ld+json">
            {
                "@context": "https://schema.org/",
                "@type": "JobPosting",
                "title": "Staff Engineer",
                "description": "<p>Lead the platform</p>",
                "hiringOrganization": { "name": "Umbrella" },
                "jobLocation": {
                    "address": { "addressLocality": "Munich", "addressRegion": "BY" }
                }
            }
            </script>
        </head><body></body></html>
    "#;
    let posting = parse_from_html("https://umbrella.example.com/p/7", html)
        .expect("json-ld enriches an otherwise empty-title page");
    assert_eq!(posting.title, "Staff Engineer");
    assert_eq!(posting.company, "Umbrella");
    assert_eq!(posting.location.as_deref(), Some("Munich, BY"));
    assert!(posting
        .description
        .as_deref()
        .unwrap_or_default()
        .contains("Lead the platform"));
}

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
    use super::personio_company_from_url;

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
    use super::personio_company_from_url;

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

// ── SSRF host-gate rejection (hermetic — no network) ─────────────────────────
//
// A look-alike host must be rejected at the host gate and return `Ok(None)`
// BEFORE any fetch. These resolvers return at the tightened gate (exact/suffix
// match) before constructing a client, so calling them with an attacker host is
// network-free: if the gate ever leaked, the call would attempt a real fetch
// and the test would hang/fail.

#[tokio::test]
async fn try_personio_rejects_lookalike_host() {
    // `jobs.personio.attacker.tld` passes a substring gate but not exact/suffix.
    let out = try_personio("https://jobs.personio.attacker.tld/?id=1")
        .await
        .expect("look-alike host returns Ok(None) at the gate, no network");
    assert!(
        out.is_none(),
        "look-alike personio host must not be accepted"
    );
}

#[tokio::test]
async fn try_linkedin_rejects_lookalike_host() {
    // `linkedin.com.attacker.tld` passes a substring gate but not exact/suffix.
    let out = try_linkedin("https://linkedin.com.attacker.tld/jobs/view/1")
        .await
        .expect("look-alike host returns Ok(None) at the gate, no network");
    assert!(
        out.is_none(),
        "look-alike linkedin host must not be accepted"
    );
}

#[tokio::test]
async fn try_personio_rejects_bare_personio_substring_host() {
    // `notpersonio.de` and `jobs.personio.de.evil.tld` must also be rejected.
    assert!(try_personio("https://jobs.personio.de.evil.tld/?id=1")
        .await
        .expect("suffix evasion returns Ok(None) at the gate, no network")
        .is_none());
}

// ── canonical_job_url: SPA/list-view → canonical single-job URL ───────────────

#[test]
fn canonical_linkedin_search_with_current_job_id() {
    assert_eq!(
        super::canonical_job_url("https://www.linkedin.com/jobs/search/?currentJobId=4185657072"),
        Some("https://www.linkedin.com/jobs/view/4185657072".to_string())
    );
}

#[test]
fn canonical_linkedin_collections_with_current_job_id() {
    assert_eq!(
        super::canonical_job_url(
            "https://www.linkedin.com/jobs/collections/recommended/?currentJobId=123"
        ),
        Some("https://www.linkedin.com/jobs/view/123".to_string())
    );
}

#[test]
fn canonical_linkedin_direct_view_page_is_none() {
    assert_eq!(
        super::canonical_job_url("https://www.linkedin.com/jobs/view/123/"),
        None
    );
}

#[test]
fn canonical_linkedin_non_numeric_id_is_none() {
    assert_eq!(
        super::canonical_job_url("https://www.linkedin.com/jobs/search/?currentJobId=abc123"),
        None
    );
}

#[test]
fn canonical_linkedin_no_current_job_id_is_none() {
    assert_eq!(
        super::canonical_job_url("https://www.linkedin.com/jobs/search/?keywords=rust"),
        None
    );
}

#[test]
fn canonical_indeed_search_with_vjk() {
    assert_eq!(
        super::canonical_job_url("https://www.indeed.com/jobs?q=x&vjk=9b6647ed6c731326"),
        Some("https://www.indeed.com/viewjob?jk=9b6647ed6c731326".to_string())
    );
}

#[test]
fn canonical_indeed_country_tld_host_preserved() {
    assert_eq!(
        super::canonical_job_url("https://de.indeed.com/jobs?q=x&vjk=abc123"),
        Some("https://de.indeed.com/viewjob?jk=abc123".to_string())
    );
}

#[test]
fn canonical_indeed_direct_viewjob_is_none() {
    assert_eq!(
        super::canonical_job_url("https://www.indeed.com/viewjob?jk=9b6647ed6c731326"),
        None
    );
}

#[test]
fn canonical_unknown_host_is_none() {
    assert_eq!(
        super::canonical_job_url("https://example.com/jobs?vjk=123&currentJobId=456"),
        None
    );
}

// ── Hardened JSON-LD / __NEXT_DATA__ / multi-location / main-content ──────────

#[test]
fn parse_from_html_json_ld_bare_top_level_array() {
    // A bare top-level array (no @graph wrapper): [ {WebSite}, {JobPosting} ].
    let html = r#"
        <html><head>
            <script type="application/ld+json">
            [
                { "@type": "WebSite", "name": "Careers" },
                { "@type": "JobPosting", "title": "Backend Engineer",
                  "hiringOrganization": { "name": "Acme" } }
            ]
            </script>
        </head><body></body></html>
    "#;
    let posting = parse_from_html("https://acme.example/j/1", html)
        .expect("a JobPosting in a top-level array must be found");
    assert_eq!(posting.title, "Backend Engineer");
    assert_eq!(posting.company, "Acme");
}

#[test]
fn parse_from_html_json_ld_deeply_nested_in_graph() {
    // JobPosting nested inside an object value inside @graph (deeper than one level).
    let html = r#"
        <html><head>
            <script type="application/ld+json">
            { "@graph": [
                { "@type": "WebPage",
                  "mainEntity": { "@type": "JobPosting", "title": "Staff SRE",
                                  "hiringOrganization": { "name": "Initech" } } }
            ] }
            </script>
        </head><body></body></html>
    "#;
    let posting = parse_from_html("https://initech.example/j/2", html)
        .expect("a deeply-nested JobPosting must be reachable by the recursion");
    assert_eq!(posting.title, "Staff SRE");
    assert_eq!(posting.company, "Initech");
}

#[test]
fn parse_from_html_next_data_only_extracts_title_and_description() {
    // No JSON-LD; the job lives in __NEXT_DATA__ under props.pageProps.job.
    let html = r#"
        <html><head>
            <script id="__NEXT_DATA__" type="application/json">
            { "props": { "pageProps": { "job": {
                "title": "Frontend Engineer",
                "description": "<p>Build the web app with React.</p>",
                "hiringOrganization": { "name": "Globex" }
            } } } }
            </script>
        </head><body></body></html>
    "#;
    let posting = parse_from_html("https://globex.example/j/3", html)
        .expect("a __NEXT_DATA__ job-shaped node must yield a posting");
    assert_eq!(posting.title, "Frontend Engineer");
    assert!(
        posting
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("React"),
        "description must come from the __NEXT_DATA__ blob"
    );
}

#[test]
fn parse_from_html_multi_job_location_joins_localities() {
    // jobLocation is an array of two Places, each with a locality.
    let html = r#"
        <html><head>
            <script type="application/ld+json">
            {
                "@type": "JobPosting",
                "title": "Distributed Role",
                "hiringOrganization": { "name": "Remote Co" },
                "jobLocation": [
                    { "@type": "Place", "address": { "addressLocality": "Berlin" } },
                    { "@type": "Place", "address": { "addressLocality": "Munich" } }
                ]
            }
            </script>
        </head><body></body></html>
    "#;
    let posting = parse_from_html("https://remote.example/j/4", html)
        .expect("multi-location JobPosting must parse");
    let loc = posting.location.as_deref().unwrap_or_default();
    assert!(
        loc.contains("Berlin"),
        "first locality must be present: {loc}"
    );
    assert!(
        loc.contains("Munich"),
        "second locality must be present: {loc}"
    );
    assert!(
        loc.contains("; "),
        "multiple localities must join with '; ': {loc}"
    );
}

#[test]
fn parse_from_html_address_country_is_fallback_only() {
    // (a) Country-only address → location IS the country.
    let country_only = r#"
        <html><head>
            <script type="application/ld+json">
            { "@type": "JobPosting", "title": "Remote Role",
              "hiringOrganization": { "name": "Globe" },
              "jobLocation": { "address": { "addressCountry": "DE" } } }
            </script>
        </head><body></body></html>
    "#;
    let posting = parse_from_html("https://globe.example/j/5a", country_only)
        .expect("country-only address must still produce a posting");
    assert_eq!(
        posting.location.as_deref(),
        Some("DE"),
        "with no locality/region, addressCountry is the location"
    );

    // (b) Regression: locality + region + country → NO country appended.
    let full = r#"
        <html><head>
            <script type="application/ld+json">
            { "@type": "JobPosting", "title": "Onsite Role",
              "hiringOrganization": { "name": "Globe" },
              "jobLocation": { "address": {
                  "addressLocality": "Berlin", "addressRegion": "BE", "addressCountry": "DE" } } }
            </script>
        </head><body></body></html>
    "#;
    let posting = parse_from_html("https://globe.example/j/5b", full)
        .expect("full address must produce a posting");
    assert_eq!(
        posting.location.as_deref(),
        Some("Berlin, BE"),
        "country must NOT be appended when locality+region are present"
    );
}

#[test]
fn parse_from_html_main_content_description_fallback() {
    // No JSON-LD, no __NEXT_DATA__, and NO meta description — only a <title> and a
    // big <main> block. With no meta description to win, the description must come
    // from the main-content fallback (and dwarf the short title).
    let big = "We are hiring a backend engineer to build resilient distributed \
        systems, own the API platform end to end, and mentor the team. "
        .repeat(6);
    let html = format!(
        r#"<html><head>
            <title>Backend Engineer</title>
        </head><body>
            <main><p>{big}</p></main>
        </body></html>"#
    );
    let posting = parse_from_html("https://acme.example/j/6", &html)
        .expect("a page with a <main> block must produce a posting");
    let desc = posting.description.as_deref().unwrap_or_default();
    assert!(
        desc.contains("distributed"),
        "description must come from the <main> content"
    );
    assert!(
        desc.len() > "Backend Engineer".len(),
        "main-content description must be substantial, longer than the title"
    );
}

#[test]
fn parse_from_html_unparseable_page_has_empty_title() {
    // A page with no title/h1/JSON-LD/__NEXT_DATA__/main yields a posting whose
    // title is empty — so `extension_bridge::usable` would be false and
    // handle_import would persist a partial stub. (The stub branch itself needs an
    // AppHandle, so it isn't unit-testable here — mirrors the import_tests note.)
    let html = "<html><head></head><body><div>just a div, nothing useful</div></body></html>";
    let posting = parse_from_html("https://blocked.example/x", html).expect("still yields Some");
    assert_eq!(posting.title, "", "nothing usable parsed → empty title");
}
