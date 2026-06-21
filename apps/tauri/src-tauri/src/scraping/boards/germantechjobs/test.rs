use super::*;

// ── identity / mode ──────────────────────────────────────────────────────────

#[test]
fn test_german_tech_jobs_scraper_id() {
    let scraper = GermanTechJobsScraper;
    assert_eq!(scraper.id(), "germantechjobs");
}

#[test]
fn test_german_tech_jobs_scraper_display_name() {
    let scraper = GermanTechJobsScraper;
    assert_eq!(scraper.display_name(), "German Tech Jobs");
}

#[test]
fn test_german_tech_jobs_scraper_mode() {
    // Replaced the tautological ScraperMode::Http == ScraperMode::Http assertion
    // with a meaningful GTJ-specific mode check.
    let scraper = GermanTechJobsScraper;
    assert_ne!(
        scraper.mode(),
        ScraperMode::Browser,
        "germantechjobs must be Http, not Browser"
    );
}

// ── CDATA unwrapping ─────────────────────────────────────────────────────────

#[test]
fn test_unwrap_cdata_strips_wrapper() {
    assert_eq!(unwrap_cdata("<![CDATA[Hello World]]>"), "Hello World");
}

#[test]
fn test_unwrap_cdata_passthrough_when_no_wrapper() {
    assert_eq!(unwrap_cdata("plain text"), "plain text");
}

#[test]
fn test_unwrap_cdata_trims_inner_whitespace() {
    assert_eq!(unwrap_cdata("<![CDATA[  trimmed  ]]>"), "trimmed");
}

// ── XML parser — inline fixture ──────────────────────────────────────────────
//
// Two jobs:
//   job1 — has <company>, <location>, <url>, DD.MM.YYYY pubdate, <title>
//   job2 — no <company> (must fall back to <company-name>), no <location>
//           (falls back to <city>+<region>), no <url> (falls back to <link>),
//           no <pubdate> (posted_at must be None), <name> instead of <title>

const FIXTURE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<jobs>
<job id="abc123" category="IT">
  <id><![CDATA[abc123]]></id>
  <title><![CDATA[Senior Rust Engineer]]></title>
  <name><![CDATA[Senior Rust Engineer]]></name>
  <link><![CDATA[https://germantechjobs.de/jobs/acme-senior-rust-engineer]]></link>
  <url><![CDATA[https://germantechjobs.de/jobs/acme-senior-rust-engineer]]></url>
  <location><![CDATA[Karl-Marx-Allee 1, Berlin]]></location>
  <city><![CDATA[Berlin]]></city>
  <region><![CDATA[Berlin]]></region>
  <company><![CDATA[Acme GmbH]]></company>
  <company-name><![CDATA[Acme GmbH]]></company-name>
  <salary><![CDATA[80.000 - 120.000 € per year]]></salary>
  <pubdate><![CDATA[15.03.2024]]></pubdate>
  <description><![CDATA[<p>Great <b>Rust</b> role.</p>]]></description>
</job>
<job id="def456" category="Dev">
  <id><![CDATA[def456]]></id>
  <name><![CDATA[Backend Developer]]></name>
  <link><![CDATA[https://germantechjobs.de/jobs/beta-backend-developer]]></link>
  <city><![CDATA[Munich]]></city>
  <region><![CDATA[Bayern]]></region>
  <company-name><![CDATA[Beta Corp]]></company-name>
  <description><![CDATA[<p>Node.js backend position.</p>]]></description>
</job>
</jobs>"#;

fn fixture_jobs() -> Vec<JobPosting> {
    let now = chrono::Utc::now().timestamp_millis();
    parse_feed(FIXTURE_XML, "germantechjobs", now)
}

#[test]
fn test_parse_fixture_two_jobs() {
    let jobs = fixture_jobs();
    assert_eq!(jobs.len(), 2, "expected 2 parsed jobs, got {}", jobs.len());
}

#[test]
fn test_parse_fixture_job1_fields() {
    let jobs = fixture_jobs();
    let j = jobs.first().expect("job1 must exist");

    assert_eq!(j.id, "germantechjobs:abc123");
    assert_eq!(j.external_id.as_deref(), Some("abc123"));
    assert_eq!(j.title, "Senior Rust Engineer");
    assert_eq!(j.company, "Acme GmbH");
    // <location> tag present — must be used directly
    assert_eq!(j.location.as_deref(), Some("Karl-Marx-Allee 1, Berlin"));
    assert_eq!(
        j.url,
        "https://germantechjobs.de/jobs/acme-senior-rust-engineer"
    );
    // pubdate: 15.03.2024 → midnight UTC millis
    let expected_ms = chrono::NaiveDate::from_ymd_opt(2024, 3, 15)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();
    assert_eq!(j.posted_at, Some(expected_ms));
    // description: HTML stripped
    let desc = j.description.as_deref().unwrap_or("");
    assert!(
        desc.contains("Rust"),
        "description should contain 'Rust', got: {desc:?}"
    );
    assert!(
        !desc.contains("<p>") && !desc.contains("<b>"),
        "description still contains HTML tags: {desc:?}"
    );
}

#[test]
fn test_parse_fixture_job2_fallbacks() {
    let jobs = fixture_jobs();
    let j = jobs.get(1).expect("job2 must exist");

    assert_eq!(j.id, "germantechjobs:def456");
    // <title> absent → falls back to <name>
    assert_eq!(j.title, "Backend Developer");
    // <company> absent → falls back to <company-name>
    assert_eq!(j.company, "Beta Corp");
    // <location> absent, <city> + <region> both present → "Munich, Bayern"
    assert_eq!(j.location.as_deref(), Some("Munich, Bayern"));
    // <url> absent → falls back to <link>
    assert_eq!(
        j.url,
        "https://germantechjobs.de/jobs/beta-backend-developer"
    );
    // <pubdate> absent → None
    assert_eq!(j.posted_at, None);
}

#[test]
fn test_parse_fixture_language_key_is_de() {
    let jobs = fixture_jobs();
    let j = jobs.first().expect("job1 must exist");
    assert_eq!(j.extra.get("language").and_then(|v| v.as_str()), Some("de"));
}

#[test]
fn test_cap_missing_tag_returns_empty() {
    // cap() over a block that does not contain the queried tag returns "".
    let block = "<title><![CDATA[My Title]]></title>";
    assert_eq!(cap(&COMPANY_RE, block), "");
}

#[test]
fn test_cap_hyphenated_tag() {
    let block = "<company-name><![CDATA[Hyphen Corp]]></company-name>";
    assert_eq!(cap(&COMPANY_NAME_RE, block), "Hyphen Corp");
}

// ── pubdate edge cases ────────────────────────────────────────────────────────

#[test]
fn test_bad_pubdate_yields_posted_at_none() {
    // A full <job> block with a malformed pubdate must produce a posting with
    // posted_at: None — not a panic, not an epoch, not skipped.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<jobs>
<job>
  <id>bad-date-job</id>
  <title>Bad Date Job</title>
  <url>https://germantechjobs.de/jobs/bad-date-job</url>
  <company>Test Corp</company>
  <pubdate>not-a-date</pubdate>
</job>
</jobs>"#;
    let now = chrono::Utc::now().timestamp_millis();
    let jobs = parse_feed(xml, "germantechjobs", now);
    assert_eq!(jobs.len(), 1, "job with bad pubdate should still parse");
    assert_eq!(
        jobs[0].posted_at, None,
        "bad pubdate must yield None, not epoch or panic"
    );
}

// ── client-side keyword / location filter ────────────────────────────────────

const FILTER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<jobs>
<job>
  <id>job-rust</id>
  <title>Rust Backend Engineer</title>
  <url>https://germantechjobs.de/jobs/rust-backend</url>
  <company>Alpha AG</company>
  <location>Berlin</location>
</job>
<job>
  <id>job-java</id>
  <title>Java Frontend Developer</title>
  <url>https://germantechjobs.de/jobs/java-frontend</url>
  <company>Beta GmbH</company>
  <location>Hamburg</location>
</job>
</jobs>"#;

#[test]
fn test_keyword_filter_excludes_non_matching_job() {
    // Only "rust" jobs should remain after parsing; "java" job must be excluded.
    let now = chrono::Utc::now().timestamp_millis();
    let all = parse_feed(FILTER_XML, "germantechjobs", now);
    assert_eq!(all.len(), 2, "fixture must parse 2 jobs before filtering");

    let q = "rust";
    let filtered: Vec<_> = all
        .into_iter()
        .filter(|p| {
            let haystack = format!(
                "{} {} {} {}",
                p.title,
                p.company,
                p.location.as_deref().unwrap_or(""),
                p.description.as_deref().unwrap_or("")
            )
            .to_lowercase();
            haystack.contains(q)
        })
        .collect();

    assert_eq!(
        filtered.len(),
        1,
        "only the Rust job should pass the filter"
    );
    assert_eq!(filtered[0].external_id.as_deref(), Some("job-rust"));
}

#[test]
fn test_location_filter_matches_on_location_field() {
    // A job matching only on the location field (not in title/company/description)
    // must still be included when filtering by that location.
    let now = chrono::Utc::now().timestamp_millis();
    let all = parse_feed(FILTER_XML, "germantechjobs", now);

    let loc = "hamburg";
    let filtered: Vec<_> = all
        .into_iter()
        .filter(|p| {
            let haystack = format!(
                "{} {} {} {}",
                p.title,
                p.company,
                p.location.as_deref().unwrap_or(""),
                p.description.as_deref().unwrap_or("")
            )
            .to_lowercase();
            haystack.contains(loc)
        })
        .collect();

    assert_eq!(
        filtered.len(),
        1,
        "only the Hamburg job should pass the location filter"
    );
    assert_eq!(filtered[0].external_id.as_deref(), Some("job-java"));
}

// ── continue-path (guard-miss) coverage ──────────────────────────────────────

/// A <job> block with NO <id> AND NO <link> must be dropped (id-fallback miss).
/// A sibling valid job in the same fixture must still appear in the output.
#[test]
fn test_parse_drops_job_with_no_id_and_no_link() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<jobs>
<job>
  <title>No ID Job</title>
  <url>https://germantechjobs.de/jobs/no-id-job</url>
  <company>Ghost Corp</company>
</job>
<job>
  <id>valid-job-1</id>
  <title>Valid Job</title>
  <url>https://germantechjobs.de/jobs/valid-job</url>
  <company>Real Corp</company>
</job>
</jobs>"#;
    let now = chrono::Utc::now().timestamp_millis();
    let jobs = parse_feed(xml, "germantechjobs", now);
    assert_eq!(
        jobs.len(),
        1,
        "job with no <id> and no <link> must be dropped; only the valid sibling must remain"
    );
    assert_eq!(
        jobs[0].external_id.as_deref(),
        Some("valid-job-1"),
        "the surviving job must be the one with a usable id"
    );
}

/// A <job> block with NO <url>, <link>, or <apply_url> must be dropped (url-fallback miss).
/// A sibling valid job in the same fixture must still appear in the output.
#[test]
fn test_parse_drops_job_with_no_url_fields() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<jobs>
<job>
  <id>no-url-job</id>
  <title>No URL Job</title>
  <company>Invisible Corp</company>
</job>
<job>
  <id>valid-job-2</id>
  <title>Has URL Job</title>
  <url>https://germantechjobs.de/jobs/has-url-job</url>
  <company>Visible Corp</company>
</job>
</jobs>"#;
    let now = chrono::Utc::now().timestamp_millis();
    let jobs = parse_feed(xml, "germantechjobs", now);
    assert_eq!(
        jobs.len(),
        1,
        "job with no <url>, <link>, or <apply_url> must be dropped; only the valid sibling must remain"
    );
    assert_eq!(
        jobs[0].external_id.as_deref(),
        Some("valid-job-2"),
        "the surviving job must be the one with a usable url"
    );
}

// ── live network (ignored in CI) ─────────────────────────────────────────────

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = GermanTechJobsScraper;
    let input = BoardSearchInput {
        query: "".to_string(),
        location: None,
        amount: 20,
        pages: 1,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        locale: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: Vec::new(),
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
    };
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        scraper.search(input, ctx),
    )
    .await
    .expect("live search timed out");
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    let first = postings.first().expect("first posting");
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.company.is_empty(), "first posting has empty company");
    assert!(!first.url.is_empty(), "first posting has empty url");
    assert!(
        first.location.is_some(),
        "first posting has no location — feed changed?"
    );
    println!("germantechjobs: {} results", postings.len());
    println!("first: {:?}", first.title);
}
