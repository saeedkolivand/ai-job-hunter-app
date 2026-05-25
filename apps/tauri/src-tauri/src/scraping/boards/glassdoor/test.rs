use super::*;

#[test]
fn test_glassdoor_scraper_id() {
    let scraper = GlassdoorScraper;
    assert_eq!(scraper.id(), "glassdoor");
}

#[test]
fn test_glassdoor_scraper_display_name() {
    let scraper = GlassdoorScraper;
    assert_eq!(scraper.display_name(), "Glassdoor");
}

#[test]
fn test_glassdoor_scraper_mode() {
    let scraper = GlassdoorScraper;
    assert_eq!(scraper.mode(), ScraperMode::Browser);
}

#[test]
fn test_glassdoor_scraper_mode_partial_eq() {
    let mode = ScraperMode::Browser;
    assert_eq!(mode, ScraperMode::Browser);
    assert_ne!(mode, ScraperMode::Http);
}

#[test]
fn test_glassdoor_url_parsing_first_page() {
    let query = urlencoding::encode("software engineer");
    let url = format!(
        "https://www.glassdoor.com/Job/jobs.htm?sc.keyword={}&locT=C&locId=&jobType=&fromAge=-1&minSalary=0&includeNoSalaryJobs=true&radius=0&cityId=-1&minRating=0.0&industryId=-1&sgocId=-1&seniorityType=&companyId=-1&employerSizes=0&applicationType=0&remoteWorkType=0",
        query
    );
    assert!(url.contains("sc.keyword="));
    assert!(!url.contains("&p="));
}

#[test]
fn test_glassdoor_url_paging() {
    let query = urlencoding::encode("software engineer");
    let url = format!(
        "https://www.glassdoor.com/Job/jobs.htm?sc.keyword={}&locT=C&locId=&jobType=&fromAge=-1&minSalary=0&includeNoSalaryJobs=true&radius=0&cityId=-1&minRating=0.0&industryId=-1&sgocId=-1&seniorityType=&companyId=-1&employerSizes=0&applicationType=0&remoteWorkType=0&p={}",
        query, 2
    );
    assert!(url.contains("&p=2"));
}

#[test]
fn test_glassdoor_job_id_extraction() {
    let url = "https://www.glassdoor.com/job-listing/job.htm?jobListingId=123456&from=JS";
    let external_id = url
        .split("jobListingId=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .unwrap_or("")
        .to_string();
    assert_eq!(external_id, "123456");
}

#[test]
fn test_glassdoor_job_id_extraction_no_id() {
    let url = "https://www.glassdoor.com/job-listing/job.htm";
    let external_id = url
        .split("jobListingId=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .unwrap_or("")
        .to_string();
    assert_eq!(external_id, "");
}

#[test]
fn test_glassdoor_href_normalization() {
    let href = "/job-listing/job.htm?jobListingId=123";
    let url = if href.starts_with("http") {
        href.to_string()
    } else {
        format!("https://www.glassdoor.com{}", href)
    };
    assert_eq!(url, "https://www.glassdoor.com/job-listing/job.htm?jobListingId=123");
}

#[test]
fn test_glassdoor_href_already_absolute() {
    let href = "https://www.glassdoor.com/job-listing/job.htm";
    let url = if href.starts_with("http") {
        href.to_string()
    } else {
        format!("https://www.glassdoor.com{}", href)
    };
    assert_eq!(url, "https://www.glassdoor.com/job-listing/job.htm");
}
