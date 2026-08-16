//! Complete a body-only cover letter with the salutation / sign-off / name
//! furniture the staged pipeline's prompt promises the app adds — the letter
//! side of a promise the résumé side already keeps
//! (`ContactProfile::apply_to_header`, `crate::contact_profile`).
//!
//! `pipeline::resume::prompts::letter_system` tells the model:
//!
//! > Do NOT write a contact header, a salutation line, or a signature block —
//! > the application adds them at export time.
//!
//! Nothing did, until [`complete_letter_text`]. It runs once, at the export
//! boundary (`export::commands::validate_and_normalize`), so every renderer
//! (PDF, both DOCX line-scanners, the live preview) and every letter already
//! stored in the DB gets the fix with no regeneration and no per-parser
//! change — `parse_cover_letter` and the DOCX scanners already know how to
//! render a salutation/sign-off correctly, they just never received one.

use crate::locale::letter::{conventions, is_salutation, is_signoff};

/// Give a body-only letter the furniture the pipeline prompt promises the app
/// adds. No-op for a letter that already carries its own salutation AND
/// sign-off: a full letter from the TS fast-path prompt
/// (`packages/prompts/src/generate/cover-letter/cover-letter.ts`, which
/// always emits both under the `### COMPLETE COVER LETTER ###` marker) must
/// round-trip through the export path unchanged — the same document gets
/// re-validated on every preview render and every export, so a non-idempotent
/// completion would double up the furniture each time.
pub(crate) fn complete_letter_text(text: &str, market: &str, name: &str) -> String {
    let body = text.trim();
    if body.is_empty() {
        // Nothing to complete — and nothing for the caller to render either.
        return text.to_string();
    }

    let has_salutation = body.lines().any(is_salutation);
    let has_signoff = body.lines().any(is_signoff);
    if has_salutation && has_signoff {
        return text.to_string();
    }

    let conv = conventions(market);
    let mut out = String::new();

    if !has_salutation {
        out.push_str(conv.salutations.generic.trim());
        out.push_str("\n\n");
    }
    out.push_str(body);

    if !has_signoff {
        out.push_str("\n\n");
        // `signoffs` is a `Vec<String>` off the shared JSON fixture — empty
        // only if a market entry were malformed, which `conventions()`'s own
        // parse-time `.expect` already guards against for every entry it
        // returns. Defensive fallback anyway: never let a missing sign-off
        // panic the export.
        out.push_str(
            conv.signoffs
                .first()
                .map(String::as_str)
                .unwrap_or("Sincerely,"),
        );
        let name = name.trim();
        if !name.is_empty() {
            out.push('\n');
            out.push_str(name);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_a_body_only_de_letter() {
        let body = "Ich schreibe Ihnen, um mein Interesse an der Stelle als \
                     Softwareentwickler auszudrücken.";
        let out = complete_letter_text(body, "de", "Max Müller");
        assert!(
            out.starts_with("Sehr geehrte Damen und Herren,\n\n"),
            "got: {out:?}"
        );
        assert!(out.contains(body));
        assert!(
            out.trim_end().ends_with("Mit freundlichen Grüßen\nMax Müller"),
            "got: {out:?}"
        );
    }

    #[test]
    fn completes_a_body_only_us_letter_with_the_english_pair() {
        let body = "I am writing to express my interest in the Software Engineer role.";
        let out = complete_letter_text(body, "us", "Jane Smith");
        assert!(out.starts_with("Dear Hiring Manager,\n\n"), "got: {out:?}");
        assert!(out.contains(body));
        assert!(
            out.trim_end().ends_with("Sincerely,\nJane Smith"),
            "got: {out:?}"
        );
    }

    /// The double-add guard: a letter that already carries its own salutation
    /// and sign-off (the shape the TS fast-path prompt always emits) must
    /// round-trip byte-for-byte — AIGeneratePage still produces full letters
    /// through this same export path.
    #[test]
    fn is_a_noop_on_an_already_complete_letter() {
        let complete =
            "Dear Hiring Manager,\n\nI am writing about the role.\n\nSincerely,\nJane Smith\n";
        assert_eq!(complete_letter_text(complete, "us", "Jane Smith"), complete);
    }

    #[test]
    fn blank_input_is_returned_unchanged() {
        assert_eq!(complete_letter_text("", "us", "Jane Smith"), "");
        assert_eq!(complete_letter_text("   \n\t ", "de", "Max"), "   \n\t ");
    }

    /// No candidate name known: the sign-off is still added, but with no
    /// dangling blank name line under it.
    #[test]
    fn skips_the_name_line_when_the_name_is_blank() {
        let out = complete_letter_text("Body text here.", "us", "");
        assert!(out.trim_end().ends_with("Sincerely,"), "got: {out:?}");
    }

    /// Only the missing half is added when the letter already has one of the
    /// two — the existing salutation/sign-off must not be duplicated.
    #[test]
    fn adds_only_the_missing_half() {
        let has_salutation_only = "Dear Hiring Manager,\n\nBody text here.";
        let out = complete_letter_text(has_salutation_only, "us", "Jane Smith");
        assert_eq!(out.matches("Dear Hiring Manager,").count(), 1);
        assert!(out.trim_end().ends_with("Sincerely,\nJane Smith"), "got: {out:?}");

        let has_signoff_only = "Body text here.\n\nSincerely,\nJane Smith";
        let out2 = complete_letter_text(has_signoff_only, "us", "Jane Smith");
        assert_eq!(out2.matches("Sincerely,").count(), 1);
        assert!(out2.starts_with("Dear Hiring Manager,\n\n"), "got: {out2:?}");
    }
}
