use super::*;

#[test]
fn test_parse_relative_time_hours() {
    let result = parse_relative_time("1 hour ago");
    assert!(result.is_some());
}

#[test]
fn test_parse_relative_time_days() {
    let result = parse_relative_time("2 days ago");
    assert!(result.is_some());
}

#[test]
fn test_parse_relative_time_weeks() {
    let result = parse_relative_time("1 week ago");
    assert!(result.is_some());
}

#[test]
fn test_parse_relative_time_minutes() {
    let result = parse_relative_time("30 minutes ago");
    assert!(result.is_some());
}

#[test]
fn test_parse_relative_time_invalid() {
    let result = parse_relative_time("invalid time");
    assert!(result.is_none());
}

#[test]
fn test_parse_relative_time_empty() {
    let result = parse_relative_time("");
    assert!(result.is_none());
}

#[test]
fn test_parse_relative_time_months() {
    let result = parse_relative_time("3 months ago");
    assert!(result.is_some());
}

#[test]
fn test_jobs_search_params_default() {
    let params = JobsSearchParams {
        keywords: "rust".to_string(),
        location: None,
        start: 0,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        geo_id: None,
        distance: None,
    };
    assert_eq!(params.keywords, "rust");
    assert_eq!(params.start, 0);
}

#[test]
fn test_jobs_search_params_with_filters() {
    let params = JobsSearchParams {
        keywords: "rust".to_string(),
        location: Some("remote".to_string()),
        start: 0,
        date_filter: Some("week".to_string()),
        job_type: Some("F".to_string()),
        work_type: Some("1".to_string()),
        experience_level: Some("2".to_string()),
        easy_apply: Some(true),
        actively_hiring: Some(true),
        verified: Some(false),
        sort_by: Some("DD".to_string()),
        geo_id: None,
        distance: None,
    };
    assert_eq!(params.location, Some("remote".to_string()));
    assert_eq!(params.easy_apply, Some(true));
}

#[test]
fn test_parse_relative_time_zero() {
    let result = parse_relative_time("0 hours ago");
    assert!(result.is_some());
}

#[test]
fn test_parse_relative_time_case_insensitive() {
    let result = parse_relative_time("2 HOURS AGO");
    assert!(result.is_some());
}

#[test]
fn test_jobs_search_params_with_start() {
    let params = JobsSearchParams {
        keywords: "rust".to_string(),
        location: None,
        start: 25,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        geo_id: None,
        distance: None,
    };
    assert_eq!(params.start, 25);
}

#[test]
fn test_jobs_search_params_all_filters_false() {
    let params = JobsSearchParams {
        keywords: "rust".to_string(),
        location: None,
        start: 0,
        date_filter: None,
        job_type: None,
        work_type: None,
        experience_level: None,
        easy_apply: Some(false),
        actively_hiring: Some(false),
        verified: Some(false),
        sort_by: None,
        geo_id: None,
        distance: None,
    };
    assert_eq!(params.easy_apply, Some(false));
    assert_eq!(params.actively_hiring, Some(false));
}

#[test]
fn test_jobs_search_params_clone() {
    let params = JobsSearchParams {
        keywords: "rust".to_string(),
        location: Some("remote".to_string()),
        start: 0,
        date_filter: Some("week".to_string()),
        job_type: Some("F".to_string()),
        work_type: Some("1".to_string()),
        experience_level: Some("2".to_string()),
        easy_apply: Some(true),
        actively_hiring: Some(true),
        verified: Some(false),
        sort_by: Some("DD".to_string()),
        geo_id: None,
        distance: None,
    };
    let cloned = params.clone();
    assert_eq!(params.keywords, cloned.keywords);
    assert_eq!(params.location, cloned.location);
}

#[test]
fn test_parse_relative_time_with_spaces() {
    let result = parse_relative_time("  2 hours ago  ");
    assert!(result.is_some());
}

#[test]
fn test_parse_relative_time_large_number() {
    let result = parse_relative_time("100 days ago");
    assert!(result.is_some());
}

#[test]
fn test_parse_relative_time_minutes_not_months() {
    // Regression: "minutes" must NOT be matched as "months" via the "m" stem.
    // Capture `now` AFTER the call so the measured elapsed is not fractionally
    // short of the parsed duration (which would truncate down).
    let result = parse_relative_time("30 minutes ago").expect("should parse minutes");
    let mins = chrono::Utc::now()
        .signed_duration_since(result)
        .num_minutes();
    assert!(
        (29..=31).contains(&mins),
        "expected ~30 minutes, got {mins}"
    );
}

#[test]
fn test_parse_relative_time_hours_value() {
    let result = parse_relative_time("5 hours ago").expect("should parse hours");
    let mins = chrono::Utc::now()
        .signed_duration_since(result)
        .num_minutes();
    assert!(
        (299..=301).contains(&mins),
        "expected ~300 minutes, got {mins}"
    );
}

#[test]
fn test_parse_relative_time_months_value() {
    let result = parse_relative_time("3 months ago").expect("should parse months");
    let days = chrono::Utc::now().signed_duration_since(result).num_days();
    assert_eq!(days, 90, "expected 90 days (3*30), got {days}");
}

#[test]
fn test_parse_iso_date_bare() {
    let result = parse_iso_date("2024-01-15").expect("should parse YYYY-MM-DD");
    assert_eq!(result.format("%Y-%m-%d").to_string(), "2024-01-15");
}

#[test]
fn test_parse_iso_date_rfc3339() {
    let result = parse_iso_date("2024-01-15T10:30:00Z").expect("should parse RFC3339");
    assert_eq!(result.format("%Y-%m-%d").to_string(), "2024-01-15");
}

#[test]
fn test_parse_iso_date_invalid() {
    assert!(parse_iso_date("not a date").is_none());
    assert!(parse_iso_date("1 hour ago").is_none());
}

#[tokio::test]
async fn test_cancellable_sleep_aborts_on_cancel() {
    let token = tokio_util::sync::CancellationToken::new();
    token.cancel(); // pre-cancelled
    let start = std::time::Instant::now();
    let interrupted = cancellable_sleep(Some(&token), std::time::Duration::from_secs(5)).await;
    let elapsed = start.elapsed();
    assert!(interrupted, "pre-cancelled token must interrupt the sleep");
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "cancelled sleep must return promptly, took {elapsed:?}"
    );
}

#[tokio::test]
async fn test_cancellable_sleep_elapses_normally() {
    let start = std::time::Instant::now();
    let interrupted = cancellable_sleep(None, std::time::Duration::from_millis(10)).await;
    assert!(!interrupted, "no signal must elapse, not interrupt");
    assert!(
        start.elapsed() >= std::time::Duration::from_millis(10),
        "sleep must actually elapse the full duration"
    );
}
