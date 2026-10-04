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
//!
//! **Known gap, not fixed here.** [`complete_letter_text`] runs on the WHOLE
//! `request.text`, including anything ahead of a
//! `### COMPLETE COVER LETTER ###` marker that `export::pdf`/`export::docx`'s
//! `extract_section` later strips. `validate_and_normalize`'s own comment
//! reasons that a marker-wrapped letter (the TS fast-path prompt's shape)
//! always carries both parts, making this a no-op for it — true for every
//! letter that prompt actually emits, but not a guarantee this module
//! enforces. A marker-wrapped document missing ONLY the salutation is NOT a
//! no-op here: the furniture-skip below (`body_start`) can land the inserted
//! salutation in the PRE-marker section, where `extract_section` discards it
//! along with the marker line. The result is a silent no-fix, not
//! corruption — the letter comes out exactly as incomplete as it went in,
//! with no error and no signal a fix was attempted and lost.

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
mod tests;
