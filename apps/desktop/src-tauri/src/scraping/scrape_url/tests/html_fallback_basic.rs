//! `parse_from_html`'s Scan-mode fetch-free basics, the #1239 title-branding
//! + logo-alt company-name fixes, and #1238's embedded-ATS-iframe detection.

use super::super::generic::{parse_generic_company, strip_site_suffix};
use super::super::*;

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

// ── #1239: generic-fallback title branding + company from a logo alt ───────
// Scan-mode Import parses the extension's captured DOM through the generic
// path, which kept LinkedIn's whole `<title>` (site branding and all) and fell
// back to the URL host as the employer.

#[test]
fn strip_site_suffix_drops_only_a_real_site_name() {
    // The #1239 repro: the trailing segment IS the host's own label.
    assert_eq!(
        strip_site_suffix(
            "Javascript Developer | Digital Waffle | LinkedIn",
            None,
            "www.linkedin.com"
        ),
        "Javascript Developer | Digital Waffle"
    );
    // og:site_name is honoured even when it does not match the host.
    assert_eq!(
        strip_site_suffix(
            "Staff Engineer - Acme Careers",
            Some("Acme Careers"),
            "jobs.example.com"
        ),
        "Staff Engineer"
    );
}

#[test]
fn strip_site_suffix_keeps_titles_that_merely_contain_a_separator() {
    // A pipe inside a real job title must survive — this is what a
    // "drop everything after the last separator" rule would destroy.
    assert_eq!(
        strip_site_suffix("Engineer | Payments", None, "www.linkedin.com"),
        "Engineer | Payments"
    );
    // An internal dash, likewise.
    assert_eq!(
        strip_site_suffix("Full-Stack Developer", None, "www.linkedin.com"),
        "Full-Stack Developer"
    );
    assert_eq!(
        strip_site_suffix("Senior Engineer - Platform", None, "www.linkedin.com"),
        "Senior Engineer - Platform"
    );
    // Never strip the whole title, even when it IS just the site name.
    assert_eq!(
        strip_site_suffix("LinkedIn", None, "www.linkedin.com"),
        "LinkedIn"
    );
}

#[test]
fn parse_generic_company_reads_a_logo_alt_when_nothing_else_names_the_employer() {
    let html = r#"<html><body>
        <img alt="Company logo for, Digital Waffle" src="/x.png">
        <h1>Javascript Developer</h1>
    </body></html>"#;
    assert_eq!(
        parse_generic_company(html).as_deref(),
        Some("Digital Waffle")
    );
}

#[test]
fn parse_generic_company_prefers_json_ld_over_the_logo_heuristic() {
    let html = r#"<html><head>
        <script type="application/ld+json">
        {"@type":"JobPosting","hiringOrganization":{"name":"Real Employer Ltd"}}
        </script>
        </head><body><img alt="Company logo for, Wrong Name" src="/x.png"></body></html>"#;
    assert_eq!(
        parse_generic_company(html).as_deref(),
        Some("Real Employer Ltd")
    );
}

#[test]
fn parse_generic_company_stays_none_on_a_page_with_no_employer_signal() {
    // The caller's host fallback must still be reachable — a page with ordinary
    // images must not be mined for a fake employer name.
    let html = r#"<html><body><img alt="A photo of the office" src="/x.png"></body></html>"#;
    assert_eq!(parse_generic_company(html), None);
}

#[test]
fn parse_from_html_cleans_a_linkedin_shaped_capture_end_to_end() {
    let html = r#"<html><head><title>Javascript Developer | Digital Waffle | LinkedIn</title></head>
        <body><img alt="Company logo for, Digital Waffle" src="/x.png">
        <main><p>We are hiring a javascript developer.</p></main></body></html>"#;
    let posting = parse_from_html("https://www.linkedin.com/jobs/view/123", html)
        .expect("a parseable document");
    assert_eq!(posting.title, "Javascript Developer | Digital Waffle");
    assert_eq!(posting.company, "Digital Waffle");
}

// ── #1238: an ATS board embedded in a cross-origin iframe ─────────────
// Keyed on RECOGNISING the board, not on "is this frame cross-origin" — the
// latter is true of the analytics, consent and video frames on nearly every
// page. The fixtures below are the real shapes seen live.

#[test]
fn embeds_ats_board_spots_a_real_embedded_careers_page() {
    // The exact frame set captured from a live company careers page: the Ashby
    // board alongside an ordinary consent frame.
    let html = r#"<html><body>
        <iframe src="https://jobs.ashbyhq.com/happyhotel?embed=js"></iframe>
        <iframe src="https://consentcdn.cookiebot.com/sdk/bc-v4.min.html"></iframe>
        <h1>Jobs &amp; Karriere</h1><p>Lots of careers-page prose.</p>
    </body></html>"#;
    assert!(embeds_ats_board(html, "https://www.happyhotel.io/karriere"));
}

#[test]
fn embeds_ats_board_spots_the_other_common_boards() {
    for src in [
        "https://boards.greenhouse.io/acme",
        "https://job-boards.greenhouse.io/acme",
        "https://jobs.lever.co/acme",
        "https://jobs.smartrecruiters.com/acme",
    ] {
        let html = format!(r#"<html><body><iframe src="{src}"></iframe></body></html>"#);
        assert!(
            embeds_ats_board(&html, "https://careers.example.com/jobs"),
            "{src} must be recognised as an embedded board"
        );
    }
}

#[test]
fn embeds_ats_board_ignores_ordinary_third_party_frames() {
    let page = "https://careers.example.com/jobs/1";
    // The frames that sit on nearly every page — the reason a bare
    // cross-origin test is useless as a signal.
    for src in [
        "https://consentcdn.cookiebot.com/sdk/bc-v4.min.html",
        "https://www.youtube.com/embed/abc123",
        "https://www.googletagmanager.com/ns.html?id=GTM-XYZ",
        "/widget.html",
        "about:blank",
    ] {
        let html = format!(r#"<html><body><iframe src="{src}"></iframe></body></html>"#);
        assert!(
            !embeds_ats_board(&html, page),
            "{src} must NOT be treated as an embedded board"
        );
    }
    // No frames at all.
    assert!(!embeds_ats_board(
        r#"<html><body><p>Just a posting.</p></body></html>"#,
        page
    ));
}

#[test]
fn embeds_ats_board_ignores_a_board_embedding_its_own_host() {
    // An ATS page that frames itself is the posting, not a wrapper around one.
    let html =
        r#"<html><body><iframe src="https://jobs.ashbyhq.com/acme/embed"></iframe></body></html>"#;
    assert!(!embeds_ats_board(
        html,
        "https://jobs.ashbyhq.com/acme/some-job"
    ));
}

#[test]
fn embeds_ats_board_still_warns_when_the_page_url_is_unparseable() {
    // With no parseable page url the same-origin check cannot run, but a
    // recognised board frame is still the signal. The safe direction for this
    // feature is to WARN: a needless "couldn't read the description" costs the
    // user a second look, while staying silent writes a wrong record.
    let html = r#"<html><body><iframe src="https://jobs.ashbyhq.com/x"></iframe></body></html>"#;
    assert!(embeds_ats_board(html, "not a url"));
}
