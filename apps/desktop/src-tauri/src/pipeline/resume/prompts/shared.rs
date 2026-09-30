//! Char caps and small string helpers shared by every stage prompt in this
//! module — language naming (ADR-010's SYSTEM-slot exception) and the
//! fenced-JSON-artifact helper.

use serde::Serialize;

use crate::prompt_fence::fenced;
use crate::validate::content::normalize_language;

/// Char cap on ONE serialized prior-stage artifact inside a prompt.
///
/// **Sized against the measured worst case, not a round number**, because
/// [`fenced`] truncates with NO marker: a cap below what an artifact can
/// legitimately reach cuts the JSON mid-object, silently, and the model reads
/// whatever survives as the whole plan. The 4 000 this used to be was below the
/// strategy artifact's own worst case (~3.9 k pretty-printed for eight roles,
/// before a single two-sentence `angle`), so a full roster's last `perCompany`
/// entries were dropped at the ONE place the roster reaches the document — and
/// the resulting `factual.dropped_role` Critical is unrepairable, because the
/// repair loop has no section to regenerate for an absence. The two artifacts
/// that ride this cap, both MEASURED rather than estimated (the numbers below
/// come from `the_strategy_artifact_survives_a_max_roster_uncapped` and
/// `the_evidence_artifact_survives_a_full_requirement_set`, which fail if they
/// grow past the margin):
///
/// * `resume_strategy` — ≤ [`super::super::stages::MAX_COMPANY_PLANS`] + 1
///   entries, each with a long angle and five emphasis terms, plus six skills
///   groups and a section order: **5 845 chars compact** (7 553
///   pretty-printed).
/// * `evidence_map` — 40 items (`stages::evidence::MAX_REQUIREMENTS`), each
///   carrying a verbatim résumé line as its quote: **13 191 chars compact**
///   (15 199 pretty-printed). This is the artifact that actually approaches the
///   cap, and its quote length is bounded only by the source résumé's own line
///   length, so a document with unusually long lines can still reach it —
///   truncating it degrades ADVICE (the strategy stage's input) rather than
///   dropping an employer, which is why the cap is sized for it rather than the
///   other way round.
///
/// 16 000 clears the measured worst cases by 2.7× and 1.2×. It is charged
/// against a prompt that also carries the résumé and the posting
/// (`RESUME_CAP` + `JOB_CAP` = 16 k chars), so the draft turn's worst case
/// is ~32 k chars ≈ 8 k tokens — inside every model this app talks to.
pub(super) const ARTIFACT_CAP: usize = 16_000;

/// Char cap on the free-text steer a user may attach to a section regenerate.
/// Mirrors the wire schema's `.max(500)`; serde enforces nothing, so the prompt
/// builder caps its own copy.
pub(super) const NOTE_CAP: usize = 500;

/// Char cap on ONE section's current text on the repair path.
pub(super) const SECTION_CAP: usize = 4_000;

/// Char cap on the `<document_context>` sibling-anchor block
/// (`stages::sections::context_anchor`'s output) — the OTHER
/// already-written sections, not the whole surrounding document.
///
/// Sized for what the anchor actually carries, not a round number: a Summary
/// section (2-4 sentences — every fixture in this crate's own tests stays
/// well under 1 000 chars) plus ONE representative Experience bullet (well
/// under 300). 1 500 leaves ~2× margin over that combined worst case. It is
/// deliberately far below `HUMANIZE_DOCUMENT_CAP` (12 000, the whole
/// document): a repair round fans out to up to `MAX_SECTIONS_PER_ROUND`
/// sections per round, up to twice per run, so whatever this cap allows is
/// charged up to 8× per pipeline run — sending the whole draft at that
/// multiplier would roughly double the repair stage's own token cost for a
/// signal the anchor already carries.
pub(in crate::pipeline::resume) const SIBLING_CONTEXT_CAP: usize = 1_500;

/// Char cap on the fenced `<company_research>` brief — same value as
/// `extension_bridge::answer_assist`'s own `BRIEF_CAP`, the other consumer of
/// `CompanyResearch::enrich_with`'s output.
pub(super) const BRIEF_CAP: usize = 2_000;

/// The language token that may reach a SYSTEM slot.
///
/// ADR-010, restated by this module's own doc: *the system slot is a fixed Rust
/// string — nothing that came off a job board, out of a user's file, or out of
/// a model ever reaches it.* `targetLanguage` is renderer-supplied free text
/// (its `.max(32)` is Zod, which does not run on the bare-`invoke` transport),
/// so interpolating it raw was that rule's one exception — and the payload is
/// the most valuable one available: text landing in the SYSTEM slot is the
/// slot the rest of the prompt calls trustworthy.
///
/// [`normalize_language`] is the closure: the same first-two-alphanumerics,
/// lowercased, `"en"`-on-empty normalization `resume_conventions` already
/// applies to derive its heading table, and the same one
/// `validate::content` uses before this value reaches a span. The output is at
/// most two alphanumeric characters — no newline, no instruction, no length.
fn system_language(lang: &str) -> String {
    normalize_language(lang)
}

/// The English NAME of a language, keyed on the ALREADY-NORMALIZED 2-char
/// code [`system_language`] produces. A closed Rust table, so ADR-010 holds a
/// fortiori: what reaches a SYSTEM slot is either a `&'static str` this file
/// owns or the ≤2-char normalized code itself (the fallback arm) — strictly
/// narrower than the bare code every SYSTEM prompt used to interpolate.
///
/// **Deliberately separate from `resume_conventions`'s curated six** (`de
/// es fr it nl pt`): that table answers "what are this market's section
/// headings", this one answers "what is this language called". Japanese has
/// a name and no heading conventions — merging the two would force a heading
/// decision for every uncurated language below.
///
/// Covers every tag `documents::keywords::locale_tag_of` knows plus the
/// common European languages `whatlang` also recognises; anything else falls
/// through to the bare code, exactly as today.
pub(in crate::pipeline::resume) fn language_name(code: &str) -> &str {
    match code {
        "en" => "English",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "nl" => "Dutch",
        "zh" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "ar" => "Arabic",
        "he" => "Hebrew",
        "hi" => "Hindi",
        "bn" => "Bengali",
        "tr" => "Turkish",
        "uk" => "Ukrainian",
        "ru" => "Russian",
        "pl" => "Polish",
        "sv" => "Swedish",
        "da" => "Danish",
        "no" => "Norwegian",
        "fi" => "Finnish",
        "cs" => "Czech",
        "sk" => "Slovak",
        "hu" => "Hungarian",
        "ro" => "Romanian",
        "bg" => "Bulgarian",
        "el" => "Greek",
        "hr" => "Croatian",
        "sl" => "Slovenian",
        "lt" => "Lithuanian",
        "lv" => "Latvian",
        "et" => "Estonian",
        other => other,
    }
}

/// [`system_language`] followed by [`language_name`] — what every SYSTEM
/// prompt in this file interpolates, so a German run reads "in German"
/// rather than "in de".
pub(super) fn system_language_name(lang: &str) -> String {
    language_name(&system_language(lang)).to_string()
}

/// Serialize a prior-stage artifact for a prompt, then FENCE it.
///
/// **Compact, not pretty-printed.** Indentation reads better to a human and
/// buys nothing here: it costs ~23% more characters on the measured worst-case
/// strategy (7 553 vs 5 845) and ~15% on the evidence map, and every one of
/// those characters is spent against [`ARTIFACT_CAP`], which truncates without
/// a marker. Margin on the artifact whose truncation loses an employer is worth
/// more than the model's marginally easier read of an indented object — and
/// JSON is a format every model parses unindented every day. The CAP is the
/// guard; this is the margin.
///
/// A serialization failure yields an empty block rather than an error: a stage
/// that cannot show the previous artifact still has the source résumé, which is
/// the only thing it is allowed to draw facts from anyway.
pub(super) fn fenced_artifact<T: Serialize>(tag: &str, artifact: &T) -> String {
    let json = serde_json::to_string(artifact).unwrap_or_default();
    fenced(tag, &json, ARTIFACT_CAP)
}
