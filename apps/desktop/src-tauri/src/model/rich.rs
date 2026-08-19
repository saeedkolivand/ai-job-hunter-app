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
/// links render the same in the export as in the WYSIWYG editor. Case-insensitive
/// (`GitHub.com/user/repo` still links) — unlike [`BARE_DOMAIN_NO_PATH_RE`] below,
/// a path already disambiguates this from a capitalised library name.
static BARE_DOMAIN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\b(?:[a-z0-9-]+\.)+[a-z]{2,}/[^\s|·•,<>"']+"#).unwrap());

/// TLDs a bare domain with NO path may end in to be linked. Mirrors
/// `validate/content/factual.rs`'s `URL_RE` arm 5,
/// `BARE_DOMAIN_NO_PATH_TLDS` (`com|org|net|io|dev|app|de|co|ai|sh|me`) —
/// added there specifically so that guard can see the identical shape this
/// arm renders as a real link; the two MUST stay in lockstep or a domain this
/// arm links slips past that file's Critical-severity
/// `factual.altered_project_link` check. This is NOT the same list as that
/// file's OTHER bare-host arm (arm 3), which is gated by the narrower
/// `CODE_HOSTS` name allowlist plus a different TLD set
/// (`com|org|io|dev|net|rs`) rather than by TLD alone — kept as a sibling
/// literal rather than a cross-module import, since this module has no other
/// dependency on `validate`. Deliberately NOT an open-ended `[a-z]{2,}`
/// class: applied to a path-less bare domain that would link `node.js` (an
/// invalid `https://` host) and the `e.g`/`i.e`/`vs.` family right along with
/// it.
const BARE_DOMAIN_TLDS: &str = "com|org|net|io|dev|app|de|co|ai|sh|me";

/// A bare domain with NO path (`aijobhunter.app`, `iamsaeed.dev`) — a real
/// portfolio/product domain written without `https://` or a trailing path.
/// Case-SENSITIVE (the only arm in this file that is) and restricted to
/// [`BARE_DOMAIN_TLDS`]: without both gates this would also link every
/// capitalised library/product name that happens to be spelled with a dot —
/// `Socket.IO`, `Bun.sh`, `Nuxt.dev` — the exact collision
/// `validate/content/factual.rs` already documents for the identical
/// path-less-bare-host shape. Every real collision on record is a capitalised
/// proper noun; every real bare portfolio domain in the wild is lowercase, so
/// the case gate alone confines this arm to the domains it should link — a
/// capitalised real domain simply stays unlinked (no worse than before this
/// arm existed), and a lowercase library name links to its genuine official
/// homepage, which is the correct destination anyway.
static BARE_DOMAIN_NO_PATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\b(?:[a-z0-9-]+\.)+(?:{BARE_DOMAIN_TLDS})\b")).unwrap());

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

    // A bare domain with NO path (`aijobhunter.app`) — lowercase + allowlisted
    // TLD only, see BARE_DOMAIN_NO_PATH_RE's doc. Runs LAST — after
    // BARE_DOMAIN_RE so a domain that DOES have a path is never re-matched as
    // just its host, and after EMAIL_RE so `jane@example.com`'s host half
    // isn't sliced off as its own bare-domain link before the email arm ever
    // sees the whole address (`example.com` alone is a lowercase,
    // allowlisted-TLD match too).
    for m in BARE_DOMAIN_NO_PATH_RE.find_iter(text) {
        let url = m.as_str();
        let overlaps = matches
            .iter()
            .any(|u| m.start() < u.end && m.end() > u.start);
        if !overlaps {
            matches.push(Match {
                start: m.start(),
                end: m.end(),
                label: url.to_string(),
                url: format!("https://{url}"),
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

    /// Owner-reported: a contact line written as
    /// `[linkedin.com/in/x](https://linkedin.com/in/x)` (the shape a pasted
    /// bare link takes with no chosen label) must show the short brand label
    /// ("LinkedIn"), not the full URL text, while the link itself survives.
    #[test]
    fn tokenize_shortens_a_markdown_link_whose_label_is_the_bare_url() {
        let rt = tokenize_rich("[linkedin.com/in/jane](https://linkedin.com/in/jane)");
        assert_eq!(
            shape(&rt),
            vec![(
                "LinkedIn".to_string(),
                false,
                Some("https://linkedin.com/in/jane".to_string())
            )]
        );
    }

    /// Regression: an uppercase-prefixed bare-URL label (`HTTPS://…`,
    /// `WWW.…`) must still be recognized as bare and shortened to the brand
    /// label — the same shape as
    /// `tokenize_shortens_a_markdown_link_whose_label_is_the_bare_url` but
    /// with a differently-cased scheme/`www.` prefix on the label side only,
    /// which used to defeat `is_bare_url_label`'s comparison (it stripped
    /// case-sensitively before comparing case-insensitively, so a stripped
    /// url and an un-stripped, differently-cased label never matched).
    #[test]
    fn tokenize_shortens_a_markdown_link_whose_label_has_an_uppercase_prefix() {
        let rt = tokenize_rich("[HTTPS://linkedin.com/in/jane](https://linkedin.com/in/jane)");
        assert_eq!(
            shape(&rt),
            vec![(
                "LinkedIn".to_string(),
                false,
                Some("https://linkedin.com/in/jane".to_string())
            )]
        );

        let rt2 = tokenize_rich("[WWW.linkedin.com/in/jane](https://linkedin.com/in/jane)");
        assert_eq!(
            shape(&rt2),
            vec![(
                "LinkedIn".to_string(),
                false,
                Some("https://linkedin.com/in/jane".to_string())
            )]
        );
    }

    /// A deliberately CHOSEN label (not the bare URL) is never overwritten.
    #[test]
    fn tokenize_keeps_a_deliberately_chosen_markdown_link_label() {
        let rt = tokenize_rich("[My Portfolio](https://janedoe.dev/work)");
        assert_eq!(
            shape(&rt),
            vec![(
                "My Portfolio".to_string(),
                false,
                Some("https://janedoe.dev/work".to_string())
            )]
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

    /// The exact shape `pipeline::resume::project_render::render_project` emits for
    /// a project title line — a bold name followed by two labeled project
    /// links, `·`-separated. Both labels ("Website"/"Github") must render as
    /// the CLICKABLE TEXT, not the raw URL — this is what carries a source
    /// résumé's own link labels through to the PDF/DOCX export rather than
    /// falling back to the bare href.
    #[test]
    fn tokenize_renders_project_link_labels_as_the_hyperlink_text() {
        let rt = tokenize_rich(
            "**Ledger CLI** · [Website](https://ledger.example.dev) · \
             [Github](https://github.com/janedoe/ledger)",
        );
        assert_eq!(
            shape(&rt),
            vec![
                ("Ledger CLI".to_string(), true, None),
                (" · ".to_string(), false, None),
                (
                    "Website".to_string(),
                    false,
                    Some("https://ledger.example.dev".to_string())
                ),
                (" · ".to_string(), false, None),
                (
                    "Github".to_string(),
                    false,
                    Some("https://github.com/janedoe/ledger".to_string())
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

    /// INTENTIONAL BEHAVIOUR CHANGE, stated explicitly per the author-contract
    /// rule that editing a test to make a change pass must be justified in
    /// writing, not silently done. This test used to assert `split_urls("github.com")`
    /// stayed plain text — that was the reported bug: a bare portfolio domain
    /// with no path (`aijobhunter.app`, `iamsaeed.dev`) rendered as plain text
    /// while a GitHub URL one line below, which happened to carry a `/path`,
    /// rendered as a clickable link. `github.com` alone is lowercase on an
    /// allowlisted TLD ([`BARE_DOMAIN_NO_PATH_RE`]), so it is now linked too —
    /// a bare `github.com` written in a résumé is a real site, not a stray
    /// token. The other half of the original test — no-domain-dot tokens like
    /// `CI/CD`/`Agile`/`TDD` never becoming links — is unaffected by this arm
    /// and stays covered here for the same reason it always was.
    #[test]
    fn split_urls_links_a_bare_domain_without_path_on_an_allowlisted_tld() {
        match split_urls("github.com").as_slice() {
            [Span::Link { label, url }] => {
                assert_eq!(label, "github.com");
                assert_eq!(url, "https://github.com");
            }
            other => panic!("expected a single link span, got {other:?}"),
        }
    }

    #[test]
    fn split_urls_ignores_short_tld_tokens_with_no_domain_dot() {
        // "CI/CD" has no domain dot at all → never read as a URL, allowlist or not.
        assert!(matches!(
            split_urls("Agile, CI/CD, TDD").as_slice(),
            [Span::Text(_)]
        ));
    }

    /// The reported bug, verbatim: portfolio domains with no path must link
    /// exactly like GitHub project URLs with a path already do.
    #[test]
    fn split_urls_links_lowercase_bare_domains_on_allowlisted_tlds() {
        for (text, href) in [
            ("aijobhunter.app", "https://aijobhunter.app"),
            ("crosskit.iamsaeed.dev", "https://crosskit.iamsaeed.dev"),
            ("iamsaeed.dev", "https://iamsaeed.dev"),
        ] {
            match split_urls(text).as_slice() {
                [Span::Link { label, url }] => {
                    assert_eq!(label, text, "label for {text}");
                    assert_eq!(url, href, "href for {text}");
                }
                other => panic!("expected a single link span for {text}, got {other:?}"),
            }
        }
    }

    /// Every documented collision this arm has to avoid is a capitalised
    /// proper noun spelled with a dot — a tech-stack line's library names,
    /// never a portfolio domain. The case gate is what tells them apart.
    #[test]
    fn split_urls_leaves_capitalized_library_names_as_plain_text() {
        for text in ["Socket.IO", "Bun.sh", "Nuxt.dev", "Node.js"] {
            assert!(
                matches!(split_urls(text).as_slice(), [Span::Text(_)]),
                "{text} must stay plain text"
            );
        }
    }

    /// Isolates the TLD-allowlist gate from the case gate: these are already
    /// all-lowercase (so they'd clear a case check), but `.js` is not on
    /// [`BARE_DOMAIN_TLDS`]. Without an explicit allowlist, the old
    /// open-ended `[a-z]{2,}` class would treat any two-letter-plus suffix as
    /// a real TLD and link these to an invalid `https://node.js` host.
    #[test]
    fn split_urls_leaves_lowercase_names_with_unlisted_tlds_as_plain_text() {
        for text in ["node.js", "vue.js", "next.js"] {
            assert!(
                matches!(split_urls(text).as_slice(), [Span::Text(_)]),
                "{text} must stay plain text (unlisted TLD)"
            );
        }
    }

    /// The new no-path arm is case-SENSITIVE, but the pre-existing arms must
    /// behave exactly as they did before this change — this only gates the
    /// NEW arm. `BARE_DOMAIN_RE` (domain+path) is genuinely `(?i)`, so an
    /// uppercase-prefixed scheme-less URL with a path must still link;
    /// `FULL_URL_RE` (scheme-prefixed) was never case-insensitive on the
    /// scheme itself even before this change (no `(?i)` flag on that regex),
    /// so a lowercase `https://` URL is the fair regression check for it.
    #[test]
    fn split_urls_keeps_pre_existing_arms_unaffected() {
        match split_urls("GitHub.com/me/repo").as_slice() {
            [Span::Link { label, url }] => {
                assert_eq!(label, "GitHub.com/me/repo");
                assert_eq!(url, "https://GitHub.com/me/repo");
            }
            other => {
                panic!("an uppercase path-carrying scheme-less URL must still link, got {other:?}")
            }
        }

        let scheme = split_urls("see https://github.com/x today");
        assert!(
            scheme
                .iter()
                .any(|s| matches!(s, Span::Link { url, .. } if url == "https://github.com/x")),
            "a scheme-prefixed URL must still be linked verbatim"
        );
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

    /// Security regression: a lookalike host that merely STARTS WITH a known
    /// domain string must never borrow that domain's brand label — the
    /// résumé would render "LinkedIn" as trusted-looking display text while
    /// linking to an attacker-controlled host. A genuine subdomain
    /// (`www.linkedin.com`, already covered by
    /// `url_label_maps_known_domains`) must still resolve correctly.
    #[test]
    fn url_label_rejects_a_lookalike_host_prefix_match() {
        assert_eq!(
            url_label("https://linkedin.com.evil.example/path"),
            "linkedin.com.evil.example"
        );
        assert_eq!(
            url_label("https://github.com.attacker.io/repo"),
            "github.com.attacker.io"
        );
        // A real subdomain of a known domain still resolves to the brand.
        assert_eq!(url_label("https://gist.github.com/jane"), "GitHub");
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
