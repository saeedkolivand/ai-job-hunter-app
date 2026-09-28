//! Hardened JSON-LD (bare array / deeply-nested `@graph`), `__NEXT_DATA__`,
//! multi-location, and Readability/main-content fallback tests for
//! `parse_from_html`.

use super::super::generic::readability_content_text;
use super::super::*;

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
fn parse_from_html_readability_skips_in_article_decoy_block() {
    // No JSON-LD, no __NEXT_DATA__, no meta description, no extension hint —
    // the whole-document last-resort chain runs. The decoy block below is
    // nested INSIDE the <article> alongside the real job paragraphs — NOT a
    // <nav>/<footer> sibling — so only Readability's own per-node scoring
    // (its `UNLIKELY_CANDIDATES` class-pattern strip, matched here by
    // `class="related sponsor"`) can drop it. `main_content_text`'s selector
    // (`main, [role="main"], article`) would keep the WHOLE <article>
    // innerHTML verbatim, decoy included — so this is the case that actually
    // fails if `readability_content_text` were never wired in.
    let html = r#"<html><head>
        <title>Senior Backend Engineer at Acme</title>
    </head><body>
        <article>
            <h1>Senior Backend Engineer</h1>
            <p>We are looking for a Senior Backend Engineer to join our platform
            team and help us scale the systems that power millions of requests
            every day across our global user base. You will design and build
            resilient distributed systems, own the API platform end to end, and
            mentor junior engineers on best practices for scalable, reliable
            architecture and long-term maintainability.</p>
            <div class="related sponsor">
                <p>Related Jobs: DevOps Engineer, Site Reliability Engineer,
                Platform Engineer. Sponsored listings from our partner network.</p>
            </div>
            <p>Responsibilities include architecting microservices,
            collaborating closely with product and design teams on new
            features, and maintaining our CI/CD pipeline for rapid, safe
            deployments across every environment we run in production today,
            including our growing multi-region rollout.</p>
        </article>
    </body></html>"#;
    let posting = parse_from_html("https://acme.example/j/readability-1", html)
        .expect("a page with a real article body must produce a posting");
    let desc = posting.description.as_deref().unwrap_or_default();
    assert!(
        desc.contains("resilient distributed systems")
            && desc.contains("architecting microservices"),
        "description must contain the real job body on both sides of the decoy, got: {desc}"
    );
    assert!(
        !desc.contains("Related Jobs") && !desc.contains("Sponsored listings"),
        "description must NOT contain the in-article decoy block, got: {desc}"
    );
}

#[test]
fn readability_content_text_returns_none_for_sparse_page() {
    // A page with too little real content to trust: `is_probably_readable()`
    // (dom_smoothie's own pre-check, default `min_content_length` 140 chars)
    // must gate it out BEFORE `parse()` ever runs, so `readability_content_text`
    // returns `None` — letting `parse_from_html`'s `.or_else(|| main_content_text(html))`
    // fall through cleanly instead of readability guessing at a too-thin document.
    let html = r#"<html><body><main><p>Short posting.</p></main></body></html>"#;
    let result = readability_content_text("https://acme.example/j/sparse", html);
    assert_eq!(
        result, None,
        "a too-thin document must fail the is_probably_readable() gate and yield None, got: {result:?}"
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
