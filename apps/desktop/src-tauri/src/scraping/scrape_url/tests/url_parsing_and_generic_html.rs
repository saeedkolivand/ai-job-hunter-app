//! `resolve()`'s SSRF-reject path, `parse_greenhouse_url`/`parse_lever_url`
//! URL-shape parsing, and the generic `<title>`/meta-description +
//! employer-name (`parse_generic_html`/`parse_generic_company`) parsing.

use super::super::generic::{parse_generic_company, parse_generic_html};
use super::super::greenhouse::parse_greenhouse_url;
use super::super::lever::parse_lever_url;

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

/// Table-driven, merges `test_parse_greenhouse_url_{standard, embed, invalid,
/// subdomain, with_trailing_slash, embed_missing_token, embed_missing_for,
/// invalid_domain, single_segment, with_query}` (10 cases; `subdomain` was a
/// byte-identical duplicate of `standard` — collapsed, not dropped).
#[test]
fn parse_greenhouse_url_cases() {
    type Case = (
        &'static str,
        &'static str,
        Option<(&'static str, &'static str)>,
    );
    let cases: &[Case] = &[
        (
            "standard",
            "https://boards.greenhouse.io/stripe/jobs/12345",
            Some(("stripe", "12345")),
        ),
        (
            "embed",
            "https://boards.greenhouse.io/embed/job_app?for=stripe&token=abc123",
            Some(("stripe", "abc123")),
        ),
        ("invalid", "https://example.com/jobs/123", None),
        // reqwest::Url splits "/stripe/jobs/12345/" into segments ["stripe","jobs","12345",""]
        // — the trailing empty label must be ignored; extraction must still yield the
        // correct (company, job_id) pair using indices 0 and 2 of the segments vec.
        (
            "with_trailing_slash",
            "https://boards.greenhouse.io/stripe/jobs/12345/",
            Some(("stripe", "12345")),
        ),
        (
            "embed_missing_token",
            "https://boards.greenhouse.io/embed/job_app?for=stripe",
            None,
        ),
        (
            "embed_missing_for",
            "https://boards.greenhouse.io/embed/job_app?token=abc123",
            None,
        ),
        (
            "invalid_domain",
            "https://example.com/stripe/jobs/12345",
            None,
        ),
        (
            "single_segment",
            "https://boards.greenhouse.io/stripe",
            None,
        ),
        (
            "with_query",
            "https://boards.greenhouse.io/stripe/jobs/12345?ref=source",
            Some(("stripe", "12345")),
        ),
    ];
    for (label, url, expected) in cases {
        let result = parse_greenhouse_url(url);
        assert_eq!(
            result,
            expected.map(|(c, j)| (c.to_string(), j.to_string())),
            "case {label}: {url}"
        );
    }
}

/// Table-driven, merges `test_parse_lever_url{_invalid, _with_subdomain,
/// _with_extra_segments, _invalid_domain, _single_segment, _with_query}` (6
/// cases; `with_subdomain` duplicated the bare `url` case and `invalid_domain`
/// duplicated `invalid` — collapsed, not dropped).
#[test]
fn parse_lever_url_cases() {
    type Case = (
        &'static str,
        &'static str,
        Option<(&'static str, &'static str)>,
        &'static str,
    );
    let cases: &[Case] = &[
        (
            "url",
            "https://jobs.lever.co/stripe/abc123",
            Some(("stripe", "abc123")),
            "",
        ),
        ("invalid", "https://example.com/stripe/abc123", None, ""),
        (
            "with_extra_segments",
            "https://jobs.lever.co/stripe/abc123/extra",
            Some(("stripe", "abc123")),
            "extra trailing segment must be ignored; first two segments must be returned",
        ),
        ("single_segment", "https://jobs.lever.co/stripe", None, ""),
        (
            "with_query",
            "https://jobs.lever.co/stripe/abc123?ref=source",
            Some(("stripe", "abc123")),
            "",
        ),
    ];
    for (label, url, expected, extra_msg) in cases {
        let result = parse_lever_url(url);
        let expected = expected.map(|(c, j)| (c.to_string(), j.to_string()));
        if extra_msg.is_empty() {
            assert_eq!(result, expected, "case {label}: {url}");
        } else {
            assert_eq!(result, expected, "{extra_msg}");
        }
    }
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
