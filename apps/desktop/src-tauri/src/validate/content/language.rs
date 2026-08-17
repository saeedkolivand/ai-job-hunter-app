//! Whether the generated document — or one of its sections — reads as the
//! target language. Split out of `mod.rs` to keep it under R8's line cap
//! (`docs/architecture-rules.md`).
//!
//! ## Known limit
//!
//! Two detectors decide and check the same question. The renderer picks the
//! target language with **franc**; this module validates with **whatlang**
//! (via [`detected_language`]). [`target_is_corroborated`] therefore really
//! asks "does whatlang agree with franc about this ad" — when the two
//! detectors disagree, the guard goes quiet. Consistent with this module's
//! posture everywhere else (a check that cannot be made reliably goes quiet
//! rather than guesses), but it is a real limit, not a future bug report.

use super::{
    issue, significant_chars, Analysis, ContentIssue, Section, SectionKind, Severity,
    CONTENT_LANGUAGE_MISMATCH,
};
use crate::documents::keywords::detected_language;

/// Below this many non-whitespace characters, `whatlang` guesses. A Critical
/// language mismatch on a two-line draft would be a false accusation, so the
/// check goes quiet instead.
pub(super) const MIN_CHARS_FOR_LANGUAGE_CHECK: usize = 120;

/// Whether `text` is confidently written in something other than `lang`.
///
/// Two independent reasons to go quiet, both already the module's stated
/// posture: too SHORT to read a language from at all
/// ([`MIN_CHARS_FOR_LANGUAGE_CHECK`]), or [`detected_language`] itself is not
/// confident (below `documents::keywords::MIN_DETECTION_CONFIDENCE`) or
/// reads a language this crate does not curate. Either way, `None` never
/// counts as a mismatch — an unreliable read cannot manufacture an
/// accusation, it can only fail to make one.
pub(super) fn is_language_mismatch(text: &str, lang: &str) -> bool {
    significant_chars(text) >= MIN_CHARS_FOR_LANGUAGE_CHECK
        && matches!(detected_language(text), Some(found) if found != lang)
}

/// Whether an independent witness — the job ad, or the candidate's own source
/// résumé — confidently reads as `lang` too, so `lang` itself is credible
/// enough to accuse a document of failing to match it.
///
/// Replaces the old `source_is_a_reliable_control`, which required the SOURCE
/// specifically to already read as the target — true only when no
/// translation was needed, i.e. false in exactly the cross-language case this
/// whole check exists to catch (an English source résumé, a German target).
/// English source + `target_language: "de"` used to make BOTH witnesses fail
/// by construction; corroboration asks a document-agnostic question instead
/// — "is `lang` real" — so a translation run is no longer disqualified from
/// having its own translation graded.
///
/// No length floor on either witness: `detected_language`'s own confidence
/// gate is a better-calibrated reliability signal than a raw character count
/// (see its doc comment) and already absorbs the false-positive risk the old
/// control's floor existed for — a terse ad or a short certifications block
/// reads at confidence 0.08–0.13 in this crate's own fixtures, comfortably
/// below the 0.9 bar, so `detected_language` already answers `None` for them
/// without a second, redundant length check here.
fn target_is_corroborated(job_ad: &str, source_resume: &str, lang: &str) -> bool {
    detected_language(job_ad) == Some(lang) || detected_language(source_resume) == Some(lang)
}

/// Whether `generated` came back in the wrong language for `target_language`,
/// given `source_resume` and `job_ad` as witnesses that the target itself is
/// real (see [`target_is_corroborated`]).
///
/// The single answer to "did this run come back in the wrong language" —
/// [`super::language_issues`] uses it (via [`Analysis::language_mismatch`])
/// for the deterministic Critical, and the pipeline's draft-retry
/// (`pipeline::resume::stages::draft`) is meant to call this SAME function
/// before spending a second model call, so `validate` and the retry guarding
/// against the same defect can never quietly disagree about what "wrong
/// language" means.
pub fn document_language_mismatch(
    generated: &str,
    source_resume: &str,
    job_ad: &str,
    target_language: &str,
) -> bool {
    let lang = super::normalize_language(target_language);
    is_language_mismatch(generated, &lang) && target_is_corroborated(job_ad, source_resume, &lang)
}

/// `content.language_mismatch` — the output is not in the language it was asked
/// for. Critical: a German résumé sent to an English-speaking employer is not a
/// quality nit, and every downstream comparison is meaningless once it holds.
///
/// Two passes, in order. The DOCUMENT pass is [`Analysis::language_mismatch`]
/// (a whole-text read via [`document_language_mismatch`]) — reliable at that
/// scale, but it takes roughly a third of a nine-section résumé drifting to a
/// minority language before the vote flips; one drifted section, or two, reads
/// as noise inside a long document and never fires. So when the document reads
/// clean, a SECTION pass checks each section on its own — the shape the
/// reported defect actually takes ("summary in English, experience in Italian,
/// skills in English again"). Both passes route through the same
/// `detected_language` kernel — this is a second SCOPE the language question
/// is asked over, not a second answer to it.
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
            None,
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
///   for exactly this reason — and it is NOT redundant with
///   `detected_language`'s confidence gate: a comma/middot tool list can read
///   as a covered-but-wrong language with confidence 1.0 (measured: whatlang
///   reads a lowercase Python/data-tooling list as confident French), so the
///   confidence gate alone does not suppress it.
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
/// Gated on `!is_language_mismatch(ctx.input.generated, &ctx.lang)` —
/// the document as a whole must not CONFIDENTLY read as the wrong language.
/// That is deliberately weaker than requiring it to confidently read as
/// RIGHT: a document with exactly one drifted section is, BY CONSTRUCTION,
/// the shape whose whole-text confidence a section-level check most needs to
/// survive. Measured on this crate's own fixture (one Italian EXPERIENCE
/// section inside an otherwise-English résumé): the whole document reads at
/// confidence 0.28, well under `MIN_DETECTION_CONFIDENCE` — so gating on "the
/// whole document confidently reads as `lang`" would have made THIS pass
/// switch itself off in exactly the one case the reported defect actually
/// takes ("summary in English, experience in Italian, skills in English
/// again"): the more a document drifts, the LESS confident the whole-text
/// read becomes, which would make the two mechanisms cancel each other out.
/// `is_language_mismatch` on its own goes quiet on that same low-confidence
/// read (`None` never counts as a mismatch), so the gate opens instead of
/// closing — the section pass gets to look for exactly what corrupted the
/// document-level confidence in the first place.
///
/// Replaces the old `source_is_a_reliable_control` gate for the same reason
/// [`target_is_corroborated`] does — that control read the SOURCE résumé
/// specifically, which fails open precisely when a translation was expected
/// (an English source, a German target: the source was never going to read
/// as German). This gate reads the GENERATED document instead, so it has no
/// translation blind spot; the accepted cost is symmetric with
/// [`target_is_corroborated`]'s: a document that confidently reads as some
/// THIRD, uncorroborated language skips both the document- and section-level
/// passes together, the same "goes quiet on a real disagreement" posture
/// this whole module takes.
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
///   Skills exclusion (see the false-positive note on [`PROSE_LOWERCASE_WORD_RATIO`]).
///
///   The cost, paid deliberately: a model that drifts ONLY a list-shaped
///   section is not caught here (the document-level pass still can, if enough
///   of the rest drifts too) — the same trade the kind allowlist already made.
fn section_language_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if is_language_mismatch(ctx.input.generated, &ctx.lang) {
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
            if !matches!(detected_language(&body), Some(found) if found != ctx.lang) {
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
