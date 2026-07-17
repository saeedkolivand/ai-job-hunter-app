use super::*;
// Not re-exported from `mod.rs` (no override there — see fix below), imported
// directly here just for this one assertion.
use crate::scraping::types::AuthRequirement;

// ── identity / mode / auth ───────────────────────────────────────────────────

#[test]
fn test_jobicy_scraper_id() {
    let scraper = JobicyScraper;
    assert_eq!(scraper.id(), "jobicy");
}

#[test]
fn test_jobicy_scraper_display_name() {
    let scraper = JobicyScraper;
    assert_eq!(scraper.display_name(), "Jobicy");
}

#[test]
fn test_jobicy_scraper_mode() {
    let scraper = JobicyScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_jobicy_scraper_auth_is_guest() {
    // Keyless API — no session/login ever required. No explicit `auth()`
    // override in `mod.rs` (it would only duplicate the trait default); this
    // guards that default against a future accidental regression.
    assert_eq!(JobicyScraper.auth(), AuthRequirement::Guest);
}

// ── JSON fixture parse ───────────────────────────────────────────────────────
//
// Shaped after a live `GET /api/v2/remote-jobs` response (verified 2026-07-17):
// job1 has a full HTML `jobDescription`; job2 omits it (must fall back to the
// HTML `jobExcerpt`); job3 has no `id` (must be dropped, sibling still parses).

const FIXTURE_JSON: &str = r#"{
  "apiVersion": "2.2.14",
  "jobCount": 3,
  "friendlyNotice": "Thanks for using Jobicy API! Please ensure Jobicy is clearly credited...",
  "jobs": [
    {
      "id": 146131,
      "url": "https://jobicy.com/jobs/146131-customer-experience-analyst",
      "jobSlug": "146131-customer-experience-analyst",
      "jobTitle": "Customer Experience Analyst",
      "companyName": "TechMagic",
      "jobIndustry": ["Customer Success"],
      "jobType": ["Full-Time"],
      "jobGeo": "Ukraine",
      "jobLevel": "Midweight",
      "jobExcerpt": "We are looking for a Customer Experience Analyst&hellip;",
      "jobDescription": "<p>We are looking for a <strong>Customer Experience Analyst</strong>.</p><ul><li>3+ years experience</li></ul>",
      "pubDate": "2026-07-17T11:20:05+00:00"
    },
    {
      "id": 146472,
      "url": "https://jobicy.com/jobs/146472-seo-analyst-fully-remote",
      "jobSlug": "146472-seo-analyst-fully-remote",
      "jobTitle": "SEO Analyst (Fully Remote)",
      "companyName": "PadSplit",
      "jobIndustry": ["SEO"],
      "jobType": ["Full-Time"],
      "jobGeo": "USA",
      "jobLevel": "Midweight",
      "jobExcerpt": "<p>Own and scale our organic search strategy&hellip;</p>",
      "pubDate": "2026-07-17T09:45:04+00:00"
    },
    {
      "url": "https://jobicy.com/jobs/no-id-job",
      "jobTitle": "No ID Job",
      "companyName": "Ghost Corp"
    }
  ],
  "success": true
}"#;

fn fixture_postings() -> Vec<JobPosting> {
    let now = chrono::Utc::now().timestamp_millis();
    let resp: Resp = serde_json::from_str(FIXTURE_JSON).expect("fixture must deserialize");
    rows_to_jobs(resp.jobs)
        .into_iter()
        .filter_map(|j| map_job(j, "jobicy", now))
        .collect()
}

#[test]
fn test_parse_fixture_drops_job_with_no_id() {
    let postings = fixture_postings();
    assert_eq!(
        postings.len(),
        2,
        "job3 has no id and must be dropped; job1+job2 must survive"
    );
}

#[test]
fn test_parse_fixture_job1_full_html_description_becomes_markdown() {
    let postings = fixture_postings();
    let j = &postings[0];

    assert_eq!(j.id, "jobicy:146131");
    assert_eq!(j.external_id.as_deref(), Some("146131"));
    assert_eq!(j.title, "Customer Experience Analyst");
    assert_eq!(j.company, "TechMagic");
    assert_eq!(j.location.as_deref(), Some("Ukraine"));
    assert_eq!(
        j.url, "https://jobicy.com/jobs/146131-customer-experience-analyst",
        "url must be the unmodified jobicy.com link (ToS attribution requirement)"
    );
    assert_eq!(j.source, "jobicy");

    let desc = j.description.as_deref().unwrap_or("");
    assert!(
        desc.contains("Customer Experience Analyst"),
        "description should retain the text, got: {desc:?}"
    );
    assert!(
        !desc.contains("<p>") && !desc.contains("<strong>") && !desc.contains("<li>"),
        "description still contains raw HTML tags, got: {desc:?}"
    );
    // htmd renders <li> as a markdown bullet, not a stripped/collapsed line.
    assert!(
        desc.contains('-') || desc.contains('*'),
        "expected a markdown bullet for the <ul><li> block, got: {desc:?}"
    );

    let expected_ms = chrono::DateTime::parse_from_rfc3339("2026-07-17T11:20:05+00:00")
        .unwrap()
        .timestamp_millis();
    assert_eq!(j.posted_at, Some(expected_ms));
    assert_eq!(j.extra.get("remote").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn test_parse_fixture_job2_falls_back_to_excerpt_when_description_missing() {
    let postings = fixture_postings();
    let j = &postings[1];

    assert_eq!(j.id, "jobicy:146472");
    let desc = j.description.as_deref().unwrap_or("");
    assert!(
        desc.contains("Own and scale"),
        "missing jobDescription must fall back to jobExcerpt, got: {desc:?}"
    );
    assert!(
        !desc.contains("<p>"),
        "excerpt fallback must still be HTML-converted, got: {desc:?}"
    );
}

#[test]
fn test_map_job_empty_description_falls_back_to_excerpt() {
    // `jobDescription: ""` (present but blank) must NOT short-circuit `.or` —
    // it must fall back to `jobExcerpt` just like a missing/`None` field.
    let j: Job = serde_json::from_str(
        r#"{
            "id": 1,
            "url": "https://jobicy.com/jobs/1-x",
            "jobTitle": "Some Job",
            "jobDescription": "",
            "jobExcerpt": "<p>Fallback excerpt text</p>"
        }"#,
    )
    .unwrap();
    let posting = map_job(j, "jobicy", 0).expect("id/title/url all present");
    let desc = posting.description.as_deref().unwrap_or("");
    assert!(
        desc.contains("Fallback excerpt text"),
        "blank jobDescription must fall back to jobExcerpt, got: {desc:?}"
    );
}

#[test]
fn test_map_job_blank_description_and_excerpt_yields_none() {
    // Both fields present but blank — no fabricated description.
    let j: Job = serde_json::from_str(
        r#"{
            "id": 1,
            "url": "https://jobicy.com/jobs/1-x",
            "jobTitle": "Some Job",
            "jobDescription": "   ",
            "jobExcerpt": ""
        }"#,
    )
    .unwrap();
    let posting = map_job(j, "jobicy", 0).expect("id/title/url all present");
    assert_eq!(posting.description, None);
}

// ── map_job edge cases ───────────────────────────────────────────────────────

#[test]
fn test_map_job_drops_when_title_blank() {
    let j: Job = serde_json::from_str(
        r#"{"id": 1, "url": "https://jobicy.com/jobs/1-x", "jobTitle": "   "}"#,
    )
    .unwrap();
    assert!(map_job(j, "jobicy", 0).is_none());
}

#[test]
fn test_map_job_drops_when_url_missing() {
    let j: Job = serde_json::from_str(r#"{"id": 1, "jobTitle": "Some Job"}"#).unwrap();
    assert!(map_job(j, "jobicy", 0).is_none());
}

#[test]
fn test_map_job_defaults_company_to_unknown_when_missing() {
    let j: Job = serde_json::from_str(
        r#"{"id": 1, "url": "https://jobicy.com/jobs/1-x", "jobTitle": "Some Job"}"#,
    )
    .unwrap();
    let posting = map_job(j, "jobicy", 0).expect("id/title/url all present");
    assert_eq!(posting.company, "Unknown");
    assert_eq!(posting.location, None);
    assert_eq!(posting.description, None);
    assert_eq!(posting.posted_at, None);
}

#[test]
fn test_map_job_bad_pub_date_yields_posted_at_none() {
    let j: Job = serde_json::from_str(
        r#"{"id": 1, "url": "https://jobicy.com/jobs/1-x", "jobTitle": "Some Job", "pubDate": "not-a-date"}"#,
    )
    .unwrap();
    let posting = map_job(j, "jobicy", 0).expect("id/title/url all present");
    assert_eq!(posting.posted_at, None, "malformed pubDate must not panic");
}

// ── url host validation (security defense-in-depth) ─────────────────────────

#[test]
fn test_is_valid_jobicy_url_accepts_https_jobicy_host() {
    assert!(is_valid_jobicy_url(
        "https://jobicy.com/jobs/146131-customer-experience-analyst"
    ));
}

#[test]
fn test_is_valid_jobicy_url_accepts_http_and_subdomain() {
    assert!(is_valid_jobicy_url("http://jobicy.com/jobs/1-x"));
    assert!(is_valid_jobicy_url("https://www.jobicy.com/jobs/1-x"));
}

#[test]
fn test_is_valid_jobicy_url_rejects_non_jobicy_host() {
    assert!(!is_valid_jobicy_url("https://evil-phish.example/jobs/1-x"));
    // A host that merely CONTAINS "jobicy" as a substring must not pass —
    // matches_domain_list is suffix/label-anchored, not a bare `contains`.
    assert!(!is_valid_jobicy_url("https://jobicy.com.evil.example/x"));
}

#[test]
fn test_is_valid_jobicy_url_rejects_non_http_scheme() {
    assert!(!is_valid_jobicy_url("javascript:alert(1)"));
    assert!(!is_valid_jobicy_url("ftp://jobicy.com/x"));
}

#[test]
fn test_map_job_drops_row_with_non_jobicy_url() {
    // A drifting/hostile response could inject a foreign URL — must be
    // dropped, never kept with a non-jobicy.com link.
    let j: Job = serde_json::from_str(
        r#"{"id": 1, "url": "https://evil-phish.example/x", "jobTitle": "Some Job"}"#,
    )
    .unwrap();
    assert!(map_job(j, "jobicy", 0).is_none());
}

// ── per-row deserialization resilience ───────────────────────────────────────

#[test]
fn test_rows_to_jobs_skips_malformed_row_keeps_good_ones() {
    // One row has `id` as a STRING (schema drift on a single row); the other
    // two are well-formed. Without per-row resilience, `Vec<Job>`'s atomic
    // deserialize would zero the whole batch on that one bad row.
    let values: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
            {"id": 1, "url": "https://jobicy.com/jobs/1-x", "jobTitle": "Good Job A"},
            {"id": "not-a-number", "url": "https://jobicy.com/jobs/2-x", "jobTitle": "Bad Row"},
            {"id": 3, "url": "https://jobicy.com/jobs/3-x", "jobTitle": "Good Job B"}
        ]"#,
    )
    .unwrap();

    let jobs = rows_to_jobs(values);
    assert_eq!(
        jobs.len(),
        2,
        "the malformed row must be skipped; both good rows must survive"
    );
    assert_eq!(jobs[0].job_title.as_deref(), Some("Good Job A"));
    assert_eq!(jobs[1].job_title.as_deref(), Some("Good Job B"));
}

#[test]
fn test_rows_to_jobs_empty_input_returns_empty() {
    assert!(rows_to_jobs(Vec::new()).is_empty());
}

// ── keyless/empty response ───────────────────────────────────────────────────

#[test]
fn test_empty_jobs_array_parses_to_empty_vec() {
    let resp: Resp = serde_json::from_str(r#"{"jobs": []}"#).unwrap();
    assert!(resp.jobs.is_empty());
}

#[test]
fn test_missing_jobs_field_defaults_to_empty_vec() {
    // Defensive: an unexpected/absent `jobs` key must not fail deserialization
    // (`#[serde(default)]`) — a keyless-empty response is `Ok(vec![])`, never a
    // parse error or a fabricated result.
    let resp: Resp = serde_json::from_str(r#"{"jobCount": 0}"#).unwrap();
    assert!(resp.jobs.is_empty());
}

// ── parse_response (hermetic — the 404-vs-other-status contract) ────────────
//
// `search()` delegates its "is this a real failure or a 404-but-valid-empty-
// result" decision to `parse_response`, a pure function with no network call,
// so this load-bearing contract no longer relies solely on the `#[ignore]`d
// live-network tests below.

#[test]
fn test_parse_response_500_is_err() {
    let result = parse_response(500, "Internal Server Error", "jobicy", 0);
    assert!(result.is_err(), "a real 5xx outage must never be Ok");
}

#[test]
fn test_parse_response_403_is_err() {
    let result = parse_response(403, "Forbidden", "jobicy", 0);
    assert!(result.is_err(), "a 403 must never be misread as empty");
}

#[test]
fn test_parse_response_404_with_empty_jobs_json_is_ok_empty() {
    let body = r#"{"jobs":[],"success":false,"message":"Nothing found..."}"#;
    let result = parse_response(404, body, "jobicy", 0);
    assert!(
        result.is_ok(),
        "a 404 with a valid empty-jobs JSON body is a genuine zero-match search"
    );
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_parse_response_200_with_jobs_is_ok_with_postings() {
    let result = parse_response(200, FIXTURE_JSON, "jobicy", 0);
    assert!(result.is_ok(), "a 200 with a well-formed body must parse");
    // job3 has no id and is dropped; job1+job2 survive (see FIXTURE_JSON doc).
    assert_eq!(result.unwrap().len(), 2);
}

#[test]
fn test_parse_response_404_with_html_body_is_err() {
    // A real routing 404 (e.g. a CDN/edge error page) returns HTML, not the
    // `{"jobs":[...]}` shape — JSON parsing must fail and propagate as `Err`,
    // never a silently-empty `Ok`.
    let body = "<html><body>404 Not Found</body></html>";
    let result = parse_response(404, body, "jobicy", 0);
    assert!(
        result.is_err(),
        "a 404 with an HTML (non-JSON) body is a real outage, not an empty result"
    );
}

// ── live network (ignored in CI) ─────────────────────────────────────────────

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = JobicyScraper;
    let input = BoardSearchInput {
        query: "engineer".to_string(),
        location: None,
        amount: 10,
        pages: 1,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
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
        on_truncation: None,
        on_note: None,
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
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    assert!(
        first.description.is_some(),
        "jobicy always returns a full description — feed changed?"
    );
    println!("jobicy: {} results", postings.len());
    println!("first: {:?}", first.title);
}

#[tokio::test]
#[ignore = "live network"]
async fn live_garbage_tag_returns_ok_empty_not_error() {
    // Regression guard for the live-verified quirk `search()` depends on:
    // Jobicy returns HTTP 404 with a valid JSON body for a genuine zero-match
    // `tag` search. That must resolve `Ok(vec![])`, not an `Err` — otherwise a
    // healthy "no results for this keyword" search would misreport as a
    // failed board in `BoardScrapeSummary.error`.
    let scraper = JobicyScraper;
    let input = BoardSearchInput {
        query: "zzz-guaranteed-no-match-xyz123".to_string(),
        location: None,
        amount: 5,
        pages: 1,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
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
        on_truncation: None,
        on_note: None,
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        scraper.search(input, ctx),
    )
    .await
    .expect("live search timed out");
    assert!(
        result.is_ok(),
        "a zero-match tag search must be Ok(empty), not Err: {:?}",
        result.err()
    );
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn search_respects_pre_cancelled_token() {
    // Not `#[ignore = "live network"]`: `fetch_text` checks `signal.is_cancelled()`
    // BEFORE issuing any request (scraping/http/mod.rs), so a pre-cancelled token
    // makes zero network calls — this hermetically exercises the cancellation
    // contract. `AppError::Cancelled` must propagate as an `Err`, not a
    // fabricated empty success.
    let scraper = JobicyScraper;
    let signal = tokio_util::sync::CancellationToken::new();
    signal.cancel();
    let input = BoardSearchInput {
        query: "".to_string(),
        location: None,
        amount: 10,
        pages: 1,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: Vec::new(),
    };
    let ctx = ScrapeContext {
        signal,
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        scraper.search(input, ctx),
    )
    .await
    .expect("cancelled search must not hang");
    assert!(
        result.is_err(),
        "a pre-cancelled token must surface as an error, not a silent empty Ok"
    );
}
