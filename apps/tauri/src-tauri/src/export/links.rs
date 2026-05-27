use regex::Regex;
use std::sync::LazyLock;

static FULL_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://[^\s|·•,<>"']+"#).unwrap()
});

static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap()
});

/// Matches post-processed markdown links: [LinkedIn](https://...) injected by
/// injectLinksIntoGeneratedText() so the label is displayed but the URL is clickable.
static MD_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[([^\]]+)\]\((https?://[^)]+)\)").unwrap()
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

/// Return the visible-only text — strips `[label](url)` → `label`.
/// Used for centering/width calculations so hidden URL bytes don't skew the estimate.
pub fn display_text(text: &str) -> std::borrow::Cow<'_, str> {
    MD_LINK_RE.replace_all(text, "$1")
}

/// Split a line of text into spans of plain text, hyperlinks, and email links.
/// URLs → Span::Link with friendly label; emails → Span::Link with mailto: href.
pub fn split_urls(text: &str) -> Vec<Span> {
    // Collect all matches (URLs and emails) sorted by start position.
    struct Match { start: usize, end: usize, label: String, url: String }

    let mut matches: Vec<Match> = Vec::new();

    // Markdown links [label](url) — injected by post-processing; take priority
    for cap in MD_LINK_RE.captures_iter(text) {
        let full = cap.get(0).unwrap();
        let label = cap[1].to_string();
        let url = cap[2].to_string();
        matches.push(Match {
            start: full.start(),
            end: full.end(),
            label,
            url,
        });
    }

    for m in FULL_URL_RE.find_iter(text) {
        let url = m.as_str().trim_end_matches(['.', ',', ')']);
        let overlaps = matches.iter().any(|u| m.start() < u.end && m.end() > u.start);
        if !overlaps {
            matches.push(Match {
                start: m.start(),
                end: m.start() + url.len(),
                label: url_label(url),
                url: url.to_string(),
            });
        }
    }

    for m in EMAIL_RE.find_iter(text) {
        let email = m.as_str();
        // Skip if this range overlaps an already-captured match
        let overlaps = matches.iter().any(|u| m.start() < u.end && m.end() > u.start);
        if !overlaps {
            matches.push(Match {
                start: m.start(),
                end: m.end(),
                label: email.to_string(),
                url: format!("mailto:{email}"),
            });
        }
    }

    matches.sort_by_key(|m| m.start);

    let mut spans = Vec::new();
    let mut last = 0;

    for m in &matches {
        if m.start > last {
            spans.push(Span::Text(text[last..m.start].to_string()));
        }
        spans.push(Span::Link { label: m.label.clone(), url: m.url.clone() });
        last = m.end;
    }

    if last < text.len() {
        spans.push(Span::Text(text[last..].to_string()));
    }

    if spans.is_empty() {
        spans.push(Span::Text(text.to_string()));
    }

    spans
}
