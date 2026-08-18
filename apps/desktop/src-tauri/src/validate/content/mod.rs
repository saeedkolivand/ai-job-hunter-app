//! Deterministic content validation of a GENERATED document.
//!
//! The export gate in [`super`] asks "did the rendered bytes survive an ATS
//! parser?". This asks the earlier question: "does the text the model wrote
//! actually match the candidate's own résumé, this posting, and the target
//! language?"
//!
//! ## The rule this module mechanically enforces
//!
//! The model decides HOW to present verified candidate evidence, never WHAT the
//! candidate has done. So **a model may never emit a Critical** — every
//! [`Severity::Critical`] here comes from a deterministic comparison against
//! `source_resume`, the single source of factual truth. Warnings are advice.
//!
//! ## Framing
//!
//! Issues are guidance, never verdicts: each one is evidence-backed, names the
//! offending span, and advises. The metrics score the DOCUMENT, never the
//! person. (Same posture as the match score — see `docs/knowledge/`'s
//! job-match standards.)
//!
//! ## False positives are the risk
//!
//! A wrong Critical destroys trust in the whole panel, so every threshold is a
//! named `const` with a test pinning it, years 1900–2099 are excluded from
//! metric checks, phone/contact bands are skipped, and anything ambiguous is a
//! Warning. Where a check cannot be made reliably (an unreadable language, an
//! empty posting) it goes quiet rather than guessing.
//!
//! Pure L1: no Tauri, no `AppHandle`, no emit, no I/O.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use rust_stemmers::Stemmer;
use serde::{Deserialize, Serialize};

use crate::documents::evidence::{classify_section, date_spans, trailing_date_column, SectionKind};
use crate::documents::keywords::{
    display_forms, keyword_coverage, keywords, keywords_normalized, languages_align, make_stemmer,
};
use crate::export::parser::parse_resume;
use crate::export::types::{LineKind, ParsedLine};
use crate::observability::Span;
use crate::validate::{Severity, EMAIL_RE};

/// The word-boundary matcher every lexicon-style comparison in this module uses.
/// Re-exported rather than reimplemented: `documents::evidence` compares
/// `PRESENT_MARKERS` with the same function, so a date marker and a voice
/// lexicon entry can never disagree about what a word boundary is.
pub(crate) use crate::documents::evidence::contains_word as contains_phrase;

mod alignment;
mod ats;
mod consistency;
mod credentials;
mod duplicates;
mod factual;
mod language;
mod letter;
pub mod lexicon;
mod voice;

use self::language::{language_issues, MIN_CHARS_FOR_LANGUAGE_CHECK};
// Only `test.rs` (a `super::*` glob import) reaches into these three directly.
#[cfg(test)]
use self::language::{is_language_mismatch, looks_like_prose, PROSE_LOWERCASE_WORD_RATIO};

/// The projects-format primitives the MAX-depth generator has to share with the
/// checks that grade its output.
///
/// The generator SEEDS a project's name, links and stack out of the same source
/// section `factual.altered_project_link` and `consistency.project_structure`
/// then compare its output against. A second answer to "where does an entry
/// begin", "is this span a link", or "how many description lines may an entry
/// carry" would make the generator and the grader disagree about a truthful
/// document — the duplicated-heuristic defect this codebase has paid for
/// before. One definition, re-exported, rather than two that drift.
pub use self::alignment::MIN_COVERAGE_DROP_POINTS;
pub use self::consistency::{project_entry_starts, MAX_PROJECT_DESCRIPTION_LINES};
pub use self::factual::{canonical_link, link_href, names_a_resource, urls_in};
/// The single predicate for "did this run come back in the wrong language" —
/// `validate_content` uses it via [`Analysis::language_mismatch`]; the
/// pipeline's draft-retry (`pipeline::resume::stages::draft`, a later step of
/// the same fix) is meant to call this SAME function before spending a
/// second model call, so the two can never quietly disagree about what "wrong
/// language" means.
pub use self::language::document_language_mismatch;

#[cfg(test)]
mod test;

// ── Issue codes (the fixed vocabulary — one table, used for UI i18n) ─────────
//
// Codes are dotted `<family>.<check>` and NEVER change once shipped: the
// renderer keys its translations off them and a saved quality report keeps them
// forever. Every code lives in [`CONTENT_ISSUE_CODES`] with its severity, and
// [`issue`] reads the severity from that table — so a code and its severity
// cannot drift apart.

pub const FACTUAL_UNSOURCED_METRIC: &str = "factual.unsourced_metric";
pub const FACTUAL_DROPPED_ROLE: &str = "factual.dropped_role";
pub const FACTUAL_UNSUPPORTED_DATE: &str = "factual.unsupported_date";
pub const FACTUAL_ALTERED_PROJECT_LINK: &str = "factual.altered_project_link";
pub const FACTUAL_UNSOURCED_TERM: &str = "factual.unsourced_term";
/// A tenure the source résumé cannot support — see `credentials`. Critical on
/// the strength of the calibration in `test.rs`: the comparison is on a NUMBER,
/// which is the one thing that survives translation and paraphrase unchanged,
/// and it only ever fires on an OVERSTATEMENT of what the source itself says or
/// dates.
pub const FACTUAL_INFLATED_EXPERIENCE: &str = "factual.inflated_experience";
/// A certification the source résumé never names. Critical: a certification is
/// the most checkable claim on a résumé — an employer can look it up — so an
/// invented one is the costliest fabrication in the family, and the trigger set
/// is a curated issuer + acronym list rather than an inference.
pub const FACTUAL_UNSOURCED_CERTIFICATION: &str = "factual.unsourced_certification";
/// The generated document names a place of study while the source names none
/// at all. A Warning, deliberately: this is the residue of a value comparison
/// that MEASURED a false positive on truthful cross-language output (see
/// `credentials::unsupported_institutions`), so the surviving check is scoped
/// to a whole invented education section and stays advisory even there.
pub const FACTUAL_UNSOURCED_INSTITUTION: &str = "factual.unsourced_institution";
pub const CONTENT_LANGUAGE_MISMATCH: &str = "content.language_mismatch";
/// An unfilled template-placeholder slot (e.g. German "Ihr Name") survived
/// into the rendered letter text — see ADR-034 Consequence #2. Deterministic:
/// reuses `locale::letter::is_template_placeholder`, the same predicate the
/// letter parser uses to stop the placeholder being promoted to
/// `signature_title`, so this is the mechanical guard for the drift the
/// parser fix alone cannot catch upstream of export.
pub const LETTER_TEMPLATE_PLACEHOLDER: &str = "letter.template_placeholder";
pub const ALIGNMENT_LOW_COVERAGE: &str = "alignment.low_coverage";
pub const ALIGNMENT_MISSING_TOP_REQUIREMENT: &str = "alignment.missing_top_requirement";
pub const CONSISTENCY_DATE_ORDER: &str = "consistency.date_order";
pub const CONSISTENCY_TITLE_DRIFT: &str = "consistency.title_drift";
pub const CONSISTENCY_SKILL_NOT_DEMONSTRATED: &str = "consistency.skill_not_demonstrated";
pub const CONSISTENCY_PROJECT_STRUCTURE: &str = "consistency.project_structure";
pub const DUPLICATE_BULLET: &str = "duplicate.bullet";
pub const ATS_KEYWORD_DENSITY: &str = "ats.keyword_density";
pub const ATS_HEADER_IN_BODY: &str = "ats.header_in_body";
/// A section heading survived into the generated document with nothing under
/// it. Deterministic and model-free: a heading followed by zero content lines
/// is unambiguous, unlike `ATS_MISSING_SECTION`'s opposite case (a section a
/// parser expects but the résumé never had one to begin with — often fine).
pub const ATS_EMPTY_SECTION: &str = "ats.empty_section";
pub const ATS_MISSING_SECTION: &str = "ats.missing_section";
pub const ATS_LONG_BULLET: &str = "ats.long_bullet";
pub const ATS_BULLET_COUNT: &str = "ats.bullet_count";
pub const VOICE_AI_TELL_LEXICAL: &str = "voice.ai_tell_lexical";
pub const VOICE_TEMPLATE_OPENER: &str = "voice.template_opener";
pub const VOICE_LOW_BURSTINESS: &str = "voice.low_burstiness";
pub const VOICE_RULE_OF_THREE_DENSITY: &str = "voice.rule_of_three_density";
pub const VOICE_EM_DASH_OVERUSE: &str = "voice.em_dash_overuse";
pub const VOICE_GENERIC_LETTER: &str = "voice.generic_letter";
/// Synthetic marker appended when [`MAX_CONTENT_ISSUES`] truncates the issue
/// list — never emitted by a real check, so it's registered like any other
/// code (i18n key + severity) rather than special-cased.
pub const REPORT_TRUNCATED: &str = "report.truncated";

// The `judge.*` family — model-emitted advisory opinions, formerly registered
// for the now-deleted max-depth judge stage. Kept in this table for historical
// reasons (old reports may reference them) but no longer emitted. Each entry is
// a Warning by convention — the rule "a model may never emit a Critical" was
// enforced at the judge's construction site (now gone). The table entry remains
// so the renderer can i18n historic report keys.
/// A sentence the reader has to re-read; a bullet saying two things at once.
pub const JUDGE_CLARITY: &str = "judge.clarity";
/// A claim that reads as unsupported — vague ownership, no result, a skill
/// asserted but never demonstrated.
pub const JUDGE_EVIDENCE: &str = "judge.evidence";
/// Something the posting asks for that the document buries, or space spent on
/// something it does not ask for.
pub const JUDGE_TAILORING: &str = "judge.tailoring";
/// A remark whose `kind` is outside the closed set above. Kept rather than
/// dropped: the model's taxonomy is the least useful part of its remark.
pub const JUDGE_NOTE: &str = "judge.note";

/// Every code this module can emit, with its severity. The single table the
/// renderer enumerates for i18n keys and the constructor reads for severity.
///
/// Criticals are exactly the deterministic factual/language/structure defects
/// that make a document wrong to send; everything else advises.
pub const CONTENT_ISSUE_CODES: &[(&str, Severity)] = &[
    (FACTUAL_UNSOURCED_METRIC, Severity::Critical),
    (FACTUAL_DROPPED_ROLE, Severity::Critical),
    (FACTUAL_UNSUPPORTED_DATE, Severity::Critical),
    (FACTUAL_ALTERED_PROJECT_LINK, Severity::Critical),
    (FACTUAL_INFLATED_EXPERIENCE, Severity::Critical),
    (FACTUAL_UNSOURCED_CERTIFICATION, Severity::Critical),
    (CONTENT_LANGUAGE_MISMATCH, Severity::Critical),
    (ATS_HEADER_IN_BODY, Severity::Critical),
    (ATS_EMPTY_SECTION, Severity::Warning),
    (LETTER_TEMPLATE_PLACEHOLDER, Severity::Critical),
    (FACTUAL_UNSOURCED_TERM, Severity::Warning),
    (FACTUAL_UNSOURCED_INSTITUTION, Severity::Warning),
    (ALIGNMENT_LOW_COVERAGE, Severity::Warning),
    (ALIGNMENT_MISSING_TOP_REQUIREMENT, Severity::Warning),
    (CONSISTENCY_DATE_ORDER, Severity::Warning),
    (CONSISTENCY_TITLE_DRIFT, Severity::Warning),
    (CONSISTENCY_SKILL_NOT_DEMONSTRATED, Severity::Warning),
    (CONSISTENCY_PROJECT_STRUCTURE, Severity::Warning),
    (DUPLICATE_BULLET, Severity::Warning),
    (ATS_KEYWORD_DENSITY, Severity::Warning),
    (ATS_MISSING_SECTION, Severity::Warning),
    (ATS_LONG_BULLET, Severity::Warning),
    (ATS_BULLET_COUNT, Severity::Warning),
    (VOICE_AI_TELL_LEXICAL, Severity::Warning),
    (VOICE_TEMPLATE_OPENER, Severity::Warning),
    (VOICE_LOW_BURSTINESS, Severity::Warning),
    (VOICE_RULE_OF_THREE_DENSITY, Severity::Warning),
    (VOICE_EM_DASH_OVERUSE, Severity::Warning),
    (VOICE_GENERIC_LETTER, Severity::Warning),
    (REPORT_TRUNCATED, Severity::Warning),
    (JUDGE_CLARITY, Severity::Warning),
    (JUDGE_EVIDENCE, Severity::Warning),
    (JUDGE_TAILORING, Severity::Warning),
    (JUDGE_NOTE, Severity::Warning),
];

/// The severity registered for `code`.
///
/// An unregistered code degrades to [`Severity::Warning`] rather than panicking
/// or silently claiming Critical — "when uncertain, warn". A `debug_assert`
/// makes it a test failure, and `every_emitted_code_is_registered` in `test.rs`
/// proves no live check reaches the fallback.
pub fn severity_for(code: &str) -> Severity {
    let found = CONTENT_ISSUE_CODES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, s)| *s);
    debug_assert!(found.is_some(), "unregistered content issue code: {code}");
    found.unwrap_or(Severity::Warning)
}

// ── Contract ────────────────────────────────────────────────────────────────

/// Which kind of document is being checked. A cover letter skips every
/// résumé-structure check (it has no sections, roles or bullets) and validates
/// its facts against the source résumé AND the job ad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DocKind {
    Resume,
    CoverLetter,
}

/// Everything a content check needs. Borrowed — this runs on text already in
/// hand, and copying a résumé three times to validate it would be silly.
#[derive(Debug, Clone, Copy)]
pub struct ContentInput<'a> {
    /// The generated résumé (or letter) text to check.
    pub generated: &'a str,
    /// The candidate's own résumé — the ONLY source of factual truth.
    pub source_resume: &'a str,
    pub job_ad: &'a str,
    /// The posting's top requirements, as the JD-analysis step extracted them.
    pub top_requirements: &'a [String],
    /// The language the document was asked to be written in (`"en"`, `"de-DE"`).
    pub target_language: &'a str,
    pub doc_kind: DocKind,
}

/// One problem found in the generated content.
///
/// `Serialize` only: `code` is a `&'static str` from [`CONTENT_ISSUE_CODES`],
/// which cannot round-trip through `Deserialize` for an arbitrary lifetime.
/// Persisted reports travel as JSON and are read back as `serde_json::Value`,
/// the same way every command in this crate returns its payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentIssue {
    pub severity: Severity,
    /// Stable machine code from [`CONTENT_ISSUE_CODES`]; the renderer's i18n key.
    pub code: &'static str,
    /// Section name, or `None` for a document-wide finding.
    pub section: Option<String>,
    /// Guidance-framed English text. The renderer localizes off `code`; this is
    /// the fallback and the developer-readable form.
    pub message: String,
    /// The exact offending span or compared term — what makes the issue
    /// checkable by the user instead of an assertion they have to trust.
    pub evidence: Option<String>,
}

/// Document-level numbers. These score the DOCUMENT, never the person.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentMetrics {
    /// Share of the posting's keywords the generated document covers (0–100).
    /// `None` when the posting has no extractable keywords.
    pub keyword_coverage: Option<f64>,
    /// How many `top_requirements` the generated document evidences. `None`
    /// when nothing was measured — an uncomparable posting, an empty
    /// requirements list, or a cover letter (which never runs the alignment
    /// pass) — because a rendered `0` claims a measurement that was never taken.
    pub top_requirement_hits: Option<u32>,
    /// The denominator for [`Self::top_requirement_hits`]: how many
    /// requirements could be measured at all. `None` exactly when the hit count
    /// is — they are two halves of one measurement, produced together by
    /// [`alignment::RequirementHits`] — so a renderer needs one null check for
    /// the pair. Lower than the requirements LIST when a requirement has no
    /// extractable keywords, and `0` when none of them had any.
    pub top_requirements_measured: Option<u32>,
    /// Share of bullets involved in at least one near-duplicate pair (0–1).
    pub duplicate_ratio: f64,
    pub roles_source: u32,
    pub roles_output: u32,
}

/// The verdict. `ok` is false only when a Critical is present.
/// `Serialize` only, for the same reason as [`ContentIssue`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentReport {
    pub ok: bool,
    pub issues: Vec<ContentIssue>,
    pub metrics: ContentMetrics,
}

/// Byte cap on [`ContentIssue::message`], enforced in [`issue`]. [`MAX_CONTENT_ISSUES`]
/// (M-3) bounds the issue *count*, not the *size* of any one issue — and
/// `ats.long_bullet` / `ats.header_in_body` / `duplicate.bullet` all quote an
/// offending span verbatim (`long_bullet` fires *because* the bullet is long,
/// so its evidence is unbounded by construction).
///
/// Sized together with [`ISSUE_EVIDENCE_MAX_BYTES`] and
/// [`ISSUE_SECTION_MAX_BYTES`], which are the OTHER two fields carrying text
/// copied out of an untrusted document. Worst case per sub-report:
/// `MAX_CONTENT_ISSUES` (200) × (400 + 400 + 120 + ~150 bytes of JSON overhead
/// for the rest of a `ContentIssue`) ≈ 214 KB raw — roughly double that once
/// JSON escaping is priced in — against `QUALITY_REPORT_MAX_BYTES` (1 MiB,
/// `commands::ai_generations::ai_generations_save`) for a wrapper that also
/// holds a second sub-report.
///
/// The point of the arithmetic is that every term in it is BOUNDED — that is
/// what keeps the save path's drop-to-sentinel branch off the table for
/// realistic content, where an issue's message is one sentence and its section
/// a two-word heading, and what keeps even a hostile document at a fixed
/// multiple of these constants instead of at the length of whatever the model
/// emitted. `section` was the term that broke it: it was copied verbatim, and
/// an ATX heading (`# …`) has no length rule anywhere in the parser, so a 1 KB
/// heading multiplied by the per-line issues underneath it made the total
/// unbounded and the claim false.
pub const ISSUE_MESSAGE_MAX_BYTES: usize = 400;

/// Byte cap on [`ContentIssue::evidence`]. See [`ISSUE_MESSAGE_MAX_BYTES`] for
/// the arithmetic this is sized against.
pub const ISSUE_EVIDENCE_MAX_BYTES: usize = 400;

/// Byte cap on [`ContentIssue::section`]. Much smaller than the other two
/// because a section LABEL is short — "PROFESSIONAL EXPERIENCE" is 23 bytes and
/// the longest heading any template renders is well inside this — while the
/// value itself is untrusted: `section` is a heading line copied out of the
/// generated document, and an ATX heading (`# …`) has no length rule anywhere
/// in the parser. Unclamped, one 1 KB heading multiplied by the per-line issues
/// under it (`ats.long_bullet`, `duplicate.bullet`, …) put the serialized
/// report back over the save path's clamp that [`MAX_CONTENT_ISSUES`] exists to
/// keep it under. Folded into that arithmetic in [`ISSUE_MESSAGE_MAX_BYTES`].
pub const ISSUE_SECTION_MAX_BYTES: usize = 120;

/// `…` truncation marker appended by [`clamp_issue_text`] when it cuts
/// anything, so a clamped span reads as visibly cut rather than as the whole
/// span. Its bytes come out of the budget (not added on top), so the result
/// never exceeds `max` — the arithmetic on [`ISSUE_MESSAGE_MAX_BYTES`] stays
/// exact.
const TRUNCATION_MARKER: &str = "…";

/// Clamp `s` to at most `max` bytes, UTF-8 char-boundary safe (delegates to
/// [`crate::applications::clamp_to_bytes`] rather than forking a second
/// truncation routine), reserving room for [`TRUNCATION_MARKER`] so a cut
/// result is still at most `max` bytes, never `max` bytes plus the marker.
fn clamp_issue_text(s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let budget = max.saturating_sub(TRUNCATION_MARKER.len());
    let mut clamped = crate::applications::clamp_to_bytes(s, budget);
    clamped.push_str(TRUNCATION_MARKER);
    clamped
}

/// Build an issue, reading its severity from [`CONTENT_ISSUE_CODES`].
///
/// All THREE untrusted fields — `message`, `evidence` and `section` — are
/// clamped here, at the one chokepoint every call site routes through, so no
/// validator has to remember to bound its own span. `section` is untrusted for
/// the same reason the other two are: it is a heading line copied out of the
/// generated document.
pub(crate) fn issue(
    code: &'static str,
    section: Option<&str>,
    message: impl Into<String>,
    evidence: Option<String>,
) -> ContentIssue {
    ContentIssue {
        severity: severity_for(code),
        code,
        section: section.map(|s| clamp_issue_text(s.to_string(), ISSUE_SECTION_MAX_BYTES)),
        message: clamp_issue_text(message.into(), ISSUE_MESSAGE_MAX_BYTES),
        evidence: evidence.map(|e| clamp_issue_text(e, ISSUE_EVIDENCE_MAX_BYTES)),
    }
}

// ── Thresholds shared across validators ─────────────────────────────────────

/// Hard cap on [`ContentReport::issues`]' length. M-3: without this, a
/// pathological/hostile "generated" document (thousands of forged roles,
/// duplicate bullets, etc.) can grow the serialized report without bound —
/// past the save path's `QUALITY_REPORT_MAX_BYTES` (1 MiB,
/// `commands::ai_generations::ai_generations_save`) byte clamp. That clamp
/// truncates mid-JSON, the stored blob becomes unparseable, and
/// `ai_generations::merge_quality_report` then silently falls back to
/// keeping the OLD stored report — a fresh report just vanishes with no
/// error anywhere. Capping the issue list here, at the source, keeps the
/// serialized report comfortably under that clamp so the truncate-then-fail
/// path is unreachable. Mirrors the same count-cap discipline the now-deleted
/// `agent::tools_quality::MAX_ISSUES` used, sized higher (200 vs. 20) because this report is
/// the FULL quality-report panel's data, not a token-budgeted tool summary.
pub const MAX_CONTENT_ISSUES: usize = 200;

// ── Analysis context ────────────────────────────────────────────────────────

/// One parsed section of a document: everything from a heading up to the next.
pub(crate) struct Section {
    /// `None` for the leading band before the first heading (name + contact).
    pub heading: Option<String>,
    pub kind: SectionKind,
    pub lines: Vec<ParsedLine>,
}

impl Section {
    pub fn bullets(&self) -> impl Iterator<Item = &ParsedLine> {
        self.lines
            .iter()
            .filter(|l| matches!(l.kind, LineKind::Bullet))
    }
}

/// The stemming decision for a comparison the POSTING is not a party to.
///
/// [`Analysis::tokens`] answers the résumé↔posting question, and its decision is
/// `languages_align(job_ad, target_language)`. Three checks compare texts this
/// report OWNS against each other — a document's skills section against its own
/// experience, a generated title against the source title at the same employer,
/// two bullets of one document — and for those the ad is a third party whose
/// language silently decided whether two halves of one comparison could match.
/// An English ad for a German-language role (the ordinary DACH case) switched
/// stemming off and made every German declension pair look like a mismatch.
///
/// So the decision is taken on the pair actually being compared and the stemmer
/// is read from the SAME text the decision was read from — the
/// `documents::evidence::JobVocabulary` pattern — so the two can never disagree
/// about which language is being stemmed. Both sides of every comparison are
/// stemmed or neither is.
///
/// The text is the GENERATED document, against [`Analysis::lang`]: it is the one
/// the target language is a statement about, this module already trusts that to
/// pick a function-word list, and [`Analysis::language_mismatch`] is what
/// withdraws the trust. Detection alone was rejected for the R5-F2 reason —
/// `whatlang` misreads terse tech résumés, and a misread would silently pick the
/// wrong Snowball algorithm; under this pairing a misread simply fails
/// `languages_align` and falls back to unstemmed.
pub(crate) struct DocumentTokens {
    stemmer: Stemmer,
    aligned: bool,
}

impl DocumentTokens {
    fn of(text: &str, lang: &str) -> Self {
        Self {
            aligned: languages_align(text, lang),
            stemmer: make_stemmer(text),
        }
    }

    /// Tokenize `text` under this decision. Stemming can only MERGE tokens, so
    /// every consumer of this that reports a difference (a skill with no
    /// backing, a drifted title) can only ever go quieter, never louder.
    pub fn tokens(&self, text: &str) -> HashSet<String> {
        if self.aligned {
            keywords(text, &self.stemmer)
        } else {
            keywords_normalized(text)
        }
    }

    /// Stem → readable form for the tokens of `text`, under the SAME decision
    /// [`Self::tokens`] used. A caller that names a token in a message owes its
    /// readable form to the decision it tokenized under: two maps keyed on
    /// different stemmers is how a display form comes back as a stem.
    pub fn display(&self, text: &str) -> HashMap<String, String> {
        if self.aligned {
            display_forms(text, &self.stemmer)
        } else {
            HashMap::new()
        }
    }
}

/// Everything the validators share, resolved once.
///
/// The single `aligned` decision matters: `alignment` compares the generated
/// document's coverage against the SOURCE's coverage of the same posting, and
/// two coverages computed under different stemming rules are not comparable.
/// It is derived from `target_language` (what both documents are supposed to be
/// written in), not from each document's own detected language.
pub(crate) struct Analysis<'a> {
    pub input: &'a ContentInput<'a>,
    /// `target_language` narrowed to a 2-char lowercase ISO-639-1 code.
    pub lang: String,
    pub generated_sections: Vec<Section>,
    pub source_sections: Vec<Section>,
    pub aligned: bool,
    pub stemmer: Stemmer,
    /// The stemming decision for the checks the POSTING is not a party to. See
    /// [`DocumentTokens`].
    pub document: DocumentTokens,
    pub job_keywords: HashSet<String>,
    pub generated_keywords: HashSet<String>,
    pub source_keywords: HashSet<String>,
    /// The generated text is not in the target language. Every posting
    /// comparison is suppressed while this holds — coverage across two
    /// languages is noise, and a cascade of derived warnings would bury the one
    /// finding that matters.
    pub language_mismatch: bool,
}

impl<'a> Analysis<'a> {
    pub fn new(input: &'a ContentInput<'a>) -> Self {
        let lang = normalize_language(input.target_language);
        // ONE alignment decision for every résumé↔posting comparison in this
        // report, taken by the same `languages_align` kernel `score_one` and
        // `rank_bullets` route through. What that guarantees is SYMMETRIC
        // NORMALIZATION — both sides of every comparison here are stemmed, or
        // neither is, on the same rule the match score uses. It does not make
        // this report's numbers equal to the match score: they count different
        // corpora (a generated document vs. a stored résumé) and round
        // differently.
        let aligned = languages_align(input.job_ad, &lang);
        let stemmer = make_stemmer(input.job_ad);
        let tokens = |text: &str| {
            if aligned {
                keywords(text, &stemmer)
            } else {
                keywords_normalized(text)
            }
        };
        Self {
            document: DocumentTokens::of(input.generated, &lang),
            generated_sections: split_sections(input.generated, input.doc_kind),
            // Always a résumé, whatever kind is being VALIDATED: a cover letter
            // is still measured against the candidate's own résumé, and that
            // document still needs the Title-Case heading repair.
            source_sections: split_sections(input.source_resume, DocKind::Resume),
            lang: lang.clone(),
            job_keywords: tokens(input.job_ad),
            generated_keywords: tokens(input.generated),
            source_keywords: tokens(input.source_resume),
            language_mismatch: document_language_mismatch(
                input.generated,
                input.source_resume,
                input.job_ad,
                input.target_language,
            ),
            aligned,
            stemmer,
            input,
        }
    }

    /// Tokenize `text` the same way both sides of this report were tokenized.
    ///
    /// Stems are an implementation detail of a comparison and must never reach a
    /// message: a token taken straight from here reads as `kubernet`,
    /// `develop`, `entwickl`, and telling a user their résumé never demonstrates
    /// "kubernet" is a finding they cannot act on. This context used to carry a
    /// stem → readable map for that, built over all three documents under THIS
    /// (posting-keyed) alignment decision — but the one check that interpolated
    /// a token into a message is `consistency::skill_not_demonstrated`, which
    /// compares a document against ITSELF and therefore needs its own
    /// document-keyed stemmer AND its own map to match. Two maps keyed on
    /// different stemmers is how a display form comes back as a stem, so this
    /// one is gone rather than kept for a caller that no longer exists. Any
    /// future check that names a token owes its readable form to the same
    /// decision it tokenized under.
    pub fn tokens(&self, text: &str) -> HashSet<String> {
        if self.aligned {
            keywords(text, &self.stemmer)
        } else {
            keywords_normalized(text)
        }
    }

    /// Coverage of the posting by `tokens`, 0–100. `None` when the posting has
    /// no extractable keywords (a sparse or garbled ad) — the caller must go
    /// quiet rather than report 0%.
    pub fn coverage(&self, tokens: &HashSet<String>) -> Option<f64> {
        keyword_coverage(&self.job_keywords, tokens).map(|(c, _)| c)
    }

    /// Whether every posting comparison should be skipped: nothing extractable
    /// on the posting side, or the output is not in the target language.
    ///
    /// Deliberately does NOT include [`Self::posting_language_diverges`] —
    /// see that method for the counter-example.
    pub fn posting_comparable(&self) -> bool {
        !self.job_keywords.is_empty() && !self.language_mismatch
    }

    /// The posting is RELIABLY in a language other than the target's.
    ///
    /// A check that INTERSECTS the posting's vocabulary with a document's — as
    /// opposed to comparing two documents' coverage of the posting against each
    /// other — is meaningless when the two are written in different languages:
    /// it counts how many foreign words happen to appear in native prose, gets
    /// ~0, and reports that as a finding about the document. That is the
    /// ordinary DACH case, where an English-language ad advertises a
    /// German-speaking role: nothing is wrong with either text, and
    /// `voice.generic_letter` told the candidate their letter "could have been
    /// sent to anyone" while it named the posting's own subject matter in every
    /// sentence.
    ///
    /// **Not folded into [`Self::posting_comparable`]**, though it looks like a
    /// third premise of the same rule. `aligned` compares the ad to the TARGET
    /// LANGUAGE, which is a user setting, not to the documents — and when the
    /// target disagrees with everything else
    /// (`language_critical_is_withheld_when_the_source_reads_the_same_way`: a
    /// German ad, a German source, a German output, `target_language: "en"`)
    /// the ad and the documents still match each other, so coverage and the
    /// source-vs-generated alignment comparison are real measurements that must
    /// survive. Only the ad↔document INTERSECTION is invalid there, and only
    /// `generic_letter` computes one.
    ///
    /// The reliability half is the same R5-F2 concern the language Critical
    /// guards against: `languages_align` answers `false` for a MISDETECTED
    /// language just as readily as for a real one, and a terse ad ("Terraform
    /// AWS PostgreSQL Kubernetes platform engineer") is a keyword soup the
    /// detector reads as anything at all. This check stays on the SAME
    /// [`MIN_CHARS_FOR_LANGUAGE_CHECK`] floor `language.rs`'s guards used to
    /// share, rather than `detected_language`'s confidence gate: `aligned`
    /// (above) is computed from `languages_align`, which exposes no
    /// confidence signal to gate on, so length is the only reliability proxy
    /// available here. Suppressing on a short-ad guess would switch the check
    /// off for ordinary short postings.
    fn posting_language_diverges(&self) -> bool {
        !self.aligned && significant_chars(self.input.job_ad) >= MIN_CHARS_FOR_LANGUAGE_CHECK
    }

    pub fn section_of_kind(&self, kind: SectionKind) -> Option<&Section> {
        self.generated_sections.iter().find(|s| s.kind == kind)
    }

    /// EVERY generated section of `kind`, not just the first.
    ///
    /// [`Self::section_of_kind`] answers "the" section — a reasonable
    /// convenience for a check that only ever needs to know ONE occurrence
    /// exists (an empty-section warning, a bullet-count check). It is NOT
    /// reasonable for a check that has to see everything the document claims:
    /// an invented project link that lands in a SECOND Projects section is
    /// invisible to `.find()`, and `factual::project_link_issues` is
    /// Critical-severity, so that blind spot is a real fabrication going
    /// unreported rather than a cosmetic miss. Use this there.
    ///
    /// The two remaining `section_of_kind` consumers
    /// (`consistency::skill_not_demonstrated_issues`,
    /// `consistency::project_structure_issues`) stay on the single-section
    /// form: both are Warning-severity, and the duplicate-section case this
    /// closes is a repair/humanize-introduced one — guarded directly by
    /// `sections::is_usable_replacement`'s single-heading check and
    /// `sections::matches_requested_kind`'s identity check, which make a
    /// generated duplicate section rare rather than routine. The residual case
    /// (a user's own résumé, or an import, already carrying two sections of a
    /// kind) is pre-existing input-quality noise, not something this pipeline
    /// introduced, and a missed Warning there costs a lot less than a missed
    /// Critical.
    pub fn generated_sections_of_kind(&self, kind: SectionKind) -> impl Iterator<Item = &Section> {
        self.generated_sections
            .iter()
            .filter(move |s| s.kind == kind)
    }

    /// EVERY source section of `kind` — the SOURCE-side mirror of
    /// [`Self::generated_sections_of_kind`]. `factual::project_link_issues` used
    /// to read only the FIRST source section of a kind on this side while the
    /// generated side already read every one, so a SECOND source Projects
    /// section's links (`SECTION_NAMES` recognises both "projects" and "side
    /// projects", and both classify `Projects`) dropped out of the sourced set
    /// — a document that changed nothing accused itself of inventing its own
    /// link.
    pub fn source_sections_of_kind(&self, kind: SectionKind) -> impl Iterator<Item = &Section> {
        self.source_sections.iter().filter(move |s| s.kind == kind)
    }
}

/// Narrow any incoming language value to a 2-letter lowercase code, defaulting
/// to `"en"`. Mirrors `normalizeLanguageCode` in `natural-voice.ts`.
///
/// L-3 fix: filters to alphanumeric characters BEFORE taking the first 2 —
/// `.trim()` only strips LEADING/TRAILING whitespace, so a control character
/// in the middle (`"a\nb"`) used to survive into the 2-char result (`"a\n"`).
/// That result is interpolated into this module's `validate:content` span
/// text (`format!("kind={} lang={}", …)`) and becomes `ctx.lang`, which
/// reaches `content.language_mismatch`'s user-facing `evidence` — a raw
/// newline in either is a log-injection primitive (ADR-027-adjacent).
/// Filtering to alphanumeric also makes a tag like `"en-US"`/`"de_DE"`
/// resolve identically to before (the separator was never part of the first
/// 2 characters anyway).
pub(crate) fn normalize_language(language: &str) -> String {
    let code: String = language
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(2)
        .collect::<String>()
        .to_lowercase();
    if code.is_empty() {
        "en".to_string()
    } else {
        code
    }
}

/// Characters that carry signal for language detection — everything but
/// whitespace. The one definition of "how much text is this really", shared by
/// [`language`]'s guards and [`Analysis::posting_language_diverges`], which
/// measure two different documents against the same bar.
fn significant_chars(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

/// How many words a line may run to and still read as a HEADING rather than as
/// content. Four covers "Beruflicher Werdegang", "Weitere technische
/// Kenntnisse" and "Compétences techniques"; past that a line is a sentence.
const MAX_PROMOTED_HEADING_WORDS: usize = 4;

/// Whether a line `export::parser` did NOT classify as a heading should be read
/// as one anyway.
///
/// `parse_resume`'s heading test is an EXACT match against its own
/// `SECTION_NAMES` list, an ATX `#` marker, or ALL-CAPS. Between them those
/// cover a lot — every ALL-CAPS heading in any language, and the exact
/// single-word entries "Berufserfahrung", "Ausbildung", "Formation",
/// "Expérience professionnelle" — but they miss every Title-Case heading
/// OUTSIDE that exact list: "Beruflicher Werdegang", "Berufliche Erfahrung",
/// "Technische Kenntnisse", "Kurzprofil", "Compétences techniques".
/// `documents::evidence::classify_section` classifies all five correctly from
/// the shared multilingual heading lexicon, so the vocabulary is not missing —
/// only this module's access to it was.
///
/// What that cost is not cosmetic: a résumé whose headings are all Title-Case
/// collapses into ONE section, `factual::metric_lines` reads `has_headings` as
/// false, and the COVER-LETTER rules (the 8-word body latch) run over a résumé
/// — deleting every short source line, the candidate's own figures included,
/// which turns restating them into a fabrication Critical.
///
/// The lexicon alone is not enough to promote on, because it is a SUBSTRING
/// match built for text the parser already decided was a heading: "Improved
/// user experience by 20%" carries `experience` and "Cloud Kubernetes Docker"
/// carries `cloud`. So a promoted line must also LOOK like a heading, on
/// signals that hold in every language this pipeline supports:
///
/// * the parser left it as plain `Text`/`Name` — never a bullet, entry,
///   contact, job title or existing heading;
/// * it opens a block (first line, or preceded by a blank), which is where a
///   heading sits and where a line in the middle of a list does not;
/// * at most [`MAX_PROMOTED_HEADING_WORDS`] words and 60 characters;
/// * no digits and no sentence/column punctuation. The digit rule is
///   load-bearing beyond shape: a promoted line leaves the section's `lines`,
///   so this guarantees promotion can never remove a FIGURE from the source's
///   metric set, which is `factual::metric_lines`' second invariant.
fn reads_as_heading(line: &ParsedLine) -> bool {
    if !matches!(line.kind, LineKind::Text | LineKind::Name) {
        return false;
    }
    let text = line.text.trim();
    !text.is_empty()
        && text.chars().count() <= 60
        && word_count(text) <= MAX_PROMOTED_HEADING_WORDS
        && !text.chars().any(|c| c.is_ascii_digit())
        && !text.contains([
            '.', ',', ';', ':', '!', '?', '|', '·', '•', '@', '(', ')', '/',
        ])
        && classify_section(text) != SectionKind::Other
}

/// Whether the line DIRECTLY below a heading candidate opens an employment
/// entry — which makes the candidate that entry's job TITLE, not a heading.
///
/// ## The shape [`reads_as_heading`] cannot tell apart on its own
///
/// A job title on its own line above the employer is one of the two ordinary
/// experience layouts, and the extracted-PDF one:
///
/// ```text
/// BERUFSERFAHRUNG
///
/// Projektleiter                         ← the candidate
/// Acme Payments · Berlin · 2021 – Heute ← the entry it labels
/// - …
/// ```
///
/// Every shape guard passes: `export::parser`'s `JobTitle` arm needs the
/// PREVIOUS line to carry a two-space date column and this one is blank, so the
/// parser leaves it `Text`; it opens a block, because that is where an entry
/// block starts; it is one word, digit-free and punctuation-free. And
/// `classify_section` matches the `projekt`/`project` stem, so "Projektleiter",
/// "Project Manager", "Senior Project Manager" and "Technical Project Lead" all
/// became a `SectionKind::Projects` heading in the MIDDLE of the experience
/// section — which takes the entries below out of
/// [`factual::count_roles`]'s reach (a résumé reporting zero roles), grades their
/// bullets as malformed project cards (`consistency.project_structure`), and can
/// hand `factual::project_link_issues` a phantom projects section to compare the
/// source's real links against.
///
/// ## Why the line BELOW is the discriminator
///
/// A heading sits above a BLOCK; a title sits above a LINE. What follows a real
/// heading is a blank, a bullet, a stack line or a prose paragraph — what
/// follows a title-above-employer is the employer line itself. So the one signal
/// that separates them without a title vocabulary is: does the next line OPEN A
/// ROLE?
///
/// That question already has an owner. `documents::evidence::extract_evidence`
/// opens a role on exactly two shapes, and this reuses both rather than writing
/// a third opinion: a `LineKind::JobEntry` (the parser's two-space, pipe/middot
/// and parenthesized forms), or an unrecognised line ending in a real date
/// COLUMN ([`trailing_date_column`], which takes a column and not a mentioned
/// year — "Acme Payments, Berlin, 2018 - 2021"). A heading and an entry label
/// cannot drift apart about what an entry line is.
///
/// *Residual, stated:* an employer written across two lines with no date on the
/// first ("Projektleiter" / "Acme Payments" / "Berlin" / "2019 – 2021") still
/// promotes the title, because nothing on the line below says an entry started.
/// It costs a section split, not a false accusation.
fn labels_the_entry_below(next: Option<&ParsedLine>) -> bool {
    next.is_some_and(|line| {
        matches!(line.kind, LineKind::JobEntry) || trailing_date_column(&line.text).is_some()
    })
}

/// Split a document into sections at its headings. The leading band before the
/// first heading (name + contact) is always section 0 with `heading: None`, so
/// "is this in a non-first section?" is just an index test.
///
/// A line the parser did not recognise is promoted to a heading by
/// [`reads_as_heading`], **per line**.
///
/// ## Why the document-wide gate is gone
///
/// The promotion used to run only in a document where `export::parser` found NO
/// heading at all — the same "no better signal was available" rule
/// `documents::evidence`'s unclassified-section fallback uses. That reading
/// assumed the parser's `SECTION_NAMES` was English-only, and it is not: it
/// carries "ausbildung", "kenntnisse", "sprachen", "formation", "compétences"
/// and their siblings in seven locales. So ONE conventional single-word heading
/// ("Ausbildung") switched promotion off for the whole document — including for
/// this repair's own headline case, "Beruflicher Werdegang", in the completely
/// ordinary mixed résumé that heads three of its four sections in Title-Case and
/// the fourth in a word the list happens to hold. That document then reports the
/// Experience and Skills sections it visibly has as MISSING, and counts none of
/// its roles.
///
/// ## What defends the promotion instead
///
/// The risk the gate was covering is an ordinary prose line resembling a heading
/// in the MIDDLE of a well-headed document, which would split a role in half. It
/// is covered by [`reads_as_heading`]'s own shape guards, which is where a
/// statement about what a heading LOOKS like belongs: the classifier must
/// recognise the line (a generic shape is never promoted), the parser must have
/// left it as plain `Text`/`Name`, it must open a block, and it must carry no
/// digits and no sentence/column punctuation. A stack line or a bullet
/// continuation fails the block test; a sentence fails the length and
/// punctuation tests; and the digit rule still guarantees promotion cannot
/// remove a FIGURE from the source's metric set.
///
/// …and it must not be the LABEL of the entry underneath it
/// ([`labels_the_entry_below`], which is what keeps an ordinary job title above
/// its employer out of the heading list).
///
/// *Residual, stated:* a ≤4-word, digit-free, punctuation-free `Text` line that
/// opens a block, carries a heading stem ("Cloud Kubernetes Docker" would, at
/// four words) and is not followed by an entry line is promoted wherever it
/// sits. That was already accepted for a heading-less document; it is the same
/// error, now reachable in a headed one, and it costs a section split rather
/// than a false accusation.
///
/// ## Promotion is a RÉSUMÉ repair — `doc_kind` decides, per text
///
/// Everything above is an argument about documents that HAVE sections. A cover
/// letter has none, so `export::parser` never finds a heading in one and every
/// short label line in it — "My Experience", "Kurzprofil", "Zu meiner Person" —
/// satisfies every shape guard there is. One of them is enough to take
/// `factual::metric_lines`' `sections.len() > 1` test from false to true, which
/// switches that pass from the LETTER rules to the résumé ones: section 0 —
/// everything above the label, i.e. most of the letter — is then skipped by
/// POSITION on the claims side, and the numbers in the letter's opening stop
/// being checked against the source at all. Silencing rather than accusing, but
/// structural: the letter loses the check the whole family exists for.
///
/// The parameter is the kind of THIS TEXT, not of the report, which is why it is
/// threaded rather than read off `ContentInput`: when the report is validating a
/// LETTER, the source résumé it is measured against is still a résumé and still
/// needs the repair.
pub(crate) fn split_sections(text: &str, doc_kind: DocKind) -> Vec<Section> {
    let lines = parse_resume(text).lines;
    let mut sections = vec![Section {
        heading: None,
        kind: SectionKind::Other,
        lines: Vec::new(),
    }];
    let mut opens_a_block = true;
    let mut lines = lines.into_iter().peekable();
    while let Some(line) = lines.next() {
        let is_heading = matches!(line.kind, LineKind::SectionHeader)
            || (doc_kind == DocKind::Resume
                && opens_a_block
                && reads_as_heading(&line)
                && !labels_the_entry_below(lines.peek()));
        opens_a_block = matches!(line.kind, LineKind::Blank);
        if is_heading {
            sections.push(Section {
                kind: classify_section(&line.text),
                heading: Some(line.text.clone()),
                lines: Vec::new(),
            });
        } else if let Some(current) = sections.last_mut() {
            current.lines.push(line);
        }
    }
    sections
}

// ── Small shared text helpers ───────────────────────────────────────────────

/// A phone number as a résumé HEADER actually writes one — the single shape
/// test behind every "is this line contact details?" question in this module.
///
/// Deliberately stricter than `export::parser::PHONE_RE`
/// (`\+?\d[\d\s\-().]{7,}`), which accepts any seven-character run of digits,
/// spaces, hyphens, dots and parens. That rule is right where it lives: the
/// parser only has to decide which BAND a header line belongs to, and
/// over-matching costs it nothing. It is wrong on both surfaces here, where the
/// answer decides whether a user is accused of something or spared a check —
/// ordinary numeric prose satisfies it constantly ("150 - 200 EUR per hour",
/// "90 000 - 110 000"), which made a salary range a second contact block
/// (`ats.header_in_body`, a Critical) and let a letter paragraph quoting a rate
/// range exempt itself from the fabricated-metric pass.
///
/// Two accepted forms, between them covering the header formats this pipeline's
/// own fixtures use in `en` and `de`:
///
/// 1. an explicit international/area-code marker — a leading `+` or `(`
///    followed by digits (`+49 30 1234567`, `+49 (0)30 1234567`,
///    `(030) 12345678`, `+1 (555) 123-4567`);
/// 2. failing that, an unbroken run of seven or more digits — the local part of
///    a German number written without a marker (`030 1234567`,
///    `0176 12345678`).
///
/// A grouped figure carries at most three digits per group and no marker, so it
/// matches neither. The cost is a MISSED match on a bare US-style number with no
/// parentheses ("555-123-4567", longest run four): accepted, because a header
/// block essentially always carries an email as well, which every caller tests
/// separately, and because both callers would rather miss than over-reach.
static HEADER_PHONE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[+(]\s*\d[\d\s\-./()]{5,}\d|\d{7,}").unwrap());

/// Whether `text` carries a header-shaped phone number **and no date span**. See
/// [`HEADER_PHONE_RE`]; `pub(crate)` so `ats::is_contact_cluster` and
/// [`has_real_contact_match`] cannot drift apart again.
///
/// ## Why the span test is part of the SHAPE, not of a call site
///
/// The `[+(]` arm matches a parenthesized DATE SPAN exactly as readily as an
/// area code: `(2019 - 2021)` is `(`, a digit, eight separator-or-digit
/// characters and a digit. `ats::is_contact_cluster` carried its own guard
/// against precisely that, and the other caller — [`has_real_contact_match`] —
/// did not, so the two answers to one question disagreed. A pre-heading line
/// carrying a span ("Contract work (2019 - 2021): 1 200 000 EUR in payment
/// volume") was read as contact details and struck out of the SOURCE's metric
/// set, and restating its figure came back as a fabrication Critical. One
/// helper, one guard, both callers.
///
/// ## Why the guard is [`date_spans`] and not "carries a year"
///
/// The statement being made is *a date span is not a phone number*, and the
/// first cut of it tested for a bare 1900–2099 run instead — strictly broader
/// than the shape it named, and it wrote off the difference as an accepted
/// missed skip ("+49 30 2019 1234" is a perfectly ordinary German number). That
/// was the wrong cost direction. `factual::metric_lines`' SOURCE side still
/// drops such a line from the truth set inside the contact band (it is a bare
/// phone line by shape), while the letter that repeats the same number keeps it
/// (this test said it was not contact details) — so the two sides dropped one
/// line through two different rules and the candidate's own phone digits came
/// back as a fabricated metric. A drop from the truth set that the claims side
/// does not mirror is an accusation channel, not a missed check.
///
/// [`date_spans`] requires a span SEPARATOR between two years, so it matches the
/// statement exactly: `(2019 - 2021)`, `(Jan 2019 – Mar 2021)` and `2018 to
/// 2021` are refused, and a subscriber number that happens to contain one
/// year-shaped run is contact details again — on BOTH callers, which is the
/// whole point of the guard living here.
pub(crate) fn looks_like_header_phone(text: &str) -> bool {
    HEADER_PHONE_RE.is_match(text) && date_spans(text).is_empty()
}

/// A line that really does carry contact details: a genuine email address, or a
/// header-shaped phone number on a line with no `@` at all.
///
/// Neither half may be the parser's own rule. `is_first_line_contact_shaped`
/// accepts a bare `@`, which any body line mentioning a Slack handle or a
/// `@decorator` satisfies — so a bullet could hide its numbers behind "this is
/// the header" just by carrying one — and its phone half is the loose
/// `PHONE_RE` that [`HEADER_PHONE_RE`] exists to replace.
///
/// `factual::metric_lines` routes its heading-less (cover-letter) band skip
/// through this; `ats::is_contact_cluster` applies the same two halves inline,
/// because it additionally has to reject a date range and to know which of the
/// two matched.
pub(crate) fn has_real_contact_match(text: &str) -> bool {
    if text.contains('@') {
        return EMAIL_RE.is_match(text);
    }
    looks_like_header_phone(text)
}

/// Split prose into sentences on `.`/`!`/`?`.
///
/// Paragraphs (blank-line separated) are unwrapped first: hard-wrapped prose is
/// normal in a generated letter, and splitting on the wrap instead of the
/// sentence would report a uniform ~12-word rhythm for every document and make
/// the burstiness check fire on everything.
///
/// Abbreviations ("e.g.") over-split, which is acceptable for the advisory
/// rhythm checks that consume this.
pub(crate) fn sentences(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n");
    let mut out = Vec::new();
    for paragraph in normalized.split("\n\n") {
        let unwrapped = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
        for fragment in unwrapped.split_inclusive(['.', '!', '?']) {
            let sentence = fragment.trim().trim_matches(['.', '!', '?']).trim();
            if !sentence.is_empty() {
                out.push(sentence.to_string());
            }
        }
    }
    out
}

pub(crate) fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Lowercase `text` with every whitespace run collapsed to one space, so a
/// multi-word phrase still matches across the hard wraps a generated letter is
/// full of ("Studies\nshow" must match "studies show").
pub(crate) fn flattened_lower(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Jaccard similarity of two token sets. `0.0` when both are empty (callers
/// must not treat "nothing in common because there is nothing" as identity).
pub(crate) fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let union = a.union(b).count();
    if union == 0 {
        return 0.0;
    }
    a.intersection(b).count() as f64 / union as f64
}

// ── Entry point ─────────────────────────────────────────────────────────────

/// Run every deterministic content check for `input`.
///
/// `ok` is `false` exactly when a Critical is present. Ordering is stable
/// (family by family, document order within a family) so a saved report and a
/// snapshot test both stay reproducible.
pub fn validate_content(input: &ContentInput) -> ContentReport {
    let span = Span::begin(
        "validate:content",
        format!(
            "kind={} lang={}",
            match input.doc_kind {
                DocKind::Resume => "resume",
                DocKind::CoverLetter => "cover_letter",
            },
            normalize_language(input.target_language)
        ),
    );

    let ctx = Analysis::new(input);
    let mut issues = Vec::new();
    issues.extend(language_issues(&ctx));

    // Every doc-kind-specific metric is decided in this one match, letter arm
    // included: a cover letter has no employment entries, so reporting the
    // SOURCE résumé's role count next to the letter's own zero rendered a
    // "2 → 0" roles drop in the quality panel on a perfectly good letter.
    let (requirement_hits, duplicate_ratio, roles_source, roles_output) = match input.doc_kind {
        DocKind::CoverLetter => {
            issues.extend(letter::validate(&ctx));
            issues.extend(credentials::validate(&ctx));
            (None, 0.0, 0, 0)
        }
        DocKind::Resume => {
            issues.extend(factual::validate(&ctx));
            issues.extend(credentials::validate(&ctx));
            let (alignment_issues, hits) = alignment::validate(&ctx);
            issues.extend(alignment_issues);
            issues.extend(consistency::validate(&ctx));
            let (duplicate_issues, ratio) = duplicates::validate(&ctx);
            issues.extend(duplicate_issues);
            issues.extend(ats::validate(&ctx));
            issues.extend(voice::validate(&ctx));
            (
                hits,
                ratio,
                factual::count_roles(&ctx.source_sections) as u32,
                factual::count_roles(&ctx.generated_sections) as u32,
            )
        }
    };

    let metrics = ContentMetrics {
        keyword_coverage: ctx
            .posting_comparable()
            .then(|| ctx.coverage(&ctx.generated_keywords))
            .flatten(),
        // Both halves of the ratio come from the same `Option`, so they cannot
        // drift into "2 hits out of nothing".
        top_requirement_hits: requirement_hits.as_ref().map(|h| h.hits),
        top_requirements_measured: requirement_hits.as_ref().map(|h| h.measured),
        duplicate_ratio,
        roles_source,
        roles_output,
    };

    // `ok` reads the criticals count from the FULL, pre-truncation list —
    // capping the visible `issues` below must never flip a genuinely-blocking
    // report to "ok".
    let criticals = issues
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .count();
    // Codes and counts only — never résumé, posting or evidence text (ADR-027).
    span.end_with(
        &format!("issues={} criticals={criticals}", issues.len()),
        true,
    );

    cap_issues(&mut issues);

    ContentReport {
        ok: criticals == 0,
        issues,
        metrics,
    }
}

/// Cap an issue list at [`MAX_CONTENT_ISSUES`], criticals first, with ONE
/// visible truncation marker.
///
/// M-3: without this, a pathological/hostile "generated" document (thousands of
/// forged roles, duplicate bullets) grows the serialized report past the save
/// path's `QUALITY_REPORT_MAX_BYTES` clamp, which truncates mid-JSON and makes
/// `merge_quality_report` silently keep the OLD stored report. Criticals sort
/// first (a stable sort, so their relative order survives) so a warning flood
/// can never push a real Critical out of the visible list — only Warnings are
/// ever cut, and the trailing `REPORT_TRUNCATED` marker says so instead of a
/// silent drop.
///
/// **A FUNCTION, and re-runnable, because the report is written twice.**
/// `validate_content` caps what it found; `stages::judge` then merges up to
/// [`MAX_JUDGE_ITEMS`](crate::pipeline::resume::stages::MAX_JUDGE_ITEMS) more
/// into the SAME list at max depth, which put the list back over the bound the
/// `QUALITY_REPORT_MAX_BYTES` derivation rests on — and did it by appending
/// Warnings, exactly the class the criticals-first sort exists to cut first. An
/// existing marker is ABSORBED (its own dropped count carried into the new one)
/// rather than left beside a second one, so calling this again is safe and the
/// count stays truthful.
pub(crate) fn cap_issues(issues: &mut Vec<ContentIssue>) {
    let mut dropped = 0usize;
    issues.retain(|candidate| {
        if candidate.code != REPORT_TRUNCATED {
            return true;
        }
        // `unwrap_or(1)`, not `0`: a marker whose count cannot be read still
        // means "issues were dropped", and defaulting to zero would ABSORB the
        // marker and then decline to re-emit it — turning an unreadable count
        // into a report that silently claims nothing was truncated.
        dropped += candidate
            .evidence
            .as_deref()
            .and_then(|count| count.parse::<usize>().ok())
            .unwrap_or(1);
        false
    });
    if issues.len() > MAX_CONTENT_ISSUES {
        dropped += issues.len() - MAX_CONTENT_ISSUES;
        issues.sort_by_key(|i| i.severity != Severity::Critical);
        issues.truncate(MAX_CONTENT_ISSUES);
    }
    if dropped == 0 {
        return;
    }
    issues.push(issue(
        REPORT_TRUNCATED,
        None,
        format!(
            "{dropped} more issue{} found but not shown here — this document has an \
             unusually large number of findings.",
            if dropped == 1 { "" } else { "s" }
        ),
        Some(dropped.to_string()),
    ));
}
