use super::*;

// ── find_ascii_ci unit tests ───────────────────────────────────────────────

#[test]
fn find_ascii_ci_basic() {
    // Case-insensitive ASCII hit.
    assert_eq!(find_ascii_ci("Hello World", "world"), Some(6));
    assert_eq!(find_ascii_ci("COMPANY: Acme", "company:"), Some(0));
    // Miss.
    assert_eq!(find_ascii_ci("no match here", "xyz"), None);
    // Empty needle → always 0.
    assert_eq!(find_ascii_ci("anything", ""), Some(0));
}

#[test]
fn find_ascii_ci_multibyte_before_match() {
    // Multibyte chars before the match must not perturb the returned offset.
    // The returned index must be a valid char boundary in `haystack`.
    let s = "🎤 Company: Acme Corp";
    let idx = find_ascii_ci(s, "company:").expect("should match");
    // Verify the offset is valid (slicing must not panic).
    let _ = &s[idx..];
    // Verify the matched bytes reproduce the original casing.
    assert_eq!(&s[idx..idx + "company:".len()], "Company:");
}

// ── extract_company priority 1 (L19 panic repro) ──────────────────────────
//
// Old bug: `lower.find(prefix)` returned offset `i` into the PER-LINE
// lowercased string; the old code applied that offset to the FULL `text`
// string.  On "🎤 Jobs\nat Acme Corp":
//   • line "at Acme Corp": lower.find("at ") == 0, prefix.len() == 3
//   • &text[0 + 3..] → text[3] == 0xA4, the last byte of 🎤
//     (U+1F3A4 = [F0 9F 8E A4]), which is a UTF-8 continuation byte → panic.
#[test]
fn company_p1_multiline_emoji_headline_no_panic() {
    let ad = "🎤 Jobs\nat Acme Corp";
    let meta = extract(ad);
    assert!(
        meta.company.contains("Acme Corp"),
        "expected 'Acme Corp', got {:?}",
        meta.company
    );
}

// ── extract_company priority 2 (L41 panic repro) ──────────────────────────
//
// Old bug: `text.to_lowercase().find(pat)` returned `idx` into the lowercased
// copy; `&text[..idx]` used that offset on the ORIGINAL string.
// For "ẞ🎤 is hiring a dev":
//   • ẞ (U+1E9E, 3 bytes [E1 BA 9E]) → ß (U+00DF, 2 bytes [C3 9F]): −1 byte
//   • lowercased = "ß🎤 is hiring a dev"; " is hiring" starts at byte 6
//   • &text[..6] → text[6] == 0xA4, last byte of 🎤 [F0 9F 8E A4]
//     (a continuation byte) → panic.
#[test]
fn company_p2_is_hiring_lowercase_contraction_no_panic() {
    // ẞ→ß contracts by 1 byte, causing the lowercased-string offset to land
    // inside the 4-byte 🎤 in the original text.  Must not panic.
    let ad = "ẞ🎤 is hiring a dev";
    let _meta = extract(ad);
}

// ── extract_role priority 1 (L82 panic repro) ─────────────────────────────
//
// Old bug: identical to the L19 bug — line-local offset applied to full `text`.
// For "🎤🎤\nrole: Backend Engineer":
//   • line "role: Backend Engineer": lower.find("role:") == 0, prefix.len() == 5
//   • &text[0 + 5..] → 2×🎤 = 8 bytes (00–07), text[5] == 0x9F
//     (byte 2 of the second 🎤 [F0 9F 8E A4], a continuation byte) → panic.
#[test]
fn role_p1_multiline_emoji_headline_no_panic() {
    let ad = "🎤🎤\nrole: Backend Engineer";
    let meta = extract(ad);
    assert!(
        meta.role.contains("Backend Engineer"),
        "expected role containing 'Backend Engineer', got {:?}",
        meta.role
    );
}

// ── edge case ─────────────────────────────────────────────────────────────

/// All-emoji input — no labels match; must return an empty company string
/// and not panic.
#[test]
fn all_emoji_no_panic() {
    let meta = extract("🎉🎊🎈🎁🎀");
    assert!(
        meta.company.is_empty(),
        "expected empty company, got {:?}",
        meta.company
    );
}

// ── Chrome / prose rejection (the values a real bundle searched for) ───────
//
// Every string here was the `role=` or `company=` a 2026-08-08 support bundle
// actually sent to the provider's web search. A wrong subject is worse than
// none: it returns a confident brief about something that is not the employer.

#[test]
fn apply_buttons_and_nav_links_are_not_roles() {
    for ad in [
        "Jetzt bewerben\n\nWir suchen einen Entwickler",
        "[← Alle offenen Stellen](/karriere)\n\nSoftware Engineer",
        "Apply now\n\nWe build things",
    ] {
        let meta = extract(ad);
        assert!(
            meta.role.is_empty() || !meta.role.contains("bewerben"),
            "chrome must not become the role, got {:?}",
            meta.role
        );
        assert!(
            !meta.role.contains("]("),
            "a markdown link must not become the role, got {:?}",
            meta.role
        );
    }
}

#[test]
fn a_prose_sentence_is_not_a_role() {
    let ad = "Please note: Fluent Dutch language skills are required for this role.";
    assert_eq!(extract(ad).role, "");
}

#[test]
fn a_real_first_line_title_is_still_extracted() {
    // The gate must not eat the common, correct case.
    let meta = extract("Senior Backend Engineer\n\nWe are a payments company.");
    assert_eq!(meta.role, "Senior Backend Engineer");
}

#[test]
fn labelled_company_and_role_still_win() {
    let meta = extract("Job title: Staff Engineer\nCompany: Codefield\n");
    assert_eq!(meta.role, "Staff Engineer");
    assert_eq!(meta.company, "Codefield");
}

// ── The `at` word-boundary bug (identical to the renderer's own regex) ─────

#[test]
fn at_inside_a_word_is_not_a_company_marker() {
    // "Wh(at) You'll Do" is a near-universal job-ad heading. The bare `"at "`
    // prefix matched inside it and yielded a company of "You'll Do", which a
    // real session then researched three times.
    let meta = extract("What You'll Do\n\nBuild and ship features.");
    assert_ne!(meta.company, "You'll Do");
    assert_eq!(meta.company, "");
}

#[test]
fn at_as_a_real_word_still_extracts_the_company() {
    // The boundary fix must not break the case the prefix exists for.
    let meta = extract("Senior Engineer at Codefield\n");
    assert_eq!(meta.company, "Codefield");
}
