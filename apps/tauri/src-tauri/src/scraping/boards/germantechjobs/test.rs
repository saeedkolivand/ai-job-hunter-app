use super::*;

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
    let scraper = GermanTechJobsScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_german_tech_jobs_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_title_company_split() {
    // "Job Title @ Company [salary]" — verify the split logic used in search()
    let title = "Senior Rust Engineer @ Acme Corp [70.000 - 100.000 €]";
    let (job_part, company_part) = if let Some(at_pos) = title.find(" @ ") {
        (&title[..at_pos], &title[at_pos + 3..])
    } else {
        (title, "Unknown")
    };
    assert_eq!(job_part, "Senior Rust Engineer");
    assert!(company_part.starts_with("Acme Corp"));
}

#[test]
fn test_title_no_at_sign() {
    let title = "Backend Developer";
    let result = if title.find(" @ ").is_some() {
        "split"
    } else {
        "no-split"
    };
    assert_eq!(result, "no-split");
}

#[test]
fn test_salary_bracket_stripped() {
    // Salary bracket must be removed; role text must be preserved intact.
    let input = "Senior Rust Engineer [70.000 - 100.000 €]";
    let result = SALARY_BRACKET_RE.replace(input, "").trim().to_string();
    assert_eq!(result, "Senior Rust Engineer");

    // No bracket → title passes through unchanged.
    let no_bracket = "Backend Engineer";
    let result2 = SALARY_BRACKET_RE.replace(no_bracket, "").trim().to_string();
    assert_eq!(result2, "Backend Engineer");
}

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
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    println!("germantechjobs: {} results", postings.len());
    println!("first: {:?}", first.title);
}
