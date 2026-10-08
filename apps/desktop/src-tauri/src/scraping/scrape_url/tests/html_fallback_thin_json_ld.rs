//! A thin JSON-LD summary loses to a much longer body of the SAME posting (#1400), and only to
//! that: listing text, hinted pages and full descriptions keep the JSON-LD.
//!
//! The specimen in the issue (product.ai) answers bots with a Cloudflare 403, so the fixture
//! reproduces its shape instead: a ~265-char JSON-LD summary and a long role body.

use super::super::*;

const URL: &str = "https://product.ai/join/product-engineer/";

fn json_ld(desc: &str) -> String {
    format!(
        r#"<script type="application/ld+json">{{"@type":"JobPosting","title":"Product Engineer","description":"{desc}"}}</script>"#
    )
}

fn role_body() -> String {
    (0..60)
        .map(|i| {
            format!("<p>Responsibility {i}: build and ship product features with the team.</p>")
        })
        .collect()
}

fn desc_of(html: &str) -> String {
    parse_from_html(URL, html).unwrap().description.unwrap()
}

#[test]
fn thin_json_ld_summary_loses_to_the_long_body_of_the_same_posting() {
    let html = format!(
        "<html><head>{}</head><body><nav>Home Jobs About</nav><main><h1>Product Engineer</h1>{}</main></body></html>",
        json_ld("<p>Join us to build the product.</p>"),
        role_body()
    );
    assert!(desc_of(&html).contains("Responsibility 59"));
}

#[test]
fn listing_page_text_never_replaces_a_thin_json_ld() {
    let others: String = (0..80)
        .map(|i| format!("<li>Data Scientist {i} - Berlin - apply for this opening today</li>"))
        .collect();
    let html = format!(
        "<html><head>{}</head><body><main><h1>Open roles</h1><ul>{others}</ul></main></body></html>",
        json_ld("<p>Join us to build the product.</p>")
    );
    let desc = desc_of(&html);
    assert!(desc.contains("Join us to build the product"), "{desc}");
    assert!(!desc.contains("Data Scientist"), "{desc}");
}

#[test]
fn listing_that_mentions_the_title_deep_in_the_body_keeps_the_thin_json_ld() {
    let others: String = (0..80)
        .map(|i| format!("<li>Data Scientist {i} - Berlin - apply for this opening today</li>"))
        .collect();
    let html = format!(
        "<html><head>{}</head><body><main><h1>Open roles</h1><ul>{others}<li>Product Engineer - Berlin</li></ul></main></body></html>",
        json_ld("<p>Join us to build the product.</p>")
    );
    let desc = desc_of(&html);
    assert!(desc.contains("Join us to build the product"), "{desc}");
}

#[test]
fn title_only_matches_whole_words() {
    let html = format!(
        "<html><head>{}</head><body><main><h1>Product Engineering</h1>{}</main></body></html>",
        json_ld("<p>Join us to build the product.</p>"),
        role_body()
    );
    assert!(!desc_of(&html).contains("Responsibility 59"));
}

#[test]
fn hinted_page_keeps_its_thin_json_ld() {
    let html = format!(
        r#"<html><head>{}</head><body><div data-ajh-job-root="true"><h1>Product Engineer</h1></div><main><h1>Product Engineer</h1>{}</main></body></html>"#,
        json_ld("<p>Join us to build the product.</p>"),
        role_body()
    );
    let desc = desc_of(&html);
    assert!(desc.contains("Join us to build the product"), "{desc}");
    assert!(!desc.contains("Responsibility 59"), "{desc}");
}

#[test]
fn json_ld_at_the_cap_is_kept_against_a_huge_body() {
    // 38 x 27 = 1026 plain-text chars: over THIN_JSON_LD_MAX_CHARS, so the body is never consulted.
    let full = format!("<p>{}</p>", "Detail line about the role. ".repeat(38));
    let html = format!(
        "<html><head>{}</head><body><main><h1>Product Engineer</h1>{}{}</main></body></html>",
        json_ld(&full),
        role_body(),
        role_body()
    );
    let desc = desc_of(&html);
    assert!(desc.contains("Detail line about the role"));
    assert!(!desc.contains("Responsibility"), "{desc}");
}
