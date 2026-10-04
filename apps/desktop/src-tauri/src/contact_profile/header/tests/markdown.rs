use super::*;

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
