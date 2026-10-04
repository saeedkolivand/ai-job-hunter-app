use super::*;

/// Second untested edge: `header_urls()` shares the exact same fix (both are
/// `sanitize_link_url` call sites) — the boundary case above must produce
/// byte-identical output there too, not just in `header_markdown`.
#[test]
fn header_urls_never_truncates_a_percent_escape_at_the_raw_cap_boundary_either() {
    let filler = "a".repeat(179);
    let p = ContactProfile {
        website: Some(format!("https://example.dev/{filler}()MORE")),
        ..Default::default()
    };
    let expected_url = format!("https://example.dev/{filler}%28");
    assert_eq!(p.header_urls(), vec![expected_url]);
}

// ── header_urls() ↔ header_markdown() sanitization lockstep (security review) ─
//
// `header_urls()` is the sole input to `validate::pdf_render_issues`'s
// `allowed` set (compared via `canonicalize_url`, which does not strip
// control characters). Any sanitization `header_markdown` applies but
// `header_urls` doesn't means the ACTUALLY-rendered (sanitized) link fails
// set membership against the UNSANITIZED "expected" entry — a legitimate
// profile then hard-fails `header_url_mismatch` (CRITICAL, blocking) on its
// own, unmodified link. The reverse (an entry `header_urls` lists that
// `header_markdown` would never render) causes a spurious, non-blocking
// `header_url_missing`.

/// A control character in a URL/email must be sanitized identically by both
/// functions, so the genuinely-rendered link is exactly what validation
/// expects — not a lookalike that fails set membership.
#[test]
fn header_urls_sanitizes_control_characters_like_header_markdown() {
    let p = ContactProfile {
        email: Some("alex@example.com\nEDUCATION".into()),
        website: Some("https://example.dev/site\nEXPERIENCE".into()),
        ..Default::default()
    };
    let urls = p.header_urls();
    assert!(
        urls.contains(&"mailto:alex@example.comEDUCATION".to_string()),
        "{urls:?}"
    );
    assert!(
        urls.contains(&"https://example.dev/siteEXPERIENCE".to_string()),
        "{urls:?}"
    );
    // What header_urls lists as "the profile's own link" must be exactly what
    // header_markdown actually renders.
    let md = p.header_markdown("en");
    assert!(md.contains("[Website](https://example.dev/siteEXPERIENCE)"));
}

/// An unsafe-scheme URL must never appear in `header_urls()` — `header_markdown`
/// drops it entirely, so it renders nothing; a phantom entry here would cause
/// a spurious `header_url_missing` warning for a link that could never exist
/// in the rendered PDF.
#[test]
fn header_urls_drops_unsafe_url_schemes_like_header_markdown() {
    let p = ContactProfile {
        linkedin: Some("javascript:alert(1)".into()),
        github: Some("data:text/html,<script>alert(1)</script>".into()),
        website: Some("https://example.dev/site".into()),
        extra_links: vec![ContactLink {
            label: "Evil".into(),
            url: "javascript:alert(2)".into(),
        }],
        ..Default::default()
    };
    assert_eq!(
        p.header_urls(),
        vec!["https://example.dev/site".to_string()]
    );
}

/// `mailto:` is dropped for a named link field (matches `header_markdown`'s
/// scheme allowlist).
#[test]
fn header_urls_drops_mailto_scheme_for_a_named_link() {
    let p = ContactProfile {
        website: Some("mailto:alex@example.com".into()),
        ..Default::default()
    };
    assert_eq!(p.header_urls(), Vec::<String>::new());
}

/// The bracket-stripping in `header_markdown` must apply identically in
/// `header_urls`, or the genuinely-rendered (bracket-stripped) URL fails set
/// membership against a differently-sanitized "expected" entry, firing
/// `header_url_mismatch` on an unmodified profile.
#[test]
fn header_urls_strips_link_breaking_brackets_like_header_markdown() {
    let p = ContactProfile {
        website: Some("https://example.dev/site)[EXPERIENCE](https://evil.example".into()),
        ..Default::default()
    };
    let md = p.header_markdown("en");
    let rendered_url = md
        .strip_prefix("[Website](")
        .and_then(|s| s.strip_suffix(')'))
        .expect("well-formed [Website](url) part");
    assert_eq!(
        p.header_urls(),
        vec![rendered_url.to_string()],
        "header_urls() must report the exact same bracket-stripped URL header_markdown renders"
    );
}

/// A URL long enough that the 200-char sanitization cap engages must be
/// capped IDENTICALLY by both methods. Capping the FORMATTED `[Label](url)`
/// string (rather than the bare URL, before formatting) truncates away the
/// closing `)` for a long-but-legitimate URL (tracking params, a long slug),
/// producing a malformed link `header_urls`'s bare-URL cap would never
/// reproduce — the genuinely-rendered (truncated) link then fails set
/// membership against `header_urls`' differently-capped entry, firing
/// `header_url_mismatch` (CRITICAL, blocking) on an unmodified profile.
#[test]
fn header_urls_and_header_markdown_cap_a_long_url_identically() {
    let long_url = format!("https://example.dev/profile?tracking={}", "a".repeat(250));
    assert!(long_url.len() > 200, "test setup: URL must exceed the cap");
    let p = ContactProfile {
        website: Some(long_url),
        ..Default::default()
    };

    let md = p.header_markdown("en");
    assert!(
        md.ends_with(')'),
        "the formatted link must not be truncated mid-URL, losing the closing \
         paren: {md:?}"
    );
    let rendered_url = md
        .strip_prefix("[Website](")
        .and_then(|s| s.strip_suffix(')'))
        .expect("well-formed [Website](url) part");
    assert!(rendered_url.len() <= 200);

    assert_eq!(
        p.header_urls(),
        vec![rendered_url.to_string()],
        "header_urls() must report the exact same (capped) URL header_markdown renders"
    );
}

/// Same lockstep guarantee for the email → `mailto:` link specifically (a
/// distinct code path: `header_urls` wraps `mailto:` around the bare email,
/// `header_markdown` never adds a scheme prefix at all — the renderer's own
/// `tokenize_rich`/`split_urls` auto-detects the bare email and links it).
#[test]
fn header_urls_and_header_markdown_cap_a_long_email_identically() {
    let long_email = format!("{}@example.com", "a".repeat(250));
    assert!(
        long_email.len() > 200,
        "test setup: email must exceed the cap"
    );
    let p = ContactProfile {
        email: Some(long_email),
        ..Default::default()
    };

    let md = p.header_markdown("en");
    let urls = p.header_urls();
    assert_eq!(urls.len(), 1);
    let capped_email = urls[0]
        .strip_prefix("mailto:")
        .expect("mailto: prefix on the email entry");

    assert_eq!(
        md, capped_email,
        "header_markdown's rendered (capped) email must be byte-identical to \
         the email header_urls() reports under mailto:"
    );
}
