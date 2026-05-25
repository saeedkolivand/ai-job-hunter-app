use super::*;

#[test]
fn test_remoteok_scraper_id() {
    let scraper = RemoteOkScraper;
    assert_eq!(scraper.id(), "remoteok");
}

#[test]
fn test_remoteok_scraper_display_name() {
    let scraper = RemoteOkScraper;
    assert_eq!(scraper.display_name(), "RemoteOK");
}

#[test]
fn test_remoteok_scraper_mode() {
    let scraper = RemoteOkScraper;
    assert_eq!(scraper.mode(), ScraperMode::Http);
}

#[test]
fn test_remoteok_scraper_mode_partial_eq() {
    let mode = ScraperMode::Http;
    assert_eq!(mode, ScraperMode::Http);
    assert_ne!(mode, ScraperMode::Browser);
}

#[test]
fn test_remote_ok_item_job_variant() {
    let item = RemoteOkItem::Job {
        id: Some(serde_json::json!(123)),
        slug: Some("test-job".to_string()),
        position: Some("Software Engineer".to_string()),
        company: Some("Test Corp".to_string()),
        location: Some("Remote".to_string()),
        tags: Some(vec!["rust".to_string()]),
        description: None,
        url: None,
        apply_url: None,
        date: None,
    };
    match item {
        RemoteOkItem::Job { position, .. } => {
            assert_eq!(position, Some("Software Engineer".to_string()));
        }
        _ => panic!("Expected Job variant"),
    }
}

#[test]
fn test_remote_ok_item_legend_variant() {
    let item = RemoteOkItem::Legend {
        _slug: "legend".to_string(),
    };
    match item {
        RemoteOkItem::Legend { .. } => {
            // Successfully matched legend
        }
        _ => panic!("Expected Legend variant"),
    }
}

#[test]
fn test_remote_ok_item_job_defaults() {
    let item = RemoteOkItem::Job {
        id: None,
        slug: None,
        position: None,
        company: None,
        location: None,
        tags: None,
        description: None,
        url: None,
        apply_url: None,
        date: None,
    };
    match item {
        RemoteOkItem::Job { position, company, .. } => {
            assert!(position.is_none());
            assert!(company.is_none());
        }
        _ => panic!("Expected Job variant"),
    }
}
