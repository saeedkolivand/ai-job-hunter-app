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
