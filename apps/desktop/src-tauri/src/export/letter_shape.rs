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

use crate::export::typst_engine::looks_like_date;
use crate::locale::letter::{conventions, is_salutation, is_signoff, is_subject_line};

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
        // Insert the salutation AFTER any leading furniture, not at line 0.
        // `letter_system` may have the model open the market's own subject
        // line (`Betreff: …`) and/or a date line BEFORE the body
        // (`pipeline::resume::prompts::letter_system`), and
        // `parse_cover_letter` stops classifying subject/date/recipient
        // lines the moment it sees the salutation (`body_started = true`).
        // Prepending at the top pushed that furniture into the body instead
        // of `model.subject` / `model.date`.
        let lines: Vec<&str> = body.lines().collect();
        let body_start = lines
            .iter()
            .position(|l| {
                let t = l.trim();
                !t.is_empty() && !is_subject_line(t) && !looks_like_date(t)
            })
            .unwrap_or(lines.len());
        let mut furniture = &lines[..body_start];
        while furniture.last().is_some_and(|l| l.trim().is_empty()) {
            furniture = &furniture[..furniture.len() - 1];
        }

        if !furniture.is_empty() {
            out.push_str(&furniture.join("\n"));
            out.push_str("\n\n");
        }
        out.push_str(conv.salutations.generic.trim());
        out.push_str("\n\n");
        out.push_str(&lines[body_start..].join("\n"));
    } else {
        out.push_str(body);
    }

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

    // ── salutation-placement regression: `14bd60c3` taught `letter_system` to
    //    have the model open a market letter with a subject line and/or a
    //    date BEFORE the body; the salutation must land AFTER that furniture,
    //    not at line 0, or it becomes unreachable pre-salutation furniture for
    //    `parse_cover_letter` (see that module's own end-to-end tests). ────

    #[test]
    fn inserts_the_salutation_after_a_leading_subject_line() {
        let body = "Betreff: Bewerbung als Software Engineer\n\n\
                     Ich bringe sechs Jahre Erfahrung mit.";
        let out = complete_letter_text(body, "de", "Max Müller");
        assert!(
            out.starts_with(
                "Betreff: Bewerbung als Software Engineer\n\nSehr geehrte Damen und Herren,\n\n"
            ),
            "the subject line must stay ahead of the salutation: {out:?}"
        );
        assert!(out.contains("Ich bringe sechs Jahre Erfahrung mit."));
    }

    #[test]
    fn inserts_the_salutation_after_a_leading_date_line() {
        let body = "Frankfurt, 12. Januar 2025\n\nIch bringe sechs Jahre Erfahrung mit.";
        let out = complete_letter_text(body, "de", "Max Müller");
        assert!(
            out.starts_with("Frankfurt, 12. Januar 2025\n\nSehr geehrte Damen und Herren,\n\n"),
            "the date line must stay ahead of the salutation: {out:?}"
        );
    }

    #[test]
    fn inserts_the_salutation_after_subject_and_date_in_either_order() {
        let subject_then_date = "Betreff: Bewerbung als Software Engineer\n\
                                  \n12. Januar 2025\n\nIch bringe Erfahrung mit.";
        let out = complete_letter_text(subject_then_date, "de", "Max Müller");
        assert!(
            out.starts_with(
                "Betreff: Bewerbung als Software Engineer\n\n12. Januar 2025\n\n\
                 Sehr geehrte Damen und Herren,\n\n"
            ),
            "got: {out:?}"
        );

        let date_then_subject = "12. Januar 2025\n\nBetreff: Bewerbung als Software Engineer\n\n\
                                  Ich bringe Erfahrung mit.";
        let out2 = complete_letter_text(date_then_subject, "de", "Max Müller");
        assert!(
            out2.starts_with(
                "12. Januar 2025\n\nBetreff: Bewerbung als Software Engineer\n\n\
                 Sehr geehrte Damen und Herren,\n\n"
            ),
            "got: {out2:?}"
        );
    }

    /// `us` never has the model write a subject line, so a body with no
    /// leading furniture must still get the salutation at the very top —
    /// the furniture skip must never swallow real body prose.
    #[test]
    fn a_market_with_no_subject_line_convention_is_unaffected() {
        let body = "I am writing to express my interest in the Software Engineer role.";
        let out = complete_letter_text(body, "us", "Jane Smith");
        assert!(out.starts_with("Dear Hiring Manager,\n\n"), "got: {out:?}");
    }

    /// The same request is re-validated on every preview render AND every
    /// export — running the completion twice (furniture-and-all) must equal
    /// running it once.
    #[test]
    fn is_idempotent_with_leading_furniture() {
        let body = "Betreff: Bewerbung als Software Engineer\n\n12. Januar 2025\n\n\
                     Ich bringe sechs Jahre Erfahrung mit.";
        let once = complete_letter_text(body, "de", "Max Müller");
        let twice = complete_letter_text(&once, "de", "Max Müller");
        assert_eq!(once, twice, "once: {once:?}\ntwice: {twice:?}");
    }
}
