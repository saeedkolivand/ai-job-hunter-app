use super::*;

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

/// Owner-reported: a GitHub/GitLab PROJECT link (a specific repo, not the
/// candidate's profile) must keep the full `domain/user/repo` text — the
/// reference résumé style ("github.com/saeedkolivand/ai-job-hunter-app")
/// — rather than collapsing to the generic "GitHub"/"GitLab" brand label,
/// which loses exactly the information a repo link exists to carry.
#[test]
fn url_label_keeps_the_full_path_for_a_github_repo_link() {
    assert_eq!(
        url_label("https://github.com/saeedkolivand/ai-job-hunter-app"),
        "github.com/saeedkolivand/ai-job-hunter-app"
    );
    assert_eq!(
        url_label("https://gitlab.com/janedoe/my-project"),
        "gitlab.com/janedoe/my-project"
    );
    // www./scheme are still stripped the same way as every other case.
    assert_eq!(
        url_label("https://www.github.com/janedoe/my-project"),
        "github.com/janedoe/my-project"
    );
    // A query or fragment is not part of the repository's identity, and this
    // label is printed verbatim on the résumé — neither may leak into it.
    assert_eq!(
        url_label("https://github.com/user/repo?tab=readme"),
        "github.com/user/repo"
    );
    assert_eq!(
        url_label("https://github.com/user/repo#installation"),
        "github.com/user/repo"
    );
    assert_eq!(
        url_label("https://gitlab.com/user/repo/?ref=nav#top"),
        "gitlab.com/user/repo"
    );
    // A query on a PROFILE link must not manufacture a second segment and
    // promote it to a repo-shaped label.
    assert_eq!(
        url_label("https://github.com/janedoe?tab=repositories"),
        "GitHub"
    );
}

/// A bare GitHub/GitLab PROFILE link (one path segment: `/user`, no repo)
/// is unaffected — still the short brand label, exactly as before.
#[test]
fn url_label_still_shortens_a_github_profile_link() {
    assert_eq!(url_label("https://github.com/jane"), "GitHub");
    assert_eq!(url_label("https://gitlab.com/jane"), "GitLab");
    assert_eq!(url_label("https://github.com/jane/"), "GitHub");
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
    // urlToFriendlyLabel() test in packages/prompts/src/generate/links/links.test.ts. Both read
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

    assert!(
        !cases.is_empty(),
        "url-labels parity fixture must not be empty"
    );
    for c in &cases {
        assert_eq!(url_label(&c.url), c.label, "url_label drift for {}", c.url);
    }
}
