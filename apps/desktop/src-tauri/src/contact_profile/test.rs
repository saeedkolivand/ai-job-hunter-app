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
