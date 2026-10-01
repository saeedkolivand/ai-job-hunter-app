//! signature_title promotion rules: case-mismatch sign-off name, template placeholders, a real job title.

use super::super::parse_cover_letter;
use super::support::*;

/// Regression: LLM sign-off name in title-case must not be promoted to
/// `signature_title` when `meta_name` is uppercase.
///
/// Before the fix, `clean.trim() != name_text.as_str()` was a
/// case-sensitive byte comparison.  "Saeed Kolivand" != "SAEED KOLIVAND"
/// so the name was mistakenly stored as `signature_title` and rendered
/// twice (bold header + plain title line).
#[test]
fn signoff_name_case_mismatch_not_promoted_to_signature_title() {
    let letter = "\
SAEED KOLIVAND
saeed@example.com | https://linkedin.com/in/saeedkolivand

June 10, 2025

Hiring Manager
Acme Corp

Dear Hiring Manager,

I am excited to apply for this role and believe my background is a strong match.

Sincerely,

Saeed Kolivand
";
    let model = parse_cover_letter(
        letter,
        None,
        Some("SAEED KOLIVAND"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert_eq!(
        model.signature_name, "SAEED KOLIVAND",
        "signature_name must come from meta_name"
    );
    assert!(
        model.signature_title.is_none(),
        "trailing sign-off name (title-case) must not be promoted to signature_title; \
         got {:?}",
        model.signature_title
    );
}

/// Live-defect regression, generalised across the placeholder shapes the
/// task describes rather than one instance: a letter-template
/// placeholder left in the signature block must never be promoted to
/// `signature_title`. The German case is the ACTUAL observed export
/// tail ("Ihr Name" after "Mit freundlichen Grüßen"); the other two rows
/// cover the English literal form and bracketed slot syntax. See
/// ADR-034 Consequence #2.
#[test]
fn template_placeholder_not_promoted_to_signature_title() {
    for (signoff, placeholder) in [
        ("Mit freundlichen Grüßen", "Ihr Name"),
        ("Sincerely,", "Your Name"),
        ("Sincerely,", "[Your Title]"),
        ("Sincerely,", "[Job Title]"),
        ("Sincerely,", "[Position]"),
    ] {
        let letter = format!(
            "Saeed Kolivand\nsaeed@example.com\n\nDear Hiring Manager,\n\nI am writing to \
             apply for this role.\n\n{signoff}\n\nSaeed Kolivand\n{placeholder}\n"
        );
        let model = parse_cover_letter(
            &letter,
            None,
            Some("Saeed Kolivand"),
            "us",
            "en",
            dummy_style(),
            false,
        );
        assert_eq!(model.signature_name, "Saeed Kolivand");
        assert!(
            model.signature_title.is_none(),
            "placeholder {placeholder:?} after {signoff:?} must not be promoted to \
             signature_title; got {:?}",
            model.signature_title
        );
    }
}

/// Negative: a genuine job title must still be promoted — the placeholder
/// guard must not over-match and swallow a real signature title.
#[test]
fn real_job_title_still_promoted_to_signature_title() {
    let letter = "\
Saeed Kolivand
saeed@example.com

Dear Hiring Manager,

I am writing to apply for this role.

Sincerely,

Saeed Kolivand
Senior Software Engineer
";
    let model = parse_cover_letter(
        letter,
        None,
        Some("Saeed Kolivand"),
        "us",
        "en",
        dummy_style(),
        false,
    );

    assert_eq!(
        model.signature_title.as_deref(),
        Some("Senior Software Engineer"),
        "a real job title must still be promoted to signature_title"
    );
}
