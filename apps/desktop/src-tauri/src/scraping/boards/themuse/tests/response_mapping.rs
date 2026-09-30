use super::super::super::test_support::*;
use super::super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(super) fn make_input(query: &str, location: Option<&str>) -> BoardSearchInput {
    BoardSearchInput {
        query: query.to_string(),
        location: location.map(|s| s.to_string()),
        ..default_search_input()
    }
}

pub(super) fn make_ctx() -> ScrapeContext {
    default_ctx()
}

pub(super) fn jobs_from(json: &str) -> Vec<TmJob> {
    let resp: TmResponse = serde_json::from_str(json).expect("fixture must parse");
    resp.results
}

// ---------------------------------------------------------------------------
// Scraper metadata
// ---------------------------------------------------------------------------

#[test]
fn test_themuse_scraper_id() {
    let scraper = TheMuseScraper;
    assert_eq!(scraper.id(), "themuse");
}

#[test]
fn test_themuse_scraper_display_name() {
    let scraper = TheMuseScraper;
    assert_eq!(scraper.display_name(), "The Muse");
}

#[test]
fn test_themuse_scraper_mode() {
    let scraper = TheMuseScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

// ---------------------------------------------------------------------------
// is_valid_http_url
// ---------------------------------------------------------------------------

#[test]
fn url_guard_accepts_http_and_https() {
    assert!(is_valid_http_url(
        "https://www.themuse.com/jobs/acme/engineer"
    ));
    assert!(is_valid_http_url(
        "http://www.themuse.com/jobs/acme/engineer"
    ));
}

#[test]
fn url_guard_rejects_non_http_schemes_and_garbage() {
    assert!(!is_valid_http_url("not-a-url"));
    assert!(!is_valid_http_url("ftp://example.com/job"));
    assert!(!is_valid_http_url(""));
}

// ---------------------------------------------------------------------------
// parse_themuse_response — fixture-based parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_themuse_response_happy_path() {
    let json = r#"{
        "results": [
            {
                "name": "Senior Backend Engineer",
                "refs": { "landing_page": "https://www.themuse.com/jobs/acme/senior-backend-engineer" },
                "company": { "name": "Acme Corp" },
                "locations": [ { "name": "Remote (US)" }, { "name": "New York, NY" } ]
            }
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 1_700_000_000_000);

    assert_eq!(postings.len(), 1);
    let p = &postings[0];
    assert_eq!(p.title, "Senior Backend Engineer");
    assert_eq!(
        p.url,
        "https://www.themuse.com/jobs/acme/senior-backend-engineer"
    );
    assert_eq!(p.company, "Acme Corp");
    assert_eq!(p.location, Some("Remote (US)".to_string()));
    assert_eq!(p.id, format!("themuse:{}", p.url));
    assert_eq!(p.external_id, Some(p.url.clone()));
    assert_eq!(p.source, "themuse");
    assert_eq!(p.captured_at, 1_700_000_000_000);
}

#[test]
fn parse_themuse_response_company_falls_back_to_the_muse() {
    let json = r#"{
        "results": [
            {
                "name": "No Company Listing",
                "refs": { "landing_page": "https://www.themuse.com/jobs/x/no-company" },
                "company": null,
                "locations": null
            },
            {
                "name": "Blank Company Name",
                "refs": { "landing_page": "https://www.themuse.com/jobs/x/blank-company" },
                "company": { "name": "   " },
                "locations": null
            }
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 0);
    assert_eq!(postings.len(), 2);
    assert_eq!(postings[0].company, "The Muse");
    assert_eq!(postings[1].company, "The Muse");
}

#[test]
fn parse_themuse_response_location_empty_when_missing_or_empty() {
    let json = r#"{
        "results": [
            {
                "name": "No Locations Field",
                "refs": { "landing_page": "https://www.themuse.com/jobs/x/no-locations" },
                "company": { "name": "Acme" },
                "locations": null
            },
            {
                "name": "Empty Locations Array",
                "refs": { "landing_page": "https://www.themuse.com/jobs/x/empty-locations" },
                "company": { "name": "Acme" },
                "locations": []
            }
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 0);
    assert_eq!(postings.len(), 2);
    assert_eq!(
        postings[0].location, None,
        "matches fleet peers (e.g. Arbeitnow): missing location is None, not Some(\"\")"
    );
    assert_eq!(postings[1].location, None);
}

#[test]
fn parse_themuse_response_empty_results_returns_empty_vec() {
    let json = r#"{ "results": [], "page_count": 0 }"#;
    let jobs = jobs_from(json);
    assert!(
        parse_themuse_response(jobs, 0).is_empty(),
        "empty results array must parse to an empty Vec, not an error"
    );
}

/// Missing/empty `name` and missing/invalid `refs.landing_page` each drop the
/// row; valid rows in the same payload must still come through.
#[test]
fn parse_themuse_response_drops_malformed_rows() {
    let json = r#"{
        "results": [
            {"name": "Valid One", "refs": {"landing_page": "https://www.themuse.com/jobs/x/valid-one"}, "company": null, "locations": null},
            {"name": null, "refs": {"landing_page": "https://www.themuse.com/jobs/x/missing-name"}, "company": null, "locations": null},
            {"name": "", "refs": {"landing_page": "https://www.themuse.com/jobs/x/empty-name"}, "company": null, "locations": null},
            {"name": "Missing Refs", "refs": null, "company": null, "locations": null},
            {"name": "Missing Landing Page", "refs": {"landing_page": null}, "company": null, "locations": null},
            {"name": "Malformed URL", "refs": {"landing_page": "not-a-url"}, "company": null, "locations": null},
            {"name": "Valid Two", "refs": {"landing_page": "https://www.themuse.com/jobs/x/valid-two"}, "company": null, "locations": null}
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 0);
    let titles: Vec<&str> = postings.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Valid One", "Valid Two"],
        "malformed rows must be dropped without panicking, valid rows kept: {titles:?}"
    );
}

/// The Muse response has no stable job id — the (validated) posting URL
/// doubles as the id. The parser does not itself dedupe (unlike Pinpoint), so
/// two rows sharing a landing_page produce two postings that share the same
/// id format `themuse:{url}` — the property the DB PK layer relies on to
/// collapse duplicates.
#[test]
fn parse_themuse_response_url_as_id_format_and_identical_urls_share_id() {
    let json = r#"{
        "results": [
            {"name": "First Listing", "refs": {"landing_page": "https://www.themuse.com/jobs/x/dup"}, "company": {"name": "Acme"}, "locations": null},
            {"name": "Duplicate Listing", "refs": {"landing_page": "https://www.themuse.com/jobs/x/dup"}, "company": {"name": "Acme"}, "locations": null},
            {"name": "Distinct Listing", "refs": {"landing_page": "https://www.themuse.com/jobs/x/other"}, "company": {"name": "Acme"}, "locations": null}
        ],
        "page_count": 1
    }"#;
    let jobs = jobs_from(json);
    let postings = parse_themuse_response(jobs, 0);
    assert_eq!(postings.len(), 3, "parser itself does not dedupe rows");

    assert_eq!(
        postings[0].id,
        format!("themuse:{}", postings[0].url),
        "id must be `themuse:{{url}}`"
    );
    assert_eq!(
        postings[0].id, postings[1].id,
        "identical landing_page urls must produce identical ids (DB PK dedupe key)"
    );
    assert_ne!(
        postings[0].id, postings[2].id,
        "distinct urls must produce distinct ids"
    );
}
