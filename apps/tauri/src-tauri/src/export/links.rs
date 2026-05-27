use regex::Regex;
use std::sync::LazyLock;

static FULL_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://[^\s|·•,<>"']+"#).unwrap()
});

/// A span of text in a contact line — either plain text or a hyperlink.
#[derive(Debug, Clone)]
pub enum Span {
    Text(String),
    Link { label: String, url: String },
}

/// Map a URL to a friendly display label.
/// Known domains get a brand name; everything else gets the bare domain (stripped of www.).
pub fn url_label(url: &str) -> String {
    let lower = url.to_lowercase();
    // Strip protocol for matching
    let host = lower
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");

    if host.starts_with("linkedin.com") { return "LinkedIn".to_string(); }
    if host.starts_with("github.com") { return "GitHub".to_string(); }
    if host.starts_with("gitlab.com") { return "GitLab".to_string(); }
    if host.starts_with("twitter.com") || host.starts_with("x.com") { return "Twitter".to_string(); }
    if host.starts_with("behance.net") { return "Behance".to_string(); }
    if host.starts_with("dribbble.com") { return "Dribbble".to_string(); }
    if host.starts_with("medium.com") { return "Medium".to_string(); }
    if host.starts_with("stackoverflow.com") { return "Stack Overflow".to_string(); }
    if host.starts_with("dev.to") { return "Dev.to".to_string(); }
    if host.starts_with("codepen.io") { return "CodePen".to_string(); }
    if host.starts_with("youtube.com") || host.starts_with("youtu.be") { return "YouTube".to_string(); }
    if host.starts_with("notion.so") { return "Notion".to_string(); }
    if host.starts_with("figma.com") { return "Figma".to_string(); }
    if host.starts_with("npmjs.com") { return "npm".to_string(); }
    if host.starts_with("crates.io") { return "crates.io".to_string(); }

    // Unknown domain: strip www. and use bare domain up to the first /
    let domain = host.split('/').next().unwrap_or(host);
    domain.to_string()
}

/// Split a line of text into spans of plain text and hyperlinks.
pub fn split_urls(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut last = 0;

    for m in FULL_URL_RE.find_iter(text) {
        if m.start() > last {
            spans.push(Span::Text(text[last..m.start()].to_string()));
        }
        let url = m.as_str().trim_end_matches(['.', ',', ')']);
        spans.push(Span::Link {
            label: url_label(url),
            url: url.to_string(),
        });
        last = m.start() + url.len();
    }

    if last < text.len() {
        spans.push(Span::Text(text[last..].to_string()));
    }

    if spans.is_empty() {
        spans.push(Span::Text(text.to_string()));
    }

    spans
}
