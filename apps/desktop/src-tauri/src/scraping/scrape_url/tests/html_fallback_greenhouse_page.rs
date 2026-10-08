//! #1409: a direct `job-boards.greenhouse.io` page import. Fixtures are real captures
//! (scripts, svgs and the application form trimmed).

use super::super::*;

const GITLAB: &str = include_str!("fixtures/greenhouse_gitlab.html");
const ANTHROPIC: &str = include_str!("fixtures/greenhouse_anthropic.html");

#[test]
fn gitlab_page_resolves_the_display_name_and_drops_the_header_block() {
    let p = parse_from_html(
        "https://job-boards.greenhouse.io/gitlab/jobs/8860302002",
        GITLAB,
    )
    .unwrap();
    assert_eq!(p.company, "GitLab");
    let d = p.description.unwrap();
    assert!(
        d.starts_with("GitLab is the intelligent orchestration platform"),
        "{d}"
    );
}

#[test]
fn anthropic_page_resolves_the_display_name_and_drops_the_header_block() {
    let p = parse_from_html(
        "https://job-boards.greenhouse.io/anthropic/jobs/4461450008",
        ANTHROPIC,
    )
    .unwrap();
    assert_eq!(p.company, "Anthropic");
    let d = p.description.unwrap();
    assert!(d.starts_with("## **About Anthropic**"), "{d}");
}

#[test]
fn a_capitalised_logo_alt_is_not_a_company_off_greenhouse() {
    let html =
        r#"<html><head><title>Eng</title></head><body><img alt="Company Logo"></body></html>"#;
    let p = parse_from_html("https://careers.example.com/jobs/1", html).unwrap();
    assert_eq!(p.company, "careers.example.com");
}

#[test]
fn a_greenhouse_branded_logo_alt_falls_back_to_the_slug() {
    let html = r#"<html><head><title>Eng</title></head><body><div class="image-container"><img alt="Greenhouse Logo"></div></body></html>"#;
    let p = parse_from_html("https://job-boards.greenhouse.io/acme-corp/jobs/1", html).unwrap();
    assert_eq!(p.company, "Acme Corp");
}
