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

/// Known domain → friendly brand label. Matched against the bare host with
/// [`host_matches_domain`] (exact or `.`-boundary suffix), never a raw prefix
/// — a prefix match would let `linkedin.com.evil.example` borrow the
/// "LinkedIn" label while linking somewhere else entirely.
const KNOWN_DOMAINS: &[(&str, &str)] = &[
    ("linkedin.com", "LinkedIn"),
    ("github.com", "GitHub"),
    ("gitlab.com", "GitLab"),
    ("twitter.com", "Twitter"),
    ("x.com", "Twitter"),
    ("behance.net", "Behance"),
    ("dribbble.com", "Dribbble"),
    ("medium.com", "Medium"),
    ("stackoverflow.com", "Stack Overflow"),
    ("dev.to", "Dev.to"),
    ("codepen.io", "CodePen"),
    ("youtube.com", "YouTube"),
    ("youtu.be", "YouTube"),
    ("notion.so", "Notion"),
    ("figma.com", "Figma"),
    ("npmjs.com", "npm"),
    ("crates.io", "crates.io"),
];

/// True when `host` (already scheme/`www.`-stripped, path still attached) IS
/// `domain`, or is a genuine subdomain of it (`gist.github.com` matches
/// `github.com`). A lookalike host that merely starts with `domain` as a
/// string prefix — `linkedin.com.evil.example` — does NOT match, because the
/// character right after `domain` there is `.` only in the sense that it
/// extends the SAME label rather than opening a new DNS component boundary;
/// concretely: `domain` must be the whole host, or preceded by a literal `.`.
fn host_matches_domain(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Map a URL to a friendly display label.
/// Known domains get a brand name; everything else gets the bare domain (stripped of www.).
///
/// EXCEPTION: a `github.com`/`gitlab.com` URL with a REPO path (`user/repo`,
/// 2+ path segments) keeps the informative `domain/path` instead of collapsing
/// to "GitHub"/"GitLab" — the path IS the point of a project link (which repo),
/// unlike a bare `/user` PROFILE link (still 1 segment, still shortened below).
/// Mirrors the same profile-vs-deep-link distinction `is_profile_shaped` in
/// `contact_profile::mod` already draws for the SAME two hosts (this module
/// sits below `contact_profile` in the dependency graph, so the check is
/// re-derived here rather than shared).
pub fn url_label(url: &str) -> String {
    let lower = url.to_lowercase();
    // Strip protocol for matching, then isolate the host from any path so a
    // domain check never sees `linkedin.com/in/jane` and never mistakes a
    // path/query segment for part of the host.
    let host = lower
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    let domain = host.split('/').next().unwrap_or(host);

    if matches!(domain, "github.com" | "gitlab.com") {
        let path = host.strip_prefix(domain).unwrap_or("");
        // Query and fragment are not part of the repository's identity, and this
        // label is printed verbatim on the résumé — without this,
        // `github.com/user/repo?tab=readme` and `…/repo#install` end up on the
        // page, and the query text also inflates the last path segment.
        let path = path.split(['?', '#']).next().unwrap_or(path);
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() >= 2 {
            return format!("{domain}/{}", segments.join("/"));
        }
    }

    for (known, label) in KNOWN_DOMAINS {
        if host_matches_domain(domain, known) {
            return (*label).to_string();
        }
    }

    // Unknown domain: bare domain (scheme/www./path already stripped above).
    domain.to_string()
}

/// True when `label` is just the bare URL text with no friendly wrapping —
/// the shape a `[label](url)` link takes when the label was typed/pasted as
/// the raw link rather than chosen (e.g. `[linkedin.com/in/x](https://…)`).
/// Compares both sides with scheme/`www.` stripped so
/// `[www.linkedin.com/in/x](https://linkedin.com/in/x)` counts too.
fn is_bare_url_label(label: &str, url: &str) -> bool {
    // Lowercase BEFORE stripping: `trim_start_matches` is a literal byte
    // match, so an `HTTPS://`/`WWW.`-prefixed label would otherwise survive
    // untouched while the other side got stripped, and the two would never
    // compare equal — defeating the whole bare-label check.
    fn bare(s: &str) -> String {
        s.to_lowercase()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .to_string()
    }
    bare(label) == bare(url)
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

    // Markdown links [label](url) — injected by post-processing; take
    // priority. When the label is just the bare URL text (no friendly
    // wrapping — e.g. a contact line written as
    // `[linkedin.com/in/x](https://linkedin.com/in/x)`), swap in
    // `url_label`'s short brand/domain form so the header shows "LinkedIn",
    // not the full URL, while the link target is unaffected.
    for cap in MD_LINK_RE.captures_iter(text) {
        let full = cap.get(0).unwrap();
        let label = cap[1].to_string();
        let url = cap[2].to_string();
        let label = if is_bare_url_label(&label, &url) {
            url_label(&url)
        } else {
            label
        };
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
mod tests;
