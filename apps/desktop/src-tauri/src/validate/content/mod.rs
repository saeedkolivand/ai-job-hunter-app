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

use rust_stemmers::Stemmer;
use serde::{Deserialize, Serialize};

use crate::documents::evidence::{classify_section, SectionKind};
use crate::documents::keywords::{
    detect_locale_tag, display_forms, keyword_coverage, keywords, keywords_normalized,
    languages_align, make_stemmer,
};
use crate::export::parser::{is_first_line_contact_shaped, parse_resume};
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
mod duplicates;
mod factual;
mod letter;
pub mod lexicon;
mod voice;

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
pub const CONTENT_LANGUAGE_MISMATCH: &str = "content.language_mismatch";
pub const ALIGNMENT_LOW_COVERAGE: &str = "alignment.low_coverage";
pub const ALIGNMENT_MISSING_TOP_REQUIREMENT: &str = "alignment.missing_top_requirement";
pub const CONSISTENCY_DATE_ORDER: &str = "consistency.date_order";
pub const CONSISTENCY_TITLE_DRIFT: &str = "consistency.title_drift";
pub const CONSISTENCY_SKILL_NOT_DEMONSTRATED: &str = "consistency.skill_not_demonstrated";
pub const CONSISTENCY_PROJECT_STRUCTURE: &str = "consistency.project_structure";
pub const DUPLICATE_BULLET: &str = "duplicate.bullet";
pub const ATS_KEYWORD_DENSITY: &str = "ats.keyword_density";
pub const ATS_HEADER_IN_BODY: &str = "ats.header_in_body";
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
    (CONTENT_LANGUAGE_MISMATCH, Severity::Critical),
    (ATS_HEADER_IN_BODY, Severity::Critical),
    (FACTUAL_UNSOURCED_TERM, Severity::Warning),
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
    /// How many `top_requirements` the generated document evidences.
    pub top_requirement_hits: u32,
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

/// Build an issue, reading its severity from [`CONTENT_ISSUE_CODES`].
pub(crate) fn issue(
    code: &'static str,
    section: Option<&str>,
    message: impl Into<String>,
    evidence: Option<String>,
) -> ContentIssue {
    ContentIssue {
        severity: severity_for(code),
        code,
        section: section.map(str::to_string),
        message: message.into(),
        evidence,
    }
}

// ── Thresholds shared across validators ─────────────────────────────────────

/// Below this many non-whitespace characters, `whatlang` guesses. A Critical
/// language mismatch on a two-line draft would be a false accusation, so the
/// check goes quiet instead.
pub const MIN_CHARS_FOR_LANGUAGE_CHECK: usize = 120;

/// Hard cap on [`ContentReport::issues`]' length. M-3: without this, a
/// pathological/hostile "generated" document (thousands of forged roles,
/// duplicate bullets, etc.) can grow the serialized report toward ~1MB —
/// well past the save path's `QUALITY_REPORT_MAX_BYTES` (256 KiB,
/// `commands::ai_generations::ai_generations_save`) byte clamp. That clamp
/// truncates mid-JSON, the stored blob becomes unparseable, and
/// `ai_generations::merge_quality_report` then silently falls back to
/// keeping the OLD stored report — a fresh report just vanishes with no
/// error anywhere. Capping the issue list here, at the source, keeps the
/// serialized report comfortably under that clamp so the truncate-then-fail
/// path is unreachable. Mirrors `agent::tools_quality::MAX_ISSUES`'s
/// count-cap discipline, sized higher (200 vs. 20) because this report is
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
    pub job_keywords: HashSet<String>,
    pub generated_keywords: HashSet<String>,
    pub source_keywords: HashSet<String>,
    /// Stem → readable, unstemmed form for every token in this report's three
    /// documents. Empty when nothing was stemmed. See [`Analysis::display`].
    display: HashMap<String, String>,
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
        // Stems are an implementation detail of the comparison and must never
        // reach a message — see [`Analysis::display`]. Built over all three
        // documents because a finding can name a token from any of them.
        let display = if aligned {
            let corpus = format!(
                "{}\n{}\n{}",
                input.generated, input.source_resume, input.job_ad
            );
            display_forms(&corpus, &stemmer)
        } else {
            HashMap::new()
        };
        Self {
            lang: lang.clone(),
            generated_sections: split_sections(input.generated),
            source_sections: split_sections(input.source_resume),
            job_keywords: tokens(input.job_ad),
            generated_keywords: tokens(input.generated),
            source_keywords: tokens(input.source_resume),
            language_mismatch: language_mismatch_for(input, &lang),
            display,
            aligned,
            stemmer,
            input,
        }
    }

    /// Tokenize `text` the same way both sides of this report were tokenized.
    pub fn tokens(&self, text: &str) -> HashSet<String> {
        if self.aligned {
            keywords(text, &self.stemmer)
        } else {
            keywords_normalized(text)
        }
    }

    /// The readable form of a token, for interpolation into a user-facing
    /// `message` or `evidence`.
    ///
    /// [`Analysis::tokens`] stems when the languages align, so a token taken
    /// straight from a comparison reads as `kubernet`, `develop`, `entwickl`.
    /// Telling a user their résumé never demonstrates "kubernet" is a finding
    /// they cannot act on and does not trust. Every token that reaches a
    /// message goes through here first.
    pub fn display(&self, token: &str) -> String {
        self.display
            .get(token)
            .cloned()
            .unwrap_or_else(|| token.to_string())
    }

    /// Coverage of the posting by `tokens`, 0–100. `None` when the posting has
    /// no extractable keywords (a sparse or garbled ad) — the caller must go
    /// quiet rather than report 0%.
    pub fn coverage(&self, tokens: &HashSet<String>) -> Option<f64> {
        keyword_coverage(&self.job_keywords, tokens).map(|(c, _)| c)
    }

    /// Whether every posting comparison should be skipped: nothing extractable
    /// on the posting side, or the output is not in the target language.
    pub fn posting_comparable(&self) -> bool {
        !self.job_keywords.is_empty() && !self.language_mismatch
    }

    pub fn section_of_kind(&self, kind: SectionKind) -> Option<&Section> {
        self.generated_sections.iter().find(|s| s.kind == kind)
    }

    pub fn source_section_of_kind(&self, kind: SectionKind) -> Option<&Section> {
        self.source_sections.iter().find(|s| s.kind == kind)
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

/// Whether `text` is written in something other than `lang`.
///
/// Routed through `languages_align` rather than a bespoke comparison so the
/// language question has ONE answer in this codebase: it detects `text`'s
/// language with the same `whatlang` path `keywords.rs` uses and asks whether
/// that language pairs with the given locale tag. Goes quiet on short text,
/// where detection is a guess.
pub(crate) fn is_language_mismatch(text: &str, lang: &str) -> bool {
    let significant = text.chars().filter(|c| !c.is_whitespace()).count();
    significant >= MIN_CHARS_FOR_LANGUAGE_CHECK && !languages_align(text, lang)
}

/// Whether this report may raise `content.language_mismatch`.
///
/// The generated text reading as the wrong language is necessary but NOT
/// sufficient. The candidate's own source résumé is the control: it was written
/// by a human in the language they meant, so when the detector reads BOTH
/// documents as the same "wrong" language, the detector is what is wrong —
/// `whatlang` routinely calls a terse, tech-heavy English résumé Dutch,
/// Norwegian or Tagalog. Firing there produces a Critical the user cannot act on
/// ("re-generate it in English" — it *is* in English) and, worse, blanks
/// `keywordCoverage` and suppresses every alignment finding along with it.
///
/// A genuinely mis-generated document does not look like its source, so the
/// real defect (an English source, a German output) still fires.
fn language_mismatch_for(input: &ContentInput, lang: &str) -> bool {
    if !is_language_mismatch(input.generated, lang) {
        return false;
    }
    let source_reads_the_same_way = is_language_mismatch(input.source_resume, lang)
        && detect_locale_tag(input.source_resume) == detect_locale_tag(input.generated);
    !source_reads_the_same_way
}

/// Split a document into sections at its headings. The leading band before the
/// first heading (name + contact) is always section 0 with `heading: None`, so
/// "is this in a non-first section?" is just an index test.
pub(crate) fn split_sections(text: &str) -> Vec<Section> {
    let mut sections = vec![Section {
        heading: None,
        kind: SectionKind::Other,
        lines: Vec::new(),
    }];
    for line in parse_resume(text).lines {
        if matches!(line.kind, LineKind::SectionHeader) {
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

/// A line that really does carry contact details: a genuine email address, or a
/// phone shape on a line with no `@` at all.
///
/// `export::parser::is_first_line_contact_shaped` accepts a bare `@`, which any
/// body line mentioning a Slack handle or a `@decorator` satisfies — so a bullet
/// could hide its numbers behind "this is the header" just by carrying one.
/// `factual::metrics_in`'s skip routes through this; `ats::is_contact_cluster`
/// applies the same email-vs-phone rule inline, because it additionally has to
/// reject a date range that `PHONE_RE` reads as a phone number.
pub(crate) fn has_real_contact_match(text: &str) -> bool {
    if text.contains('@') {
        return EMAIL_RE.is_match(text);
    }
    is_first_line_contact_shaped(text)
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

    let (top_requirement_hits, duplicate_ratio) = match input.doc_kind {
        DocKind::CoverLetter => {
            issues.extend(letter::validate(&ctx));
            (0, 0.0)
        }
        DocKind::Resume => {
            issues.extend(factual::validate(&ctx));
            let (alignment_issues, hits) = alignment::validate(&ctx);
            issues.extend(alignment_issues);
            issues.extend(consistency::validate(&ctx));
            let (duplicate_issues, ratio) = duplicates::validate(&ctx);
            issues.extend(duplicate_issues);
            issues.extend(ats::validate(&ctx));
            issues.extend(voice::validate(&ctx));
            (hits, ratio)
        }
    };

    let metrics = ContentMetrics {
        keyword_coverage: ctx
            .posting_comparable()
            .then(|| ctx.coverage(&ctx.generated_keywords))
            .flatten(),
        top_requirement_hits,
        duplicate_ratio,
        roles_source: factual::count_roles(&ctx.source_sections) as u32,
        roles_output: factual::count_roles(&ctx.generated_sections) as u32,
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

    // M-3 fix: cap the issue list at MAX_CONTENT_ISSUES (see its doc) so the
    // serialized report can't blow the save path's byte clamp. Criticals sort
    // first so a pathological warning flood can never push a real Critical
    // out of the visible list — only warnings are ever silently cut, and the
    // trailing REPORT_TRUNCATED marker says so instead of a silent drop.
    if issues.len() > MAX_CONTENT_ISSUES {
        let dropped = issues.len() - MAX_CONTENT_ISSUES;
        issues.sort_by_key(|i| i.severity != Severity::Critical);
        issues.truncate(MAX_CONTENT_ISSUES);
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

    ContentReport {
        ok: criticals == 0,
        issues,
        metrics,
    }
}

/// `content.language_mismatch` — the output is not in the language it was asked
/// for. Critical: a German résumé sent to an English-speaking employer is not a
/// quality nit, and every downstream comparison is meaningless once it holds.
fn language_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    if !ctx.language_mismatch {
        return Vec::new();
    }
    vec![issue(
        CONTENT_LANGUAGE_MISMATCH,
        None,
        format!(
            "This document does not read as {}, the language it was generated for. \
             Re-generate it with the target language set correctly before sending.",
            ctx.lang
        ),
        Some(ctx.lang.clone()),
    )]
}
