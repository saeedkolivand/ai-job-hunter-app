use super::*;
use crate::extraction::types::Link;

fn link(url: &str) -> Link {
    Link {
        anchor_text: url.to_string(),
        url: url.to_string(),
    }
}

#[test]
fn header_markdown_uses_named_fields_in_canonical_order() {
    let p = ContactProfile {
        full_name: Some("Alex Carter".into()),
        email: Some("alex.carter@example.com".into()),
        phone: Some("+31 6 12345678".into()),
        location: Some(LocalizedText {
            default: "Netherlands".into(),
            by_lang: [("de".to_string(), "Niederlande".to_string())].into(),
        }),
        linkedin: Some("https://www.linkedin.com/in/alex-carter/".into()),
        github: Some("https://github.com/alexcarter".into()),
        website: Some("https://solo.to/alexc".into()),
        extra_links: vec![],
        photo: None,
    };

    // German doc: localized location, canonical order.
    assert_eq!(
        p.header_markdown("de"),
        "Niederlande | alex.carter@example.com | +31 6 12345678 | \
         [LinkedIn](https://www.linkedin.com/in/alex-carter/) | \
         [GitHub](https://github.com/alexcarter) | [Website](https://solo.to/alexc)"
    );
    // English doc: default location.
    assert!(p.header_markdown("en").starts_with("Netherlands | "));
}

/// `header_markdown`'s output is spliced verbatim into plain, `\n`-split
/// document text (H's header-seeding path in the renderer), so a control
/// character in any field — reachable via lenient upstream URL classification
/// / `.trim()`-only import merging, not just direct user typing — must never
/// survive into the joined string. A raw `\n` would otherwise inject an
/// arbitrary extra physical line, including a well-formed section heading.
#[test]
fn header_markdown_strips_control_characters_from_every_part() {
    let p = ContactProfile {
        location: Some(LocalizedText {
            default: "Berlin\nSKILLS\nRust (injected)".into(),
            ..Default::default()
        }),
        email: Some("alex@example.com\r\nEDUCATION".into()),
        website: Some("https://example.dev/site\nEXPERIENCE".into()),
        ..Default::default()
    };
    let md = p.header_markdown("en");
    assert!(
        !md.contains('\n') && !md.contains('\r'),
        "no control character may survive into the joined header line: {md:?}"
    );
    assert!(md.contains("BerlinSKILLSRust (injected)"));
    assert!(md.contains("alex@example.comEDUCATION"));
    assert!(md.contains("[Website](https://example.dev/siteEXPERIENCE)"));
}

/// LOW (security re-review): a Unicode Format (`Cf`) character — a bidi
/// override (`RIGHT-TO-LEFT OVERRIDE`, U+202E) above all — must be stripped
/// too, not just `char::is_control()`'s `Cc` category. Left in, a bidi
/// override embedded in a name/location could visually REVERSE the
/// surrounding rendered header text.
#[test]
fn header_markdown_strips_bidi_override_characters() {
    let p = ContactProfile {
        location: Some(LocalizedText {
            default: "Berlin\u{202E}nilreB".into(), // U+202E RIGHT-TO-LEFT OVERRIDE
            ..Default::default()
        }),
        ..Default::default()
    };
    let md = p.header_markdown("en");
    assert_eq!(md, "BerlinnilreB");
    assert!(!md.contains('\u{202E}'));
}

/// A non-`http(s)`/`mailto:` scheme (`javascript:`, `data:`, …) must never
/// reach the header as a clickable link, however lenient the upstream URL
/// classifier / import-merge path is about accepting it into the profile.
#[test]
fn header_markdown_drops_unsafe_url_schemes() {
    let p = ContactProfile {
        email: Some("alex@example.com".into()),
        linkedin: Some("javascript:alert(1)".into()),
        github: Some("data:text/html,<script>alert(1)</script>".into()),
        website: Some("https://example.dev/site".into()),
        extra_links: vec![ContactLink {
            label: "Evil".into(),
            url: "javascript:alert(2)".into(),
        }],
        ..Default::default()
    };
    let md = p.header_markdown("en");
    assert_eq!(md, "alex@example.com | [Website](https://example.dev/site)");
}

/// MEDIUM (security re-review): `mailto:` is DROPPED, not allowed, for a
/// named link field — `model::rich::MD_LINK_RE` (the downstream matcher that
/// turns `[Label](url)` markdown back into a real clickable link) only
/// recognizes an `http(s)://` URL group, never `mailto:`. A `mailto:`-valued
/// Website used to render as literal, unlinked `[Website](mailto:…)`
/// markdown text — `EMAIL_RE` still auto-linked the bare address buried
/// inside it, but the surrounding brackets/parens stayed visible as text.
/// Same shape as the javascript:/data: rejection above; proven against the
/// actual `tokenize_rich` output below, not just the markdown string.
#[test]
fn header_markdown_drops_mailto_scheme_for_a_named_link() {
    let p = ContactProfile {
        email: Some("alex@example.com".into()),
        website: Some("mailto:alex@example.com".into()),
        ..Default::default()
    };
    let md = p.header_markdown("en");
    assert_eq!(md, "alex@example.com");
    let rich = tokenize_rich(&md);
    assert_eq!(
        rich.len(),
        1,
        "must render as ONE clean run, never literal [Website](mailto:…) text: {rich:?}"
    );
    assert_eq!(rich[0].link.as_deref(), Some("mailto:alex@example.com"));
    assert_eq!(rich[0].text, "alex@example.com");
}

/// A pathologically long field is capped rather than left to balloon the
/// header line (and, once spliced into `generateResume`'s output, the whole
/// document).
#[test]
fn header_markdown_caps_an_overlong_part() {
    let p = ContactProfile {
        email: Some("a".repeat(500)),
        ..Default::default()
    };
    let md = p.header_markdown("en");
    assert_eq!(md.len(), 200);
}

/// A `[`, `]`, `(`, or `)` in a URL/label that ends up inside a `[Label](url)`
/// construct must never survive as a literal byte, not just control
/// characters — those four characters could otherwise close the link early
/// or open a second one. `is_safe_header_url` only checks the scheme prefix,
/// so an `https://`-prefixed value still carries the payload past it. `[`/`]`
/// are dropped on both sides; `(`/`)` are dropped for a label but
/// PERCENT-ENCODED for a URL (see the next test for why).
#[test]
fn header_markdown_strips_link_breaking_brackets_from_url_and_label() {
    let p = ContactProfile {
        website: Some("https://example.dev/site)[EXPERIENCE](https://evil.example".into()),
        extra_links: vec![ContactLink {
            label: "Real](url)[Fake".into(),
            url: "https://example.dev/extra)[EXPERIENCE](https://evil.example".into(),
        }],
        ..Default::default()
    };
    // MEDIUM (security re-review): `(`/`)` are PERCENT-ENCODED in a URL, not
    // deleted — deleting them (as the label sanitizer still does) would
    // corrupt a legitimate paren-bearing URL into a different destination.
    // `%28`/`%29` decode back to the exact same URL while still removing the
    // literal byte that could close the markdown construct early. `[`/`]`
    // stay dropped on both sides (label AND url).
    assert_eq!(
        p.header_markdown("en"),
        "[Website](https://example.dev/site%29EXPERIENCE%28https://evil.example) | \
         [RealurlFake](https://example.dev/extra%29EXPERIENCE%28https://evil.example)"
    );
}

/// MEDIUM (security re-review): a legitimate paren-bearing URL (a
/// Wikipedia-style path segment is the canonical real-world example) must
/// still resolve to the SAME destination after sanitization — the sanitizer
/// must not corrupt it into a different URL just because it happens to
/// contain the same two characters a malicious value would abuse.
#[test]
fn header_markdown_percent_encodes_parens_in_a_legitimate_url_without_corrupting_it() {
    let p = ContactProfile {
        website: Some("https://en.wikipedia.org/wiki/Rust_(programming_language)".into()),
        ..Default::default()
    };
    assert_eq!(
        p.header_markdown("en"),
        "[Website](https://en.wikipedia.org/wiki/Rust_%28programming_language%29)"
    );
}

/// LOW (security re-review): cap the RAW value BEFORE percent-encoding, not
/// after — encoding EXPANDS (1 byte → 3), so capping the expanded string can
/// truncate mid-escape and leave a mangled `%2`/bare `%` tail. The `(` here
/// sits exactly at raw index 199 (the 200th raw char, the last one the
/// MAX_LEN=200 cap includes) — proves the fix: it is either fully included
/// (a whole `%28`) or fully excluded, never split. Everything after it (the
/// closing paren + more) falls past the raw cap and is dropped whole, never
/// half-encoded.
#[test]
fn header_markdown_never_truncates_a_percent_escape_at_the_raw_cap_boundary() {
    let filler = "a".repeat(179); // 20-char prefix + 179 = 199, so '(' lands at index 199
    let p = ContactProfile {
        website: Some(format!("https://example.dev/{filler}()MORE")),
        ..Default::default()
    };
    let md = p.header_markdown("en");
    let expected_url = format!("https://example.dev/{filler}%28");
    assert_eq!(md, format!("[Website]({expected_url})"));
    assert!(
        !md.contains(")MORE"),
        "content past the raw cap must not survive: {md:?}"
    );
    // No bare '%' or truncated escape anywhere in the output.
    for (i, c) in md.char_indices() {
        if c == '%' {
            assert!(
                md[i..].starts_with("%28") || md[i..].starts_with("%29"),
                "found a truncated percent escape at byte {i}: {md:?}"
            );
        }
    }
}

/// CodeRabbit (test-coverage re-review): the mirrored boundary case — the
/// `(` sits at raw index 200 (the FIRST char `take(MAX_LEN)` EXCLUDES, one
/// past the test above's inclusive boundary). It must be dropped whole, not
/// partially encoded — no `%2`/bare `%` fragment, and no bare `(` either.
#[test]
fn header_markdown_never_truncates_a_percent_escape_just_past_the_raw_cap_boundary() {
    let filler = "a".repeat(180); // 20-char prefix + 180 = 200, so '(' lands at index 200
    let p = ContactProfile {
        website: Some(format!("https://example.dev/{filler}()MORE")),
        ..Default::default()
    };
    let md = p.header_markdown("en");
    // Exact match: the paren just past the cap (and everything after it) is
    // dropped whole — no `%28`/`%29`, no bare `(`/`)`, no `%2`/`%` fragment
    // anywhere in the URL. If the cap-before-encode ordering ever regressed
    // back to encode-then-cap, this URL would instead grow to 200+ chars and
    // this exact-match assertion would fail immediately.
    let expected_url = format!("https://example.dev/{filler}");
    assert_eq!(md, format!("[Website]({expected_url})"));
}

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

#[test]
fn header_rich_makes_each_named_link_clickable_with_the_right_url() {
    let p = ContactProfile {
        email: Some("alex.carter@example.com".into()),
        linkedin: Some("https://www.linkedin.com/in/alex-carter/".into()),
        github: Some("https://github.com/alexcarter".into()),
        website: Some("https://solo.to/alexc".into()),
        ..Default::default()
    };
    let rich = p.header_rich("en");
    // The LinkedIn label is bound to the PERSONAL profile URL (not a company page).
    let linkedin = rich
        .iter()
        .find(|r| r.text == "LinkedIn")
        .expect("LinkedIn run");
    assert_eq!(
        linkedin.link.as_deref(),
        Some("https://www.linkedin.com/in/alex-carter/")
    );
    let website = rich.iter().find(|r| r.text == "Website").expect("Website");
    assert_eq!(website.link.as_deref(), Some("https://solo.to/alexc"));
    assert!(rich
        .iter()
        .any(|r| r.link.as_deref() == Some("mailto:alex.carter@example.com")));
}

#[test]
fn classify_picks_personal_links_and_rejects_company_pool() {
    // Mirrors the bug data set: a personal profile, a company page, an employer
    // site, plus the real personal site — in document order.
    let links = vec![
        link("https://www.linkedin.com/in/alex-carter/"),
        link("https://github.com/alexcarter"),
        link("https://www.linkedin.com/company/acme/about/"),
        link("http://example-employer.com"),
        link("https://solo.to/alexc"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(
        p.linkedin.as_deref(),
        Some("https://www.linkedin.com/in/alex-carter/"),
        "must pick the personal /in/ profile, never the /company/ page"
    );
    assert_eq!(p.github.as_deref(), Some("https://github.com/alexcarter"));
    assert_eq!(
        p.website.as_deref(),
        Some("https://solo.to/alexc"),
        "a known link-in-bio host wins the Website slot over an employer URL"
    );
}

#[test]
fn classify_does_not_use_a_job_board_as_website() {
    let links = vec![
        link("https://www.indeed.com/cmp/acme"),
        link("https://my-portfolio.dev"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(
        p.website.as_deref(),
        Some("https://my-portfolio.dev"),
        "job-board URL must be skipped; the real portfolio takes Website"
    );
}

#[test]
fn classify_extracts_mailto_email() {
    let links = vec![link("mailto:alex.carter@example.com")];
    let p = classify_contact_links(&links);
    assert_eq!(p.email.as_deref(), Some("alex.carter@example.com"));
}

#[test]
fn empty_profile_is_detected() {
    assert!(ContactProfile::default().is_effectively_empty());
    let only_name = ContactProfile {
        full_name: Some("x".into()),
        ..Default::default()
    };
    assert!(
        only_name.is_effectively_empty(),
        "name alone is not a header"
    );
}

#[test]
fn classify_keeps_other_personal_links_as_labelled_extras() {
    // A personal profile + a known website host + two portfolio links + a job board.
    let links = vec![
        link("https://www.linkedin.com/in/lena-vos/"),
        link("https://solo.to/lenavos"), // website-host → Website slot
        link("https://dribbble.com/lenavos"),
        link("https://www.behance.net/lenavos"),
        link("https://www.indeed.com/cmp/acme"), // job board → never surfaced
    ];
    let p = classify_contact_links(&links);
    assert_eq!(p.website.as_deref(), Some("https://solo.to/lenavos"));

    let labels: Vec<&str> = p.extra_links.iter().map(|e| e.label.as_str()).collect();
    assert!(labels.contains(&"Dribbble"), "extras = {labels:?}");
    assert!(labels.contains(&"Behance"), "extras = {labels:?}");
    assert!(
        !p.extra_links.iter().any(|e| e.url.contains("linkedin.com")
            || e.url.contains("solo.to")
            || e.url.contains("indeed.com")),
        "named fields and job boards must not leak into extras: {:?}",
        p.extra_links
    );
}

/// Project/repo/demo links must never leak into the contact profile, even
/// though they share a host with a genuine platform profile — only the
/// profile-shaped form (bare user page) qualifies. Mirrors `isProfileShaped`/
/// `classifyLinks` in `packages/prompts/src/generate/links/links.ts`.
#[test]
fn classify_excludes_deep_path_project_links_by_shape() {
    let links = vec![
        link("https://github.com/alice"),
        link("https://github.com/alice/my-project"),
        link("https://gitlab.com/alice"),
        link("https://gitlab.com/alice/my-project"),
        link("https://linkedin.com/in/alice"),
        link("https://linkedin.com/company/acme"),
        link("https://myapp.com/demo"),
        link("https://alice.dev"),
        link("https://dribbble.com/alice"),
    ];
    let p = classify_contact_links(&links);

    assert_eq!(p.github.as_deref(), Some("https://github.com/alice"));
    assert_eq!(p.linkedin.as_deref(), Some("https://linkedin.com/in/alice"));
    assert_eq!(p.website.as_deref(), Some("https://alice.dev"));

    let extra_urls: Vec<&str> = p.extra_links.iter().map(|e| e.url.as_str()).collect();
    assert!(
        extra_urls.contains(&"https://dribbble.com/alice"),
        "a platform profile must still seed extras: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://github.com/alice"),
        "a github profile promoted to the github field must not also appear in extras: {extra_urls:?}"
    );
    assert!(
        extra_urls.contains(&"https://gitlab.com/alice"),
        "a bare GitLab profile is profile-shaped and must seed extras: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://github.com/alice/my-project"),
        "a repo URL is a project reference, not an identity — must not leak: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://gitlab.com/alice/my-project"),
        "a GitLab repo URL is a project reference, not an identity — must not leak: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://linkedin.com/company/acme"),
        "a company page must never seed the header: {extra_urls:?}"
    );
    assert!(
        !extra_urls.contains(&"https://myapp.com/demo"),
        "a deep-path demo link must never seed the header: {extra_urls:?}"
    );
}

/// The bug data set: an apex domain, one of its own subdomains, an unrelated
/// bare-root project domain, and a GitHub user + one of that user's repos.
/// Only the apex reaches `website`; the unrelated bare-root domain and the
/// subdomain never enter the profile anywhere (not `website`, not
/// `extra_links`) since a personal bare-root domain that loses the website
/// slot is a body link, never a header link.
#[test]
fn website_prefers_apex_over_subdomain_and_rejected_domains_never_leak_to_extras() {
    let links = vec![
        link("https://apex.dev"),
        link("https://sub.apex.dev"),
        link("https://other.app"),
        link("https://github.com/u"),
        link("https://github.com/u/repo"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(p.website.as_deref(), Some("https://apex.dev"));
    assert_eq!(p.github.as_deref(), Some("https://github.com/u"));
    assert!(
        p.extra_links.is_empty(),
        "a rejected bare-root domain (sub.apex.dev, other.app) or a repo path \
         (github.com/u/repo) must never leak into extra_links: {:?}",
        p.extra_links
    );
}

/// The SAME input, order reversed — proves website selection is
/// order-independent: the apex/subdomain shape relationship decides the
/// winner, not raw document position.
#[test]
fn website_prefers_apex_over_subdomain_order_independent() {
    let links = vec![
        link("https://github.com/u/repo"),
        link("https://github.com/u"),
        link("https://other.app"),
        link("https://sub.apex.dev"),
        link("https://apex.dev"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(
        p.website.as_deref(),
        Some("https://apex.dev"),
        "apex must win regardless of document order"
    );
    assert_eq!(p.github.as_deref(), Some("https://github.com/u"));
    assert!(p.extra_links.is_empty(), "{:?}", p.extra_links);
}

/// A second GitHub user (not just a repo path under the first) is still a
/// genuine platform profile and must still surface as an extra — the
/// extras-are-platform-profiles-only tightening must not regress this
/// documented intent (same for Dribbble / Behance).
#[test]
fn second_platform_profile_still_becomes_extra_after_extras_tightening() {
    let links = vec![
        link("https://github.com/alice"),
        link("https://github.com/bob"),
        link("https://dribbble.com/alice"),
        link("https://www.behance.net/alice"),
    ];
    let p = classify_contact_links(&links);
    assert_eq!(p.github.as_deref(), Some("https://github.com/alice"));
    let extra_urls: Vec<&str> = p.extra_links.iter().map(|e| e.url.as_str()).collect();
    assert!(
        extra_urls.contains(&"https://github.com/bob"),
        "a second GitHub user must still become an extra: {extra_urls:?}"
    );
    let labels: Vec<&str> = p.extra_links.iter().map(|e| e.label.as_str()).collect();
    assert!(labels.contains(&"Dribbble"), "extras = {labels:?}");
    assert!(labels.contains(&"Behance"), "extras = {labels:?}");
}

#[test]
fn fill_empty_from_completes_sparse_profile_without_clobbering() {
    // The user edited only the website (their portfolio); an import suggests a full set.
    let mut current = ContactProfile {
        website: Some("https://my.portfolio/".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        email: Some("l@example.com".into()),
        phone: Some("+31 6 12345678".into()),
        location: Some(LocalizedText {
            default: "Amsterdam, Netherlands".into(),
            ..Default::default()
        }),
        linkedin: Some("https://www.linkedin.com/in/l/".into()),
        website: Some("https://drive.google.com/xyz".into()), // must NOT overwrite the user's
        extra_links: vec![ContactLink {
            label: "Dribbble".into(),
            url: "https://dribbble.com/l".into(),
        }],
        ..Default::default()
    };
    current.fill_empty_from(&suggested);

    assert_eq!(
        current.website.as_deref(),
        Some("https://my.portfolio/"),
        "a user-set field is never overwritten"
    );
    assert_eq!(current.email.as_deref(), Some("l@example.com"));
    assert_eq!(current.phone.as_deref(), Some("+31 6 12345678"));
    assert_eq!(
        current.location.as_ref().map(|l| l.default.as_str()),
        Some("Amsterdam, Netherlands")
    );
    assert_eq!(
        current.linkedin.as_deref(),
        Some("https://www.linkedin.com/in/l/")
    );
    assert!(current.extra_links.iter().any(|e| e.label == "Dribbble"));
}

#[test]
fn fill_empty_from_merges_extras_by_url_without_duplicates() {
    let mut current = ContactProfile {
        extra_links: vec![ContactLink {
            label: "Dribbble".into(),
            url: "https://dribbble.com/l".into(),
        }],
        ..Default::default()
    };
    let suggested = ContactProfile {
        extra_links: vec![
            ContactLink {
                label: "Dribbble".into(),
                url: "https://dribbble.com/l".into(), // duplicate by URL → skipped
            },
            ContactLink {
                label: "Behance".into(),
                url: "https://behance.net/l".into(),
            },
        ],
        ..Default::default()
    };
    current.fill_empty_from(&suggested);
    assert_eq!(
        current.extra_links.len(),
        2,
        "duplicate deduped, new extra added: {:?}",
        current.extra_links
    );
}

#[test]
fn localized_text_resolves_primary_subtag() {
    let loc = LocalizedText {
        default: "Netherlands".into(),
        by_lang: [("de".to_string(), "Niederlande".to_string())].into(),
    };
    assert_eq!(loc.resolve("de"), "Niederlande");
    assert_eq!(loc.resolve("de-DE"), "Niederlande");
    assert_eq!(loc.resolve("en"), "Netherlands");
    assert_eq!(loc.resolve("fr"), "Netherlands");
}

// ── detect_contact_conflicts — no-conflict (normalized-equal) cases ───────────

/// Same email differing only in case → normalized equal → no conflict.
#[test]
fn no_conflict_email_case_insensitive() {
    let current = ContactProfile {
        email: Some("Alex.Carter@Example.COM".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        email: Some("alex.carter@example.com".into()),
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "same email differing only by case must not produce a conflict"
    );
}

/// Same phone formatted differently → digits-only normalization → no conflict.
#[test]
fn no_conflict_phone_formatting_differences() {
    let current = ContactProfile {
        phone: Some("+1 (555) 123-4567".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        phone: Some("15551234567".into()),
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "same phone with different formatting must not produce a conflict"
    );
}

/// Same LinkedIn URL differing by scheme, www., and trailing slash → no conflict.
#[test]
fn no_conflict_url_scheme_www_trailing_slash() {
    let current = ContactProfile {
        linkedin: Some("https://www.linkedin.com/in/x/".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        linkedin: Some("http://linkedin.com/in/x".into()),
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "same URL differing only by scheme/www./trailing-slash must not produce a conflict"
    );
}

/// Same website URL differing by https vs http → no conflict.
#[test]
fn no_conflict_website_scheme_only() {
    let current = ContactProfile {
        website: Some("https://my-portfolio.dev/work".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        website: Some("http://my-portfolio.dev/work".into()),
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "same website URL differing only by http/https scheme must not produce a conflict"
    );
}

// ── detect_contact_conflicts — real conflict cases ────────────────────────────

/// Genuinely different email values → one conflict with correct field key and
/// original (un-normalized) current/suggested values.
#[test]
fn conflict_different_email() {
    let current = ContactProfile {
        email: Some("alice@example.com".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        email: Some("bob@example.com".into()),
        ..Default::default()
    };
    let conflicts = detect_contact_conflicts(&current, &suggested);
    assert_eq!(
        conflicts.len(),
        1,
        "expected exactly one conflict: {conflicts:?}"
    );
    let c = &conflicts[0];
    assert_eq!(c.field, "email");
    assert_eq!(c.current, "alice@example.com");
    assert_eq!(c.suggested, "bob@example.com");
}

/// Genuinely different phone numbers → one conflict with original values.
#[test]
fn conflict_different_phone() {
    let current = ContactProfile {
        phone: Some("+31 6 12345678".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        phone: Some("+1 (800) 555-0199".into()),
        ..Default::default()
    };
    let conflicts = detect_contact_conflicts(&current, &suggested);
    assert_eq!(
        conflicts.len(),
        1,
        "expected exactly one conflict: {conflicts:?}"
    );
    let c = &conflicts[0];
    assert_eq!(c.field, "phone");
    assert_eq!(c.current, "+31 6 12345678");
    assert_eq!(c.suggested, "+1 (800) 555-0199");
}

/// Genuinely different LinkedIn paths → one conflict.
#[test]
fn conflict_different_linkedin() {
    let current = ContactProfile {
        linkedin: Some("https://linkedin.com/in/alice".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        linkedin: Some("https://linkedin.com/in/bob".into()),
        ..Default::default()
    };
    let conflicts = detect_contact_conflicts(&current, &suggested);
    assert_eq!(
        conflicts.len(),
        1,
        "expected exactly one conflict: {conflicts:?}"
    );
    assert_eq!(conflicts[0].field, "linkedin");
    assert_eq!(conflicts[0].current, "https://linkedin.com/in/alice");
    assert_eq!(conflicts[0].suggested, "https://linkedin.com/in/bob");
}

/// Genuinely different GitHub usernames → one conflict with correct field key
/// and original un-normalized current/suggested values.
#[test]
fn conflict_different_github() {
    let current = ContactProfile {
        github: Some("https://github.com/alice".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        github: Some("https://github.com/bob".into()),
        ..Default::default()
    };
    let conflicts = detect_contact_conflicts(&current, &suggested);
    assert_eq!(
        conflicts.len(),
        1,
        "expected exactly one conflict: {conflicts:?}"
    );
    let c = &conflicts[0];
    assert_eq!(c.field, "github");
    // Original (un-normalized) strings are preserved in the conflict.
    assert_eq!(c.current, "https://github.com/alice");
    assert_eq!(c.suggested, "https://github.com/bob");
}

/// Genuinely different website URLs → one conflict.
#[test]
fn conflict_different_website() {
    let current = ContactProfile {
        website: Some("https://alice.dev".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        website: Some("https://bob.dev".into()),
        ..Default::default()
    };
    let conflicts = detect_contact_conflicts(&current, &suggested);
    assert_eq!(
        conflicts.len(),
        1,
        "expected exactly one conflict: {conflicts:?}"
    );
    assert_eq!(conflicts[0].field, "website");
}

/// Genuinely different location.default values → one conflict.
#[test]
fn conflict_different_location_default() {
    let current = ContactProfile {
        location: Some(LocalizedText {
            default: "Amsterdam, Netherlands".into(),
            by_lang: Default::default(),
        }),
        ..Default::default()
    };
    let suggested = ContactProfile {
        location: Some(LocalizedText {
            default: "Berlin, Germany".into(),
            by_lang: Default::default(),
        }),
        ..Default::default()
    };
    let conflicts = detect_contact_conflicts(&current, &suggested);
    assert_eq!(
        conflicts.len(),
        1,
        "expected exactly one conflict: {conflicts:?}"
    );
    let c = &conflicts[0];
    assert_eq!(c.field, "location");
    assert_eq!(c.current, "Amsterdam, Netherlands");
    assert_eq!(c.suggested, "Berlin, Germany");
}

/// location.default case-insensitive → no conflict.
#[test]
fn no_conflict_location_case_insensitive() {
    let current = ContactProfile {
        location: Some(LocalizedText {
            default: "Netherlands".into(),
            by_lang: Default::default(),
        }),
        ..Default::default()
    };
    let suggested = ContactProfile {
        location: Some(LocalizedText {
            default: "netherlands".into(),
            by_lang: Default::default(),
        }),
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "location.default differing only by case must not produce a conflict"
    );
}

/// Differing byLang with same .default → no conflict (only .default is compared).
#[test]
fn no_conflict_location_differing_bylang_only() {
    let current = ContactProfile {
        location: Some(LocalizedText {
            default: "Netherlands".into(),
            by_lang: [("de".to_string(), "Niederlande".to_string())].into(),
        }),
        ..Default::default()
    };
    let suggested = ContactProfile {
        location: Some(LocalizedText {
            default: "Netherlands".into(),
            by_lang: [("de".to_string(), "Holland".to_string())].into(),
        }),
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "identical location.default with differing byLang must not produce a conflict"
    );
}

// ── detect_contact_conflicts — one-side-empty cases ──────────────────────────

/// Field present only on current side → not a conflict.
#[test]
fn no_conflict_when_suggested_field_empty() {
    let current = ContactProfile {
        email: Some("alice@example.com".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        email: None,
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "a field present only on the current side must not produce a conflict"
    );
}

/// Field present only on suggested side → not a conflict.
#[test]
fn no_conflict_when_current_field_empty() {
    let current = ContactProfile {
        email: None,
        ..Default::default()
    };
    let suggested = ContactProfile {
        email: Some("bob@example.com".into()),
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "a field present only on the suggested side must not produce a conflict"
    );
}

/// Whitespace-only value on suggested side → treated as empty → not a conflict.
/// `non_empty` is the gate: it trims and rejects blank strings before any
/// field-comparison normalizer is reached.
#[test]
fn no_conflict_when_suggested_whitespace_only() {
    let current = ContactProfile {
        phone: Some("+31 6 12345678".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        phone: Some("   ".into()),
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "whitespace-only suggested value must be treated as empty"
    );
}

// ── norm_url no-host / malformed-value edge cases ────────────────────────────
//
// `non_empty` is the gate: whitespace-only values are filtered before conflict
// detection and never reach `norm_url`. What follows documents the behavior for
// the non-empty malformed inputs that DO reach `norm_url`.
//
// Finding (no source bug): all cases are deterministic.
//
// - A bare non-URL string (e.g. "not-a-url") has no http(s) scheme prefix, so
//   `norm_url` treats the whole trimmed string as the "host". It normalizes to
//   itself, which differs from any real URL's normalized form → conflict IS
//   generated. This is correct behavior: the user stored a malformed value and
//   the import has a real URL; surfacing the mismatch is the right call.
//
// - A scheme-only value (e.g. "https://") passes `non_empty` (it is non-empty
//   after trimming). `norm_url` strips the scheme, finds no host segment, and
//   returns "". This differs from any real URL → conflict IS generated.
//   Documented as expected: the empty-host path produces an empty normal form,
//   which collides with nothing and correctly triggers a conflict report.

/// A bare non-URL string on the current side vs a real URL on the suggested
/// side: `non_empty` lets it through; `norm_url` treats the bare string as its
/// own "host" → the values normalize differently → one conflict is generated.
#[test]
fn norm_url_no_host_bare_string_yields_conflict() {
    let current = ContactProfile {
        website: Some("not-a-url".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        website: Some("https://alice.dev".into()),
        ..Default::default()
    };
    let conflicts = detect_contact_conflicts(&current, &suggested);
    // Deterministic: one conflict, original values preserved.
    assert_eq!(
        conflicts.len(),
        1,
        "bare non-URL vs real URL must produce a conflict: {conflicts:?}"
    );
    let c = &conflicts[0];
    assert_eq!(c.field, "website");
    assert_eq!(c.current, "not-a-url");
    assert_eq!(c.suggested, "https://alice.dev");
}

/// A scheme-only value ("https://") passes `non_empty` (it is non-whitespace)
/// and normalizes via `norm_url` to "" (no host, no path). A real URL on the
/// other side normalizes to its host → they differ → one conflict is generated.
#[test]
fn norm_url_no_host_scheme_only_yields_conflict() {
    let current = ContactProfile {
        linkedin: Some("https://linkedin.com/in/alice".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        linkedin: Some("https://".into()),
        ..Default::default()
    };
    let conflicts = detect_contact_conflicts(&current, &suggested);
    // Deterministic: one conflict, original values preserved.
    assert_eq!(
        conflicts.len(),
        1,
        "scheme-only value vs real URL must produce a conflict: {conflicts:?}"
    );
    let c = &conflicts[0];
    assert_eq!(c.field, "linkedin");
    assert_eq!(c.current, "https://linkedin.com/in/alice");
    assert_eq!(c.suggested, "https://");
}

// ── apply_to_header — name fallback ──────────────────────────────────────────

/// When `header.name` is blank, `apply_to_header` fills it from `full_name` so
/// a profile-edited name is not silently dropped during export without generation
/// metadata (the "H6 — full_name never rendered" regression).
#[test]
fn apply_to_header_fills_blank_name_from_full_name() {
    use crate::export::types::DocumentType;
    use crate::model::document::DocumentModel;

    let profile = ContactProfile {
        full_name: Some("Jordan Lee".into()),
        email: Some("jordan@example.com".into()),
        ..Default::default()
    };

    let mut model = DocumentModel::new(DocumentType::Resume);
    // Simulate a header that arrived with no name (blank).
    model.header.name = String::new();

    profile.apply_to_header(&mut model.header, "en");

    assert_eq!(
        model.header.name, "Jordan Lee",
        "blank header.name must be filled from profile.full_name"
    );
    // Contact line is also set.
    assert!(
        !model.header.contact.is_empty(),
        "contact rich text must be set from profile"
    );
}

/// When `header.name` is already set, `apply_to_header` must not overwrite it —
/// the generation metadata name takes precedence over the profile name.
#[test]
fn apply_to_header_does_not_overwrite_existing_name() {
    use crate::export::types::DocumentType;
    use crate::model::document::DocumentModel;

    let profile = ContactProfile {
        full_name: Some("Jordan Lee".into()),
        email: Some("jordan@example.com".into()),
        ..Default::default()
    };

    let mut model = DocumentModel::new(DocumentType::Resume);
    model.header.name = "Alex Carter".to_string();

    profile.apply_to_header(&mut model.header, "en");

    assert_eq!(
        model.header.name, "Alex Carter",
        "an already-populated header.name must never be overwritten"
    );
}

// ── apply_to_header — contact-line fallback (editor-is-source-of-truth) ──────

/// When the text already parses a contact line, `apply_to_header` must leave it
/// alone — the editor's text is the source of truth for what exports, and the
/// profile is only a fallback for a document that has none.
#[test]
fn apply_to_header_keeps_text_derived_contact_line() {
    use crate::model::adapter::model_from_resume_text;

    let text = "Jordan Lee\nBerlin, Germany | jordan@editor.example.com | +49 30 0000000\n\nSUMMARY\nSome text.";
    let mut model = model_from_resume_text(text);
    assert!(
        !model.header.contact.is_empty(),
        "test setup: resume text must parse a contact line"
    );
    let before = model.header.contact.clone();

    let profile = ContactProfile {
        full_name: Some("Jordan Lee".into()),
        email: Some("jordan@profile.example.com".into()),
        phone: Some("+1 555 0100".into()),
        ..Default::default()
    };
    profile.apply_to_header(&mut model.header, "en");

    assert_eq!(
        model.header.contact, before,
        "a text-derived contact line must never be overwritten by the profile"
    );
}

/// When the text has no contact line, `apply_to_header` fills it from the
/// profile — the fallback case these overrides exist for.
#[test]
fn apply_to_header_fills_contact_from_profile_when_text_has_none() {
    use crate::model::adapter::model_from_resume_text;

    let text = "Jordan Lee\n\nSUMMARY\nSome text with no contact line at all.";
    let mut model = model_from_resume_text(text);
    assert!(
        model.header.contact.is_empty(),
        "test setup: resume text must parse with no contact line"
    );

    let profile = ContactProfile {
        full_name: Some("Jordan Lee".into()),
        email: Some("jordan@profile.example.com".into()),
        ..Default::default()
    };
    profile.apply_to_header(&mut model.header, "en");

    assert!(
        !model.header.contact.is_empty(),
        "the profile must fill a header that has no contact line"
    );
    assert!(model
        .header
        .contact
        .iter()
        .any(|r| r.link.as_deref() == Some("mailto:jordan@profile.example.com")));
}

/// extra_links differences are never reported as conflicts.
#[test]
fn no_conflict_for_extra_links() {
    let current = ContactProfile {
        extra_links: vec![ContactLink {
            label: "Dribbble".into(),
            url: "https://dribbble.com/alice".into(),
        }],
        ..Default::default()
    };
    let suggested = ContactProfile {
        extra_links: vec![ContactLink {
            label: "Behance".into(),
            url: "https://behance.net/alice".into(),
        }],
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&current, &suggested).is_empty(),
        "extra_links differences must never be reported as conflicts"
    );
}

/// Multiple genuinely conflicting fields → all reported, in field order.
#[test]
fn multiple_conflicts_reported_independently() {
    let current = ContactProfile {
        email: Some("alice@example.com".into()),
        phone: Some("+31 6 00000001".into()),
        github: Some("https://github.com/alice".into()),
        ..Default::default()
    };
    let suggested = ContactProfile {
        email: Some("bob@example.com".into()),
        phone: Some("+31 6 99999999".into()),
        github: Some("https://github.com/bob".into()),
        ..Default::default()
    };
    let conflicts = detect_contact_conflicts(&current, &suggested);
    assert_eq!(
        conflicts.len(),
        3,
        "all three conflicts must be reported: {conflicts:?}"
    );
    let fields: Vec<&str> = conflicts.iter().map(|c| c.field.as_str()).collect();
    assert!(fields.contains(&"email"), "email conflict missing");
    assert!(fields.contains(&"phone"), "phone conflict missing");
    assert!(fields.contains(&"github"), "github conflict missing");
}
