use crate::scraping::scrape_url::parse_from_html;

// ─────────────────────────────────────────────────────────────────────────────
// A1. Scan-mode parse goldens — JSON-LD path (LinkedIn-style)
// ─────────────────────────────────────────────────────────────────────────────

/// LinkedIn serves a fully-hydrated JSON-LD `JobPosting` block. The extension
/// captures the authenticated DOM; `parse_from_html` must extract all four
/// fields from that block and NOT fall back to the generic meta path.
#[test]
fn scan_mode_linkedin_style_json_ld_extracts_all_fields() {
    // Minimal LinkedIn-shaped HTML: title in JSON-LD wins over <title>,
    // hiringOrganization supplies company, jobLocation supplies location.
    let html = r#"
        <html>
        <head>
            <title>LinkedIn | Jobs</title>
            <script type="application/ld+json">
            {
                "@context": "https://schema.org/",
                "@type": "JobPosting",
                "title": "Senior Software Engineer",
                "description": "<p>Build distributed systems at scale.</p>",
                "hiringOrganization": {
                    "@type": "Organization",
                    "name": "Acme Corp",
                    "sameAs": "https://www.acmecorp.example"
                },
                "jobLocation": {
                    "@type": "Place",
                    "address": {
                        "@type": "PostalAddress",
                        "addressLocality": "Berlin",
                        "addressRegion": "BE",
                        "addressCountry": "DE"
                    }
                },
                "datePosted": "2026-01-15",
                "employmentType": "FULL_TIME"
            }
            </script>
        </head>
        <body>
            <main>
                <h1 class="job-title">Senior Software Engineer</h1>
                <div class="job-description">Build distributed systems at scale.</div>
            </main>
        </body>
        </html>
    "#;

    let posting = parse_from_html("https://www.linkedin.com/jobs/view/9876543210", html)
        .expect("a valid JSON-LD JobPosting must produce Some");

    assert_eq!(
        posting.title, "Senior Software Engineer",
        "title must come from JSON-LD, not the generic <title> tag"
    );
    assert_eq!(
        posting.company, "Acme Corp",
        "company must be extracted from hiringOrganization.name"
    );
    assert_eq!(
        posting.location.as_deref(),
        Some("Berlin, BE"),
        "location must be assembled from addressLocality + addressRegion"
    );
    assert!(
        posting
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("distributed systems"),
        "description must carry the JSON-LD text (HTML-stripped or raw)"
    );
    assert_eq!(
        posting.url, "https://www.linkedin.com/jobs/view/9876543210",
        "url must be the input URL passed to parse_from_html"
    );
}

/// LinkedIn sometimes wraps the `JobPosting` node inside an `@graph` array
/// alongside `BreadcrumbList` and `WebPage` nodes. The parser must reach into
/// the graph and pull the correct node.
#[test]
fn scan_mode_linkedin_json_ld_graph_array_extracts_job_posting_node() {
    let html = r#"
        <html>
        <head>
            <script type="application/ld+json">
            {
                "@context": "https://schema.org",
                "@graph": [
                    { "@type": "WebPage", "url": "https://www.linkedin.com/jobs/view/42" },
                    { "@type": "BreadcrumbList" },
                    {
                        "@type": "JobPosting",
                        "title": "Staff Infrastructure Engineer",
                        "hiringOrganization": { "name": "Initech" },
                        "jobLocation": {
                            "address": {
                                "addressLocality": "Munich",
                                "addressRegion": "BY"
                            }
                        }
                    }
                ]
            }
            </script>
        </head>
        <body></body>
        </html>
    "#;

    let posting = parse_from_html("https://www.linkedin.com/jobs/view/42", html)
        .expect("@graph-wrapped JobPosting must produce Some");

    assert_eq!(posting.title, "Staff Infrastructure Engineer");
    assert_eq!(posting.company, "Initech");
    assert_eq!(posting.location.as_deref(), Some("Munich, BY"));
}

// ─────────────────────────────────────────────────────────────────────────────
// A2. Scan-mode parse goldens — generic meta path (Indeed / Workday style)
// ─────────────────────────────────────────────────────────────────────────────

/// Indeed (and many ATS job pages) render job details in `<title>`, `<h1>`,
/// `og:description`, and `og:site_name` but have NO JSON-LD block. The parser
/// must fall back to the generic meta path and extract meaningful fields.
#[test]
fn scan_mode_indeed_style_generic_meta_extracts_fields() {
    let html = r#"
        <html>
        <head>
            <title>Backend Engineer - Globex | Indeed.com</title>
            <meta property="og:site_name" content="Globex">
            <meta property="og:description"
                  content="Own the API platform. Remote-friendly, great team.">
        </head>
        <body>
            <h1 class="jobsearch-JobInfoHeader-title">Backend Engineer</h1>
        </body>
        </html>
    "#;

    let posting = parse_from_html("https://www.indeed.com/viewjob?jk=abc123def456", html)
        .expect("generic meta path must produce Some when <h1> or <title> is present");

    // The title selector prefers <h1> inside body if one exists.
    assert!(
        !posting.title.is_empty(),
        "title must not be empty when <h1> or <title> exists"
    );
    assert_eq!(
        posting.company, "Globex",
        "company must come from og:site_name when no JSON-LD is present"
    );
    assert!(
        posting.description.as_deref().unwrap_or_default().len() > 5,
        "description must carry the og:description content"
    );
    assert_eq!(posting.source, "url");
}

/// Workday and similar ATSes often have a `name="description"` meta but no
/// `og:*` tags and no JSON-LD. The parser must still produce a posting with
/// the plain `name="description"` value.
#[test]
fn scan_mode_workday_style_plain_meta_description_used_as_fallback() {
    let html = r#"
        <html>
        <head>
            <title>Data Analyst</title>
            <meta name="description" content="Analyze business intelligence data at Umbrella Corp.">
        </head>
        <body></body>
        </html>
    "#;

    let posting = parse_from_html("https://umbrella.wd1.myworkdayjobs.com/jobs/42", html)
        .expect("plain-meta path must produce Some when <title> is present");

    assert_eq!(posting.title, "Data Analyst");
    assert!(
        posting
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("business intelligence"),
        "description must carry the name=description content"
    );
    // No og:site_name and no JSON-LD → company falls back to empty string (not a panic).
    assert_eq!(posting.source, "url");
}
