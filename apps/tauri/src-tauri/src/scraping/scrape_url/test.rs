use super::*;

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
    let html = r#"
        <html>
            <head><title>Page Title</title></head>
            <body><h1>Job Title</h1></body>
        </html>
    "#;
    let (title, _description) = parse_generic_html(html);
    // title selector matches both title and h1, first match wins
    assert!(!title.is_empty());
}

#[test]
fn test_parse_greenhouse_url_subdomain() {
    let url = "https://boards.greenhouse.io/stripe/jobs/12345";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "12345".to_string())));
}

#[test]
fn test_parse_greenhouse_url_with_trailing_slash() {
    let url = "https://boards.greenhouse.io/stripe/jobs/12345/";
    let result = parse_greenhouse_url(url);
    // Trailing slash may affect path segments
    assert!(result.is_some());
}

#[test]
fn test_parse_lever_url_with_subdomain() {
    let url = "https://jobs.lever.co/stripe/abc123";
    let result = parse_lever_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "abc123".to_string())));
}

#[test]
fn test_parse_lever_url_with_extra_segments() {
    let url = "https://jobs.lever.co/stripe/abc123/extra";
    let result = parse_lever_url(url);
    // Should still extract first two segments
    assert!(result.is_some());
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
