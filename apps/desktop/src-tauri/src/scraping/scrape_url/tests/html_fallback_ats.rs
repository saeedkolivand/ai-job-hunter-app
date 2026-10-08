//! #1359 (ATS slug as company) and #1360 (`embeds_ats_board` vs the page's own JSON-LD).

use super::super::*;

#[test]
fn parse_from_html_uses_the_ats_board_slug_not_the_host_as_company() {
    // #1359: a React-rendered Greenhouse page has no JSON-LD / og:site_name.
    let html = "<html><head><title>Staff Engineer</title></head><body></body></html>";
    let posting = parse_from_html(
        "https://job-boards.greenhouse.io/anthropic/jobs/5186067008",
        html,
    )
    .expect("a title is present");
    assert_eq!(posting.company, "anthropic");
}

#[test]
fn embeds_ats_board_ignores_a_page_with_its_own_job_posting_json_ld() {
    let html = r#"<html><head><script type="application/ld+json">
        {"@type":"JobPosting","title":"Engineer","description":"<p>Do it</p>"}
        </script></head><body><iframe src="https://jobs.ashbyhq.com/other"></iframe></body></html>"#;
    assert!(!embeds_ats_board(
        html,
        "https://careers.example.com/jobs/1"
    ));
}

#[test]
fn embeds_ats_board_ignores_the_real_ashby_shape_of_a_job_posting_page() {
    // #1360: a live Ashby posting ships a JobPosting JSON-LD AND `embedded-media` frames
    // that `ats_ref` reads as another board (slug `embed`).
    let html = r#"<html><head><script type="application/ld+json">
        {"@type":"JobPosting","title":"Engineer","description":"<p>Do it</p>"}
        </script></head><body>
        <iframe src="https://embedded-media.ashbyhq.com/embed/abc123"></iframe></body></html>"#;
    assert!(!embeds_ats_board(
        html,
        "https://jobs.ashbyhq.com/ashby/7458d4e9"
    ));
}

#[test]
fn embeds_ats_board_still_flags_a_wrapper_whose_json_ld_is_not_a_job_posting() {
    // #1238 guard: an Organization-only JSON-LD is no posting, so the wrapper stays flagged.
    let html = r#"<html><head><script type="application/ld+json">
        {"@type":"Organization","name":"Acme"}
        </script></head><body><iframe src="https://jobs.ashbyhq.com/acme"></iframe></body></html>"#;
    assert!(embeds_ats_board(html, "https://www.acme.example/karriere"));
}

#[test]
fn json_ld_and_og_site_name_beat_the_ats_slug_for_company() {
    let url = "https://job-boards.greenhouse.io/anthropic/jobs/1";
    let og = r#"<html><head><title>Eng</title><meta property="og:site_name" content="Anthropic PBC"></head></html>"#;
    assert_eq!(parse_from_html(url, og).unwrap().company, "Anthropic PBC");
    let ld = r#"<html><head><script type="application/ld+json">{"@type":"JobPosting","title":"Eng","hiringOrganization":{"name":"Anthropic Inc"}}</script></head></html>"#;
    assert_eq!(parse_from_html(url, ld).unwrap().company, "Anthropic Inc");
}

#[test]
fn the_ats_slug_company_is_percent_decoded() {
    let html = "<html><head><title>Eng</title></head></html>";
    let p = parse_from_html("https://job-boards.greenhouse.io/acme%20labs/jobs/1", html).unwrap();
    assert_eq!(p.company, "acme labs");
}

#[test]
fn parse_from_html_marks_a_page_with_its_own_job_posting_json_ld() {
    let ld = r#"<script type="application/ld+json">{"@type":"JobPosting","title":"Dev","hiringOrganization":{"name":"Acme"}}</script>"#;
    let with = parse_from_html(
        "https://acme.example/x",
        &format!("<html><head>{ld}</head></html>"),
    )
    .unwrap();
    assert_eq!(
        with.extra.get("company_src"),
        Some(&serde_json::json!("jsonld"))
    );
    let without = parse_from_html(
        "https://acme.example/x",
        "<html><head><title>Dev</title></head></html>",
    )
    .unwrap();
    assert!(!without.extra.contains_key("company_src"));
}
