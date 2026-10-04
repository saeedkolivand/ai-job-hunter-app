use super::*;
use crate::contact_profile::{ContactLink, LocalizedText};

/// A profile with ONE identity field set, keyed by its conflict `field` name.
fn with(field: &str, value: &str) -> ContactProfile {
    let mut p = ContactProfile::default();
    let v = Some(value.to_string());
    match field {
        "email" => p.email = v,
        "phone" => p.phone = v,
        "linkedin" => p.linkedin = v,
        "github" => p.github = v,
        "website" => p.website = v,
        "location" => {
            p.location = Some(LocalizedText {
                default: value.to_string(),
                ..Default::default()
            })
        }
        other => panic!("unknown identity field {other}"),
    }
    p
}

// ── detect_contact_conflicts — no-conflict cases (normalized-equal, or one side empty) ──

#[test]
fn no_conflict_when_values_normalize_equal_or_one_side_is_empty() {
    for (current, suggested, why) in [
        // Same email differing only in case → normalized equal.
        (
            with("email", "Alex.Carter@Example.COM"),
            with("email", "alex.carter@example.com"),
            "same email differing only by case must not produce a conflict",
        ),
        // Same phone formatted differently → digits-only normalization.
        (
            with("phone", "+1 (555) 123-4567"),
            with("phone", "15551234567"),
            "same phone with different formatting must not produce a conflict",
        ),
        // Same LinkedIn URL differing by scheme, www., and trailing slash.
        (
            with("linkedin", "https://www.linkedin.com/in/x/"),
            with("linkedin", "http://linkedin.com/in/x"),
            "same URL differing only by scheme/www./trailing-slash must not produce a conflict",
        ),
        // Same website URL differing by https vs http.
        (
            with("website", "https://my-portfolio.dev/work"),
            with("website", "http://my-portfolio.dev/work"),
            "same website URL differing only by http/https scheme must not produce a conflict",
        ),
        // location.default case-insensitive.
        (
            with("location", "Netherlands"),
            with("location", "netherlands"),
            "location.default differing only by case must not produce a conflict",
        ),
        // Field present only on current side.
        (
            with("email", "alice@example.com"),
            ContactProfile::default(),
            "a field present only on the current side must not produce a conflict",
        ),
        // Field present only on suggested side.
        (
            ContactProfile::default(),
            with("email", "bob@example.com"),
            "a field present only on the suggested side must not produce a conflict",
        ),
        // Whitespace-only value on suggested side → treated as empty.
        // `non_empty` is the gate: it trims and rejects blank strings before any
        // field-comparison normalizer is reached.
        (
            with("phone", "+31 6 12345678"),
            with("phone", "   "),
            "whitespace-only suggested value must be treated as empty",
        ),
    ] {
        assert!(
            detect_contact_conflicts(&current, &suggested).is_empty(),
            "{why}"
        );
    }
}

/// Differing byLang with same .default → no conflict (only .default is compared).
#[test]
fn no_conflict_location_differing_bylang_only() {
    let location = |de: &str| ContactProfile {
        location: Some(LocalizedText {
            default: "Netherlands".into(),
            by_lang: [("de".to_string(), de.to_string())].into(),
        }),
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(&location("Niederlande"), &location("Holland")).is_empty(),
        "identical location.default with differing byLang must not produce a conflict"
    );
}

/// extra_links differences are never reported as conflicts.
#[test]
fn no_conflict_for_extra_links() {
    let one_link = |label: &str, url: &str| ContactProfile {
        extra_links: vec![ContactLink {
            label: label.into(),
            url: url.into(),
        }],
        ..Default::default()
    };
    assert!(
        detect_contact_conflicts(
            &one_link("Dribbble", "https://dribbble.com/alice"),
            &one_link("Behance", "https://behance.net/alice")
        )
        .is_empty(),
        "extra_links differences must never be reported as conflicts"
    );
}

// ── detect_contact_conflicts — real conflicts ─────────────────────────────────
//
// Each case yields exactly one conflict that carries the field key and the
// ORIGINAL (un-normalized) current/suggested values.
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

#[test]
fn genuinely_different_values_yield_one_conflict_with_the_original_values() {
    for (field, current, suggested, why) in [
        (
            "email",
            "alice@example.com",
            "bob@example.com",
            "expected exactly one conflict",
        ),
        (
            "phone",
            "+31 6 12345678",
            "+1 (800) 555-0199",
            "expected exactly one conflict",
        ),
        (
            "linkedin",
            "https://linkedin.com/in/alice",
            "https://linkedin.com/in/bob",
            "expected exactly one conflict",
        ),
        (
            "github",
            "https://github.com/alice",
            "https://github.com/bob",
            "expected exactly one conflict",
        ),
        (
            "website",
            "https://alice.dev",
            "https://bob.dev",
            "expected exactly one conflict",
        ),
        (
            "location",
            "Amsterdam, Netherlands",
            "Berlin, Germany",
            "expected exactly one conflict",
        ),
        // A bare non-URL string on the current side vs a real URL on the suggested
        // side: `non_empty` lets it through; `norm_url` treats the bare string as its
        // own "host" → the values normalize differently → one conflict is generated.
        (
            "website",
            "not-a-url",
            "https://alice.dev",
            "bare non-URL vs real URL must produce a conflict",
        ),
        // A scheme-only value ("https://") passes `non_empty` (it is non-whitespace)
        // and normalizes via `norm_url` to "" (no host, no path). A real URL on the
        // other side normalizes to its host → they differ → one conflict is generated.
        (
            "linkedin",
            "https://linkedin.com/in/alice",
            "https://",
            "scheme-only value vs real URL must produce a conflict",
        ),
    ] {
        let conflicts = detect_contact_conflicts(&with(field, current), &with(field, suggested));
        assert_eq!(conflicts.len(), 1, "{why}: {conflicts:?}");
        let c = &conflicts[0];
        assert_eq!(c.field, field);
        assert_eq!(c.current, current);
        assert_eq!(c.suggested, suggested);
    }
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
