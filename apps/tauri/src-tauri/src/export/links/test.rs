use super::*;

#[test]
fn url_label_maps_known_domains() {
    assert_eq!(url_label("https://www.linkedin.com/in/jane"), "LinkedIn");
    assert_eq!(url_label("http://github.com/jane"), "GitHub");
    assert_eq!(url_label("https://x.com/jane"), "Twitter");
    assert_eq!(url_label("https://twitter.com/jane"), "Twitter");
    assert_eq!(url_label("https://stackoverflow.com/u/1"), "Stack Overflow");
    assert_eq!(url_label("https://youtu.be/abc"), "YouTube");
    assert_eq!(url_label("https://crates.io/crates/serde"), "crates.io");
}

#[test]
fn url_label_falls_back_to_bare_domain() {
    assert_eq!(url_label("https://www.example.com/path/to/page"), "example.com");
    assert_eq!(url_label("http://my-portfolio.dev"), "my-portfolio.dev");
}

#[test]
fn display_text_strips_markdown_links() {
    let out = display_text("Berlin | [LinkedIn](https://linkedin.com/in/x) | done");
    assert_eq!(out, "Berlin | LinkedIn | done");
}

#[test]
fn display_text_leaves_plain_text_untouched() {
    assert_eq!(display_text("just plain text"), "just plain text");
}

#[test]
fn split_urls_returns_single_text_span_when_no_links() {
    let spans = split_urls("nothing to see here");
    assert_eq!(spans.len(), 1);
    assert!(matches!(&spans[0], Span::Text(t) if t == "nothing to see here"));
}

#[test]
fn split_urls_extracts_a_bare_url_with_friendly_label() {
    let spans = split_urls("see https://github.com/jane for more");
    let link = spans
        .iter()
        .find_map(|s| match s {
            Span::Link { label, url } => Some((label.clone(), url.clone())),
            Span::Text(_) => None,
        })
        .expect("expected a link span");
    assert_eq!(link.0, "GitHub");
    assert_eq!(link.1, "https://github.com/jane");
}

#[test]
fn split_urls_turns_emails_into_mailto_links() {
    let spans = split_urls("reach me at jane@example.com today");
    let has_mailto = spans.iter().any(|s| matches!(
        s,
        Span::Link { url, .. } if url == "mailto:jane@example.com"
    ));
    assert!(has_mailto);
}

#[test]
fn split_urls_prefers_markdown_links_over_bare_urls() {
    let spans = split_urls("[LinkedIn](https://linkedin.com/in/x)");
    assert_eq!(spans.len(), 1);
    match &spans[0] {
        Span::Link { label, url } => {
            assert_eq!(label, "LinkedIn");
            assert_eq!(url, "https://linkedin.com/in/x");
        }
        Span::Text(_) => panic!("expected a link span"),
    }
}

#[test]
fn split_urls_labels_arbitrary_website_with_bare_domain() {
    // A non-platform personal site / portfolio URL must survive with a bare-domain
    // label (mirrors the TS "Website" admission for the contact line).
    let spans = split_urls("portfolio: https://janedoe.dev/work today");
    let link = spans
        .iter()
        .find_map(|s| match s {
            Span::Link { label, url } => Some((label.clone(), url.clone())),
            Span::Text(_) => None,
        })
        .expect("expected a link span");
    assert_eq!(link.0, "janedoe.dev");
    assert_eq!(link.1, "https://janedoe.dev/work");
}

#[test]
fn url_label_matches_ts_url_to_friendly_label_fixture() {
    // Cross-language parity guard: this exact fixture is also asserted by the TS
    // urlToFriendlyLabel() test in packages/prompts/src/generate.test.ts. Both read
    // the same file, so the two implementations can never silently drift.
    #[derive(serde::Deserialize)]
    struct Case {
        url: String,
        label: String,
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/url-labels.json");
    let raw = std::fs::read_to_string(&path)
        .expect("read url-labels parity fixture (packages/prompts/src/fixtures/url-labels.json)");
    let cases: Vec<Case> = serde_json::from_str(&raw).expect("parse url-labels parity fixture");

    assert!(!cases.is_empty(), "url-labels parity fixture must not be empty");
    for c in &cases {
        assert_eq!(url_label(&c.url), c.label, "url_label drift for {}", c.url);
    }
}
