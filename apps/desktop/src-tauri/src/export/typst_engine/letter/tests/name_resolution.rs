//! Letterhead/signature name resolution: applicant-name leakage guard + ALL-CAPS casing + meta_name fallback rungs.

use super::super::parse_cover_letter;
use super::support::*;
use crate::contact_profile::ContactProfile;

/// B.1: the applicant's own name — however the model cased it, plain or
/// **bold** — must be skipped as a header line, never pushed into the
/// recipient block, even when `meta_name` is stored ALL-CAPS.
#[test]
fn applicant_name_not_leaked_into_recipient_block() {
    for first_line in ["Saeed Kolivand", "**Saeed Kolivand**"] {
        let letter = format!(
            "\
{first_line}
saeed@example.com | https://linkedin.com/in/saeedkolivand

June 10, 2025

Hiring Manager
Acme Corp

Dear Hiring Manager,

I am excited to apply for this role and believe my background is a strong match.

Sincerely,

Saeed Kolivand
"
        );
        let model = parse_cover_letter(
            &letter,
            None,
            Some("SAEED KOLIVAND"),
            "us",
            "en",
            dummy_style(),
            false,
        );

        assert!(
            model
                .recipient_lines
                .iter()
                .any(|l| l.contains("Acme") || l.contains("Hiring Manager")),
            "recipient block should still capture the company/manager lines ({first_line:?}); \
             got {:?}",
            model.recipient_lines
        );
        assert!(
            !model.recipient_lines.iter().any(|l| l
                .trim()
                .trim_end_matches([',', '.'])
                .to_lowercase()
                == "saeed kolivand"),
            "applicant name must not leak into the recipient block ({first_line:?}); got {:?}",
            model.recipient_lines
        );
    }
}

/// B.2: an ALL-CAPS stored name adopts the contact profile's mixed-case
/// casing for BOTH letterhead and signature when the profile's `full_name`
/// matches case-insensitively.
#[test]
fn all_caps_name_adopts_profile_casing() {
    let profile = ContactProfile {
        full_name: Some("Saeed Kolivand".to_string()),
        ..Default::default()
    };
    let model = parse_cover_letter(
        "Dear Hiring Manager,\n\nHello there.\n\nSincerely,\n",
        Some(&profile),
        Some("SAEED KOLIVAND"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert_eq!(model.letterhead.name, "Saeed Kolivand");
    assert_eq!(model.signature_name, "Saeed Kolivand");
}

/// B.2 no-op: with no profile, an ALL-CAPS name is left untouched (never
/// title-cased) — the stored casing is authoritative.
#[test]
fn all_caps_name_without_profile_stays_uppercase() {
    let model = parse_cover_letter(
        "Dear Hiring Manager,\n\nHello there.\n\nSincerely,\n",
        None,
        Some("SAEED KOLIVAND"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert_eq!(model.letterhead.name, "SAEED KOLIVAND");
    assert_eq!(model.signature_name, "SAEED KOLIVAND");
}

/// B.2 no-op: a profile for a DIFFERENT person must not re-case the stored
/// name — the casing preference is scoped to the same name only.
#[test]
fn all_caps_name_with_different_profile_stays_uppercase() {
    let profile = ContactProfile {
        full_name: Some("Jane Smith".to_string()),
        ..Default::default()
    };
    let model = parse_cover_letter(
        "Dear Hiring Manager,\n\nHello there.\n\nSincerely,\n",
        Some(&profile),
        Some("SAEED KOLIVAND"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert_eq!(model.letterhead.name, "SAEED KOLIVAND");
    assert_eq!(model.signature_name, "SAEED KOLIVAND");
}

/// A blank `meta_name` (the shape three renderer call sites actually send
/// when no candidate name is known) resolves the name from the attached
/// `ContactProfile` rather than falling all the way to the first text
/// line — which, on a body-only letterhead-less letter, is a salutation,
/// not a name.
#[test]
fn blank_meta_name_resolves_from_the_contact_profile_before_the_first_line_fallback() {
    let profile = ContactProfile {
        full_name: Some("Jane Smith".to_string()),
        ..Default::default()
    };
    let letter = "Dear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\n";
    for meta in [None, Some("")] {
        let model = parse_cover_letter(
            letter,
            Some(&profile),
            meta,
            "us",
            "en",
            dummy_style(),
            false,
        );
        assert_eq!(
            model.letterhead.name, "Jane Smith",
            "meta={meta:?}: the contact profile's name must win over the \
             salutation-line fallback"
        );
        assert_eq!(model.signature_name, "Jane Smith");
    }
}

/// With NO contact profile either, the fallback still degrades to the
/// first-line rule exactly as before — this rung is additive, not a
/// replacement.
#[test]
fn blank_meta_name_with_no_profile_still_falls_back_to_the_first_line() {
    let model = parse_cover_letter(
        "Jane Smith\n\nDear Hiring Manager,\n\nHello there.\n\nSincerely,\n",
        None,
        None,
        "us",
        "en",
        dummy_style(),
        false,
    );
    assert_eq!(model.letterhead.name, "Jane Smith");
}
