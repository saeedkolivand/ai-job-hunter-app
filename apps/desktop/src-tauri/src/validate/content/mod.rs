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

use serde::{Deserialize, Serialize};

use crate::documents::evidence::SectionKind;
use crate::observability::Span;
use crate::validate::Severity;

/// The word-boundary matcher every lexicon-style comparison in this module uses.
/// Re-exported rather than reimplemented: `documents::evidence` compares
/// `PRESENT_MARKERS` with the same function, so a date marker and a voice
/// lexicon entry can never disagree about what a word boundary is.
pub(crate) use crate::documents::evidence::contains_word as contains_phrase;

mod alignment;
mod analysis;
mod ats;
mod consistency;
mod credentials;
mod duplicates;
mod factual;
mod issues;
mod language;
mod letter;
pub mod lexicon;
mod text;
mod voice;

use self::analysis::significant_chars;
pub(crate) use self::analysis::{normalize_language, split_sections, Analysis, Section};
pub(crate) use self::issues::{cap_issues, issue};
use self::language::language_issues;
use self::text::{
    flattened_lower, has_real_contact_match, jaccard, looks_like_header_phone, sentences,
    word_count,
};
// Only `tests` (a `super::*` glob import) reaches into these three directly.
pub use self::issues::{
    ISSUE_EVIDENCE_MAX_BYTES, ISSUE_MESSAGE_MAX_BYTES, ISSUE_SECTION_MAX_BYTES, MAX_CONTENT_ISSUES,
};
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
mod tests;

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
/// A tenure the source résumé cannot support — see `credentials`.
///
/// **A Warning, chosen against the shipping criterion rather than by failing
/// it.** The measurement permitted a Critical: zero false positives across 25
/// truthful documents, while reading the tenure sentences this repo's own
/// fixtures write verbatim. What argues the other way is the SHAPE of the
/// input. Two independent review rounds found five distinct registers that
/// produced false Criticals — `$1.2M per year`, `a 30 year old mainframe`,
/// `2015 - Actualidad`, `quinze années`, a block headed `EARLIER ROLES` — and
/// each time the corpus was green before the next register was named. The
/// claims side reads unbounded natural-language prose in seven languages,
/// discriminated by three hand-curated word lists; the corpus can only ever
/// falsify it, never establish it.
///
/// The cost of being wrong is not symmetric either. This code's section is the
/// SUMMARY, which `repair` can regenerate, so a false one would spend provider
/// calls rewriting a correct summary against "offending text: 15 years" and
/// could pressure the model into understating a tenure the candidate really
/// has. A Warning cannot: `criticals_by_section` never sees it.
///
/// `factual.unsourced_certification` stays Critical because its trigger is
/// three bounded vocabularies intersecting, not prose.
pub const FACTUAL_INFLATED_EXPERIENCE: &str = "factual.inflated_experience";
/// A certification the source résumé never names, found by its ACRONYM: an
/// uppercase, word-bounded token from a curated 23-entry list.
///
/// Critical, and the only credential code that is. A certification is the most
/// checkable claim on a résumé — an employer can look it up — and this arm's
/// evidence is genuinely bounded: it reads no prose, and each token means one
/// thing on a résumé in any language. Three review rounds produced no false
/// positive from it.
pub const FACTUAL_UNSOURCED_CERTIFICATION: &str = "factual.unsourced_certification";
/// A credential the source résumé never names, found by an ISSUER beside a
/// certification word in prose (`AWS Certified Solutions Architect`).
///
/// A Warning, split out of `factual.unsourced_certification` because it rests
/// on a different evidence class: unbounded natural language in seven
/// languages, discriminated by three curated vocabularies. It has been measured
/// wrong twice — "Certified the release on AWS each Thursday" (a verb), then
/// "Docker Certified images" (a vendor's adjective) — each time on the first
/// adversarial pass after a green corpus. That is the same shape, and the same
/// cadence, as `factual.inflated_experience`, and it earns the same tier.
/// Keeping one code for both arms would have made the Critical reachable from
/// the prose path.
pub const FACTUAL_UNSOURCED_CREDENTIAL: &str = "factual.unsourced_credential";
/// The generated document names a place of study while the source names none
/// at all. A Warning, deliberately: this is the residue of a value comparison
/// that MEASURED a false positive on truthful cross-language output (see
/// `credentials::unsupported_institutions`), so the surviving check is scoped
/// to a whole invented education section.
///
/// Advisory at the RUN level too, not just in this table — which took a
/// deliberate omission to make true. A Warning listed in
/// `commands::resume_pipeline::report::FABRICATION_CODES` parks its run in
/// `needsReview` until the user decides it, so this code is kept out of that
/// list; "advisory" that blocks a run is not advisory.
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
    (FACTUAL_UNSOURCED_CERTIFICATION, Severity::Critical),
    (CONTENT_LANGUAGE_MISMATCH, Severity::Critical),
    (ATS_HEADER_IN_BODY, Severity::Critical),
    (ATS_EMPTY_SECTION, Severity::Warning),
    (LETTER_TEMPLATE_PLACEHOLDER, Severity::Critical),
    (FACTUAL_UNSOURCED_TERM, Severity::Warning),
    (FACTUAL_INFLATED_EXPERIENCE, Severity::Warning),
    (FACTUAL_UNSOURCED_CREDENTIAL, Severity::Warning),
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
/// makes it a test failure, and `every_emitted_code_is_registered_with_its_declared_severity` in `tests/report_contract.rs`
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
