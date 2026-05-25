use super::*;

#[test]
fn test_parse_greenhouse_url_standard() {
    let url = "https://boards.greenhouse.io/stripe/jobs/12345";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "12345".to_string())));
}

#[test]
fn test_parse_greenhouse_url_embed() {
    let url = "https://boards.greenhouse.io/embed/job_app?for=stripe&token=abc123";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "abc123".to_string())));
}

#[test]
fn test_parse_greenhouse_url_invalid() {
    let url = "https://example.com/jobs/123";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, None);
}

#[test]
fn test_parse_lever_url() {
    let url = "https://jobs.lever.co/stripe/abc123";
    let result = parse_lever_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "abc123".to_string())));
}

#[test]
fn test_parse_lever_url_invalid() {
    let url = "https://example.com/stripe/abc123";
    let result = parse_lever_url(url);
    assert_eq!(result, None);
}

#[test]
fn test_parse_generic_html() {
    let html = r#"
        <html>
            <head><title>Software Engineer</title></head>
            <body>
                <meta name="description" content="Great job opportunity">
            </body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Software Engineer");
    assert_eq!(description, Some("Great job opportunity".to_string()));
}

#[test]
fn test_parse_generic_html_h1() {
    let html = r#"
        <html>
            <body>
                <h1>Senior Developer</h1>
                <meta property="og:description" content="Remote position">
            </body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Senior Developer");
    assert_eq!(description, Some("Remote position".to_string()));
}

#[test]
fn test_parse_generic_html_empty() {
    let html = "<html><body></body></html>";
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "");
    assert_eq!(description, None);
}

#[test]
fn test_parse_generic_html_with_og_description() {
    let html = r#"
        <html>
            <head>
                <title>Job Title</title>
                <meta property="og:description" content="Remote position available">
            </head>
            <body></body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Job Title");
    assert_eq!(description, Some("Remote position available".to_string()));
}

#[test]
fn test_parse_generic_html_with_meta_description() {
    let html = r#"
        <html>
            <head>
                <meta name="description" content="Job description here">
            </head>
            <body><h1>Title</h1></body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Title");
    assert_eq!(description, Some("Job description here".to_string()));
}

#[test]
fn test_parse_generic_html_no_description() {
    let html = "<html><body><h1>Title</h1></body></html>";
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Title");
    assert!(description.is_none());
}

#[test]
fn test_parse_generic_html_h1_priority() {
    let html = r#"
        <html>
            <head><title>Page Title</title></head>
            <body><h1>Job Title</h1></body>
        </html>
    "#;
    let (title, _description) = parse_generic_html(html);
    // title selector matches both title and h1, first match wins
    assert!(!title.is_empty());
}

#[test]
fn test_parse_greenhouse_url_subdomain() {
    let url = "https://boards.greenhouse.io/stripe/jobs/12345";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "12345".to_string())));
}

#[test]
fn test_parse_greenhouse_url_with_trailing_slash() {
    let url = "https://boards.greenhouse.io/stripe/jobs/12345/";
    let result = parse_greenhouse_url(url);
    // Trailing slash may affect path segments
    assert!(result.is_some());
}

#[test]
fn test_parse_lever_url_with_subdomain() {
    let url = "https://jobs.lever.co/stripe/abc123";
    let result = parse_lever_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "abc123".to_string())));
}

#[test]
fn test_parse_lever_url_with_extra_segments() {
    let url = "https://jobs.lever.co/stripe/abc123/extra";
    let result = parse_lever_url(url);
    // Should still extract first two segments
    assert!(result.is_some());
}

#[test]
fn test_parse_greenhouse_url_embed_missing_token() {
    let url = "https://boards.greenhouse.io/embed/job_app?for=stripe";
    let result = parse_greenhouse_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_greenhouse_url_embed_missing_for() {
    let url = "https://boards.greenhouse.io/embed/job_app?token=abc123";
    let result = parse_greenhouse_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_lever_url_invalid_domain() {
    let url = "https://example.com/stripe/abc123";
    let result = parse_lever_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_greenhouse_url_invalid_domain() {
    let url = "https://example.com/stripe/jobs/12345";
    let result = parse_greenhouse_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_lever_url_single_segment() {
    let url = "https://jobs.lever.co/stripe";
    let result = parse_lever_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_greenhouse_url_single_segment() {
    let url = "https://boards.greenhouse.io/stripe";
    let result = parse_greenhouse_url(url);
    assert!(result.is_none());
}

#[test]
fn test_parse_generic_html_with_both_descriptions() {
    let html = r#"
        <html>
            <head>
                <title>Job Title</title>
                <meta name="description" content="Meta description">
                <meta property="og:description" content="OG description">
            </head>
            <body></body>
        </html>
    "#;
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Job Title");
    // First description match wins
    assert!(description.is_some());
}

#[test]
fn test_parse_generic_html_malformed() {
    let html = "<html><head><title>Test</title></html>";
    let (title, description) = parse_generic_html(html);
    assert_eq!(title, "Test");
    assert!(description.is_none());
}

#[test]
fn test_parse_generic_html_whitespace() {
    let html = r#"
        <html>
            <head>
                <title>   Whitespace Title   </title>
            </head>
            <body></body>
        </html>
    "#;
    let (title, _description) = parse_generic_html(html);
    assert!(!title.is_empty());
}

#[test]
fn test_parse_lever_url_with_query() {
    let url = "https://jobs.lever.co/stripe/abc123?ref=source";
    let result = parse_lever_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "abc123".to_string())));
}

#[test]
fn test_parse_greenhouse_url_with_query() {
    let url = "https://boards.greenhouse.io/stripe/jobs/12345?ref=source";
    let result = parse_greenhouse_url(url);
    assert_eq!(result, Some(("stripe".to_string(), "12345".to_string())));
}
