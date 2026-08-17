//! Whether the generated document — or one of its sections — reads as the
//! target language. Split out of `mod.rs` to keep it under R8's line cap
//! (`docs/architecture-rules.md`); the split changes nothing else.

use super::{
    issue, significant_chars, Analysis, ContentInput, ContentIssue, Section, SectionKind, Severity,
    CONTENT_LANGUAGE_MISMATCH,
};
use crate::documents::keywords::languages_align;

/// Below this many non-whitespace characters, `whatlang` guesses. A Critical
/// language mismatch on a two-line draft would be a false accusation, so the
/// check goes quiet instead.
pub(super) const MIN_CHARS_FOR_LANGUAGE_CHECK: usize = 120;

/// Whether `text` is written in something other than `lang`.
///
/// Routed through `languages_align` rather than a bespoke comparison so the
/// language question has ONE answer in this codebase: it detects `text`'s
/// language with the same `whatlang` path `keywords.rs` uses and asks whether
/// that language pairs with the given locale tag. Goes quiet on short text,
/// where detection is a guess.
pub(super) fn is_language_mismatch(text: &str, lang: &str) -> bool {
    let significant = text.chars().filter(|c| !c.is_whitespace()).count();
    significant >= MIN_CHARS_FOR_LANGUAGE_CHECK && !languages_align(text, lang)
}

/// Whether this report may raise `content.language_mismatch`.
///
/// The generated text reading as the wrong language is necessary but NOT
/// sufficient. The candidate's own source résumé is the CONTROL: it was written
/// by a human in the language they meant, so it is what proves the detector can
/// read this candidate's writing at all. `whatlang` routinely calls a terse,
/// tech-heavy English résumé Dutch, Norwegian or Tagalog; firing on its word
/// alone produces a Critical the user cannot act on ("re-generate it in English"
/// — it *is* in English) and, worse, blanks `keywordCoverage` and suppresses
/// every alignment finding along with it.
///
/// So the control must PASS, positively: long enough for the detector to have
/// read it, and reading as the target language. The earlier form of this guard
/// only suppressed when the source's misdetected tag EQUALLED the generated
/// one, which failed open in both directions a real document takes:
///
/// * a **short** source — [`is_language_mismatch`] goes quiet below
///   [`MIN_CHARS_FOR_LANGUAGE_CHECK`], and the guard read that silence as "the
///   source is fine", i.e. as evidence FOR the accusation;
/// * a **heavily-reworded** source — two mis-reads of the same English land on
///   different tags (`nl` vs `tl`) as easily as on one, and the equality test
///   then treated a detector disagreement as a generation defect.
///
/// A genuinely mis-generated document still fires: an English source reads as
/// English (control passes), the German output does not.
///
/// *No confidence signal is used, because there is none to use:* both detector
/// wrappers this crate owns (`documents::keywords::languages_align` and
/// `detect_locale_tag`) discard `whatlang`'s `Info` and return only the language,
/// so `is_reliable()`/`confidence()` are not reachable without a second,
/// independent detection path in this module — which is exactly the drift the
/// "one answer to the language question" rule exists to prevent. The length gate
/// plus a positive control covers the same ground: an unreliable read cannot
/// pass a control that requires the EXPECTED language.
pub(super) fn language_mismatch_for(input: &ContentInput, lang: &str) -> bool {
    is_language_mismatch(input.generated, lang) && source_is_a_reliable_control(input, lang)
}

/// Whether the source résumé can carry the weight of a Critical: long enough for
/// `whatlang` to be reading rather than guessing, and reading as `lang`.
fn source_is_a_reliable_control(input: &ContentInput, lang: &str) -> bool {
    significant_chars(input.source_resume) >= MIN_CHARS_FOR_LANGUAGE_CHECK
        && languages_align(input.source_resume, lang)
}

/// `content.language_mismatch` — the output is not in the language it was asked
/// for. Critical: a German résumé sent to an English-speaking employer is not a
/// quality nit, and every downstream comparison is meaningless once it holds.
///
/// Two passes, in order. The DOCUMENT pass is a majority read over the whole
/// text (`languages_align` over everything concatenated) — reliable at that
/// scale, but it takes roughly a third of a nine-section résumé drifting to a
/// minority language before the vote flips; one drifted section, or two, reads
/// as noise inside a long document and never fires. So when the document reads
/// clean, a SECTION pass checks each section on its own — the shape the
/// reported defect actually takes ("summary in English, experience in Italian,
/// skills in English again"). Both passes route through the same
/// `languages_align` kernel — this is a second SCOPE the language question is
/// asked over, not a second answer to it.
pub(super) fn language_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if ctx.language_mismatch {
        return vec![issue(
            CONTENT_LANGUAGE_MISMATCH,
            None,
            format!(
                "This document does not read as {}, the language it was generated for. \
                 Re-generate it with the target language set correctly before sending.",
                ctx.lang
            ),
            Some(ctx.lang.clone()),
        )];
    }
    section_language_issues(ctx)
}

/// Share of `text`'s alphabetic-initial words that open LOWERCASE — the
/// discriminator [`section_language_issues`] uses in place of a heading-kind
/// allowlist to decide whether a section reads as PROSE at all.
///
/// A list item — a certification, a tool name, a keyword line — is written in
/// Title Case almost without exception. A sentence in any language this
/// pipeline supports is not, however heavily it capitalizes its OWN nouns
/// (German capitalizes every one): it still connects them with lowercase
/// articles, prepositions, pronouns and verbs. So the signal is not case
/// itself (language-dependent) but how MUCH of the text sits in lowercase —
/// exactly the function-word density `whatlang` needs to read a language from
/// at all, without needing to know what that language is first.
///
/// *Accepted cost, in both directions:*
///
/// - A lowercase technical list under a heading `classify_section` does NOT
///   recognize as [`SectionKind::Skills`] ("kafka, postgresql" under
///   Experience) reads as prose here — canonical tool-name casing (`pandas`,
///   `git`, `nginx`) IS lowercase, so this is measured, not hypothetical.
///   The call site excludes `SectionKind::Skills` outright (belt and braces)
///   for exactly this reason.
/// - A caseless script (Arabic, Hebrew, CJK, Thai, Devanagari, …) has no
///   uppercase/lowercase distinction at all, so a whole-TEXT ratio would
///   always read 0 and silently skip it — worse, since a real drift then
///   goes unreported. A caseless section still carries a Latin heading word
///   ahead of it in [`section_text`]'s output ("EXPERIENCE" over an Arabic
///   body), which would poison a whole-text caseless check too, so
///   [`looks_like_prose`] decides PER WORD: a word with no case distinction
///   counts toward the lowercase side, same as a genuine lowercase word.
pub(super) const PROSE_LOWERCASE_WORD_RATIO: f64 = 0.2;

pub(super) fn looks_like_prose(text: &str) -> bool {
    let mut words = 0usize;
    let mut lowercase_initial = 0usize;
    for word in text.split_whitespace() {
        let Some(first) = word.chars().next() else {
            continue;
        };
        if !first.is_alphabetic() {
            continue; // A bare number, a bullet marker, a parenthesised year.
        }
        words += 1;
        // A caseless-script word can never "open Title Case" the way a Latin
        // word can, so it counts as lowercase too — see the doc above.
        let has_case = word.chars().any(|c| c.is_uppercase() || c.is_lowercase());
        if first.is_lowercase() || !has_case {
            lowercase_initial += 1;
        }
    }
    words > 0 && (lowercase_initial as f64 / words as f64) >= PROSE_LOWERCASE_WORD_RATIO
}

/// Per-section half of [`language_issues`].
///
/// Gated on the SAME positive control the document-level check requires
/// ([`source_is_a_reliable_control`]) rather than a fresh reliability read per
/// section — the control question is a property of the candidate's source
/// résumé, not of any one generated section.
///
/// Two more guards a section-scoped read needs that the document-scoped one
/// does not, both TRADED conservatively toward missing a defect rather than
/// accusing a truthful section:
///
/// * the SAME [`MIN_CHARS_FOR_LANGUAGE_CHECK`] floor, applied per section;
/// * a [`SectionKind::Skills`] exclusion, AND only sections that
///   [`looks_like_prose`] on top of that — belt and braces, not either
///   alone. The allowlist this replaced was a heading-KIND check
///   (Summary/Experience/Projects) too NARROW to see a drifted "Work
///   History"/"Selected Roles" section at all; `looks_like_prose` widens
///   coverage to any sentence-shaped section, including ones
///   `classify_section` can't name, but it is not a substitute for the
///   Skills exclusion — a Skills section written in canonical lowercase
///   tool-name casing (`pandas`, `git`, `nginx`) reads as prose by the same
///   signal a real sentence does. Keep both.
///
///   The cost, paid deliberately: a model that drifts ONLY a list-shaped
///   section is not caught here (the document-level pass still can, if enough
///   of the rest drifts too) — the same trade the kind allowlist already made.
fn section_language_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if !source_is_a_reliable_control(ctx.input, &ctx.lang) {
        return Vec::new();
    }
    ctx.generated_sections
        .iter()
        .skip(1) // section 0 is the header band (name + contact), not prose
        .filter(|section| section.kind != SectionKind::Skills)
        .filter_map(|section| {
            let heading = section.heading.as_deref()?;
            let body = section_text(section);
            if significant_chars(&body) < MIN_CHARS_FOR_LANGUAGE_CHECK {
                return None;
            }
            if !looks_like_prose(&body) {
                return None;
            }
            if languages_align(&body, &ctx.lang) {
                return None;
            }
            let mut found = issue(
                CONTENT_LANGUAGE_MISMATCH,
                Some(heading),
                format!(
                    "The \"{heading}\" section does not read as {}, the language the rest of \
                     this document is written in. Re-generate it (or that section) before \
                     sending.",
                    ctx.lang
                ),
                Some(ctx.lang.clone()),
            );
            // `Other` (Volunteer/Awards/…) has no `SectionKey` — downgraded so it surfaces, not blocks.
            if section.kind == SectionKind::Other {
                found.severity = Severity::Warning;
            }
            Some(found)
        })
        .collect()
}

/// A section's own text — heading plus every line beneath it, one per line —
/// for a check that reads the section as one span rather than line by line.
fn section_text(section: &Section) -> String {
    let mut text = String::new();
    if let Some(heading) = &section.heading {
        text.push_str(heading);
        text.push('\n');
    }
    for line in &section.lines {
        text.push_str(&line.text);
        text.push('\n');
    }
    text
}
