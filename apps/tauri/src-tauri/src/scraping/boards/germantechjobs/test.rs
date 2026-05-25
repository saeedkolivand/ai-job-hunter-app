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
fn test_next_job_struct_fields() {
    let job = NextJob {
        _id: Some("123".to_string()),
        id: None,
        slug: None,
        title: Some("Software Engineer".to_string()),
        company_name: Some("Test Corp".to_string()),
        description: None,
        location: None,
        remote: None,
        tags: None,
        skills: None,
        created_at: None,
        published_at: None,
        url: None,
    };
    assert_eq!(job._id, Some("123".to_string()));
    assert_eq!(job.title, Some("Software Engineer".to_string()));
}

#[test]
fn test_next_job_struct_defaults() {
    let job = NextJob {
        _id: None,
        id: None,
        slug: None,
        title: None,
        company_name: None,
        description: None,
        location: None,
        remote: None,
        tags: None,
        skills: None,
        created_at: None,
        published_at: None,
        url: None,
    };
    assert!(job.title.is_none());
    assert!(job.remote.is_none());
}

#[test]
fn test_next_data_regex() {
    let re = regex::Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap();
    let html = r#"<script id="__NEXT_DATA__" type="application/json">{"test": true}</script>"#;
    let caps = re.captures(html);
    assert!(caps.is_some());
}

#[test]
fn test_next_data_regex_no_match() {
    let re = regex::Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap();
    let html = "<div>No script here</div>";
    let caps = re.captures(html);
    assert!(caps.is_none());
}
