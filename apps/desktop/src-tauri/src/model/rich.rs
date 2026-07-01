//! Canonical rich-text model: a single representation for inline formatting
//! (bold / italic) AND hyperlinks.
//!
//! This replaces the previously-separate `TextSegment{text,bold}` (formatting
//! only) and `Span` (links only) types, so header and body inline rendering can
//! share one codepath and body links become possible. The link helpers
//! (`url_label`, `split_urls`, `display_text`, `Span`) were moved here verbatim
//! from `export::links`, which now re-exports them as thin shims so existing
//! renderer imports keep compiling.

use std::sync::LazyLock;

use regex::Regex;

use crate::export::parser::parse_inline_md;

static FULL_URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^\s|·•,<>"']+"#).unwrap());

/// A scheme-less URL written out in body text: a domain WITH a path
/// (`github.com/user/repo`). Linked verbatim — the full text stays visible and an
/// `https://` scheme is added only for the hyperlink target — so résumé project
/// links render the same in the export as in the WYSIWYG editor. A bare domain
/// with no path, or a short-TLD token like `CI/CD`, is intentionally not matched.
static BARE_DOMAIN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\b(?:[a-z0-9-]+\.)+[a-z]{2,}/[^\s|·•,<>"']+"#).unwrap());

static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap());

/// Matches post-processed markdown links: [LinkedIn](https://...) injected by
/// injectLinksIntoGeneratedText() so the label is displayed but the URL is clickable.
static MD_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\((https?://[^)]+)\)").unwrap());

/// One run of inline text with uniform formatting and an optional hyperlink.
/// Unifies bold/italic (was `TextSegment`) with links (was `Span::Link`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    /// Hyperlink target (`http(s)://…` or `mailto:…`) when this run is a link.
    pub link: Option<String>,
}

/// A sequence of formatted runs forming one logical line / paragraph.
pub type RichText = Vec<TextRun>;

/// A span of text in a contact line — either plain text or a hyperlink.
/// Retained for the existing renderers; new code should prefer [`RichText`].
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

    if host.starts_with("linkedin.com") {
        return "LinkedIn".to_string();
    }
    if host.starts_with("github.com") {
        return "GitHub".to_string();
    }
    if host.starts_with("gitlab.com") {
        return "GitLab".to_string();
    }
    if host.starts_with("twitter.com") || host.starts_with("x.com") {
        return "Twitter".to_string();
    }
    if host.starts_with("behance.net") {
        return "Behance".to_string();
    }
    if host.starts_with("dribbble.com") {
        return "Dribbble".to_string();
    }
    if host.starts_with("medium.com") {
        return "Medium".to_string();
    }
    if host.starts_with("stackoverflow.com") {
        return "Stack Overflow".to_string();
    }
    if host.starts_with("dev.to") {
        return "Dev.to".to_string();
    }
    if host.starts_with("codepen.io") {
        return "CodePen".to_string();
    }
    if host.starts_with("youtube.com") || host.starts_with("youtu.be") {
        return "YouTube".to_string();
    }
    if host.starts_with("notion.so") {
        return "Notion".to_string();
    }
    if host.starts_with("figma.com") {
        return "Figma".to_string();
    }
    if host.starts_with("npmjs.com") {
        return "npm".to_string();
    }
    if host.starts_with("crates.io") {
        return "crates.io".to_string();
    }

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
    struct Match {
        start: usize,
        end: usize,
        label: String,
        url: String,
    }

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
        let overlaps = matches
            .iter()
            .any(|u| m.start() < u.end && m.end() > u.start);
        if !overlaps {
            matches.push(Match {
                start: m.start(),
                end: m.start() + url.len(),
                label: url_label(url),
                url: url.to_string(),
            });
        }
    }

    // Scheme-less project URLs (any domain): linked verbatim, scheme added only
    // for the href. Runs after FULL_URL_RE so a scheme-full URL isn't matched twice.
    for m in BARE_DOMAIN_RE.find_iter(text) {
        let url = m.as_str().trim_end_matches(['.', ',', ')']);
        let end = m.start() + url.len();
        let overlaps = matches.iter().any(|u| m.start() < u.end && end > u.start);
        if !overlaps {
            matches.push(Match {
                start: m.start(),
                end,
                label: url.to_string(),
                url: format!("https://{url}"),
            });
        }
    }

    for m in EMAIL_RE.find_iter(text) {
        let email = m.as_str();
        // Skip if this range overlaps an already-captured match
        let overlaps = matches
            .iter()
            .any(|u| m.start() < u.end && m.end() > u.start);
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
        spans.push(Span::Link {
            label: m.label.clone(),
            url: m.url.clone(),
        });
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

/// Tokenize one line into [`RichText`], merging bold (`**…**`), markdown links
/// (`[label](url)`), bare URLs and emails into a single sequence of runs.
///
/// Links are resolved first (via [`split_urls`], so `[label](url)` is matched
/// before `**` stripping could split it apart), then bold is parsed within each
/// plain-text span and within link labels. Italic is not emitted yet — the
/// markdown parser only recognizes bold today; the field exists for forward
/// compatibility.
pub fn tokenize_rich(line: &str) -> RichText {
    let mut runs: RichText = Vec::new();
    for span in split_urls(line) {
        match span {
            Span::Text(t) => {
                for seg in parse_inline_md(&t) {
                    runs.push(TextRun {
                        text: seg.text,
                        bold: seg.bold,
                        italic: false,
                        link: None,
                    });
                }
            }
            Span::Link { label, url } => {
                // A link label may itself contain bold, e.g. `[**Site**](url)`.
                let segs = parse_inline_md(&label);
                if segs.is_empty() {
                    runs.push(TextRun {
                        text: label,
                        bold: false,
                        italic: false,
                        link: Some(url),
                    });
                } else {
                    for seg in segs {
                        runs.push(TextRun {
                            text: seg.text,
                            bold: seg.bold,
                            italic: false,
                            link: Some(url.clone()),
                        });
                    }
                }
            }
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience: collapse a RichText into (text, bold, link) tuples for asserts.
    fn shape(rt: &RichText) -> Vec<(String, bool, Option<String>)> {
        rt.iter()
            .map(|r| (r.text.clone(), r.bold, r.link.clone()))
            .collect()
    }

    #[test]
    fn tokenize_plain_text_is_one_run() {
        let rt = tokenize_rich("just plain text");
        assert_eq!(
            shape(&rt),
            vec![("just plain text".to_string(), false, None)]
        );
    }

    #[test]
    fn tokenize_parses_bold_segments() {
        let rt = tokenize_rich("Built **React** and **Rust** apps");
        assert_eq!(
            shape(&rt),
            vec![
                ("Built ".to_string(), false, None),
                ("React".to_string(), true, None),
                (" and ".to_string(), false, None),
                ("Rust".to_string(), true, None),
                (" apps".to_string(), false, None),
            ]
        );
    }

    #[test]
    fn tokenize_keeps_markdown_links_as_link_runs() {
        let rt = tokenize_rich("See [GitHub](https://github.com/jane) today");
        assert_eq!(
            shape(&rt),
            vec![
                ("See ".to_string(), false, None),
                (
                    "GitHub".to_string(),
                    false,
                    Some("https://github.com/jane".to_string())
                ),
                (" today".to_string(), false, None),
            ]
        );
    }

    #[test]
    fn tokenize_labels_bare_urls_and_emails() {
        let url = tokenize_rich("visit https://janedoe.dev now");
        assert_eq!(
            shape(&url),
            vec![
                ("visit ".to_string(), false, None),
                (
                    "janedoe.dev".to_string(),
                    false,
                    Some("https://janedoe.dev".to_string())
                ),
                (" now".to_string(), false, None),
            ]
        );

        let email = tokenize_rich("reach me at jane@example.com");
        assert_eq!(
            shape(&email),
            vec![
                ("reach me at ".to_string(), false, None),
                (
                    "jane@example.com".to_string(),
                    false,
                    Some("mailto:jane@example.com".to_string())
                ),
            ]
        );
    }

    #[test]
    fn tokenize_merges_bold_and_links_in_one_line() {
        let rt = tokenize_rich("**Lead** — [LinkedIn](https://linkedin.com/in/x)");
        assert_eq!(
            shape(&rt),
            vec![
                ("Lead".to_string(), true, None),
                (" — ".to_string(), false, None),
                (
                    "LinkedIn".to_string(),
                    false,
                    Some("https://linkedin.com/in/x".to_string())
                ),
            ]
        );
    }

    #[test]
    fn tokenize_parses_bold_inside_a_link_label() {
        let rt = tokenize_rich("[**Site**](https://janedoe.dev)");
        assert_eq!(
            shape(&rt),
            vec![(
                "Site".to_string(),
                true,
                Some("https://janedoe.dev".to_string())
            )]
        );
    }

    #[test]
    fn split_urls_links_scheme_less_project_urls_for_any_domain() {
        for (text, label, href) in [
            (
                "github.com/me/repo",
                "github.com/me/repo",
                "https://github.com/me/repo",
            ),
            (
                "gitlab.com/me/proj",
                "gitlab.com/me/proj",
                "https://gitlab.com/me/proj",
            ),
            (
                "behance.net/me/case",
                "behance.net/me/case",
                "https://behance.net/me/case",
            ),
            (
                "my-site.dev/work/x",
                "my-site.dev/work/x",
                "https://my-site.dev/work/x",
            ),
        ] {
            let spans = split_urls(text);
            assert_eq!(spans.len(), 1, "expected one span for {text}");
            match &spans[0] {
                Span::Link { label: l, url: u } => {
                    assert_eq!(l.as_str(), label, "label for {text}");
                    assert_eq!(u.as_str(), href, "href for {text}");
                }
                _ => panic!("expected a link span for {text}"),
            }
        }
    }

    #[test]
    fn split_urls_ignores_bare_domain_without_path_and_short_tld_tokens() {
        // No path → not a project link; "CI/CD" has no domain dot → not a URL.
        assert!(matches!(
            split_urls("github.com").as_slice(),
            [Span::Text(_)]
        ));
        assert!(matches!(
            split_urls("Agile, CI/CD, TDD").as_slice(),
            [Span::Text(_)]
        ));
    }

    // ── Link helpers (moved here with their implementation from export::links) ──

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
        assert_eq!(
            url_label("https://www.example.com/path/to/page"),
            "example.com"
        );
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
        let has_mailto = spans
            .iter()
            .any(|s| matches!(s, Span::Link { url, .. } if url == "mailto:jane@example.com"));
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
        let raw = std::fs::read_to_string(&path).expect(
            "read url-labels parity fixture (packages/prompts/src/fixtures/url-labels.json)",
        );
        let cases: Vec<Case> = serde_json::from_str(&raw).expect("parse url-labels parity fixture");

        assert!(
            !cases.is_empty(),
            "url-labels parity fixture must not be empty"
        );
        for c in &cases {
            assert_eq!(url_label(&c.url), c.label, "url_label drift for {}", c.url);
        }
    }
}
