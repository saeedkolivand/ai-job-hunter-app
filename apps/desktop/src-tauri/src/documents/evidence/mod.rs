//! Evidence extraction — the candidate's OWN résumé, structured and scored
//! against one posting.
//!
//! Two entry points over one scorer:
//!
//! * [`rank_bullets`] — the bullet scorer moved verbatim out of
//!   `commands::match_resume::rank_trim_candidates`. Ranks a résumé's bullets
//!   weakest-first for a posting; the trim panel still consumes it through a
//!   `TrimCandidate: From<EvidenceBullet>` shim so its IPC payload is unchanged.
//! * [`extract_evidence`] — the same scoring applied section-by-section, so a
//!   generation prompt (or a validator) can talk about *which role* an
//!   achievement came from instead of a flat list of lines.
//!
//! The point of the module is the honesty spine: the model decides HOW to
//! present the candidate's verified evidence, never WHAT the candidate has
//! done. Everything here is derived from the source résumé — nothing is
//! invented, nothing comes from a model.
//!
//! Deliberately embedding-free: it reuses the `documents::keywords` kernel and
//! the same JD-derived stemmer as `score_one`, so no surface here can disagree
//! with the match score the user sees for the same pair. Zero model calls,
//! works offline.
//!
//! One scorer, split by responsibility (R8): `vocabulary` (the posting's resolved
//! vocabulary and per-line scoring), `sections` (heading classification),
//! `extract` (the section-by-section walk), `function_words` (the display-only
//! skills filter), and `entry` / `dates` (what an entry line SAYS: employer, title,
//! date column).

use serde::Serialize;

use crate::export::parser::parse_resume;
use crate::export::types::LineKind;

mod dates;
mod entry;
mod extract;
mod function_words;
mod sections;
mod vocabulary;

#[cfg(test)]
mod tests;

use self::vocabulary::JobVocabulary;

/// The entry/label/date shape family, re-exported so every existing
/// `documents::evidence::…` path keeps resolving after the split.
pub use self::dates::{
    contains_word, date_spans, is_open_ended, looks_like_date_span, trailing_date_column, years_in,
    PRESENT_MARKERS,
};
pub use self::entry::{identity_tokens, split_entry, GEOGRAPHY_TOKENS, LEGAL_FORMS};
pub use self::extract::extract_evidence;
pub(crate) use self::function_words::{function_words, has_curated_function_words};
pub use self::sections::{classify_section, SectionKind};

/// One résumé bullet, scored by how much of THIS posting's vocabulary it carries.
///
/// `score` is a hit COUNT stored as `f64` (always a non-negative whole number)
/// so a future weighted scorer can refine it without a wire-type change.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceBullet {
    /// Stable, document-order id (`b3`, `r1b0`, `p2`) — lets a prompt or a
    /// validator reference one specific line without quoting it back.
    pub id: String,
    /// The bullet's markdown-stripped text, as the reader sees it.
    pub text: String,
    /// Readable (unstemmed) job keywords this line carries — may be empty.
    pub hits: Vec<String>,
    pub score: f64,
}

/// One employment entry with the bullets that belong to it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRole {
    pub company: String,
    pub title: String,
    /// The entry's date span exactly as the source wrote it (`2021 – Present`).
    pub dates: String,
    pub bullets: Vec<EvidenceBullet>,
}

/// Everything the source résumé can actually vouch for, for one posting.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSet {
    pub roles: Vec<EvidenceRole>,
    /// Posting keywords the source résumé DOES evidence, readable + sorted.
    pub skills_present: Vec<String>,
    /// Posting keywords it does not — the honest gap list, readable + sorted.
    pub skills_absent: Vec<String>,
    pub education: Vec<String>,
    pub projects: Vec<EvidenceBullet>,
}

/// Weakest first. Among lines carrying equally little, the LONGEST goes first:
/// same loss to the reader, most space recovered.
fn sort_weakest_first(bullets: &mut [EvidenceBullet]) {
    bullets.sort_by(|a, b| {
        a.score
            .total_cmp(&b.score)
            .then_with(|| b.text.len().cmp(&a.text.len()))
    });
}

/// Rank a résumé's bullets weakest-first for a given posting.
///
/// Only `LineKind::Bullet` lines are candidates — section headers, job entries
/// and the contact block are structural, and cutting them to save space is never
/// the right advice.
///
/// Returns empty when the posting has no extractable keywords, mirroring
/// `keyword_coverage`'s `None`: with nothing to score against, every line would
/// tie at zero and the ranking would be noise dressed up as a recommendation.
///
/// Ids are `b<n>` in the résumé's own bullet order, assigned BEFORE the ranking
/// sort, so `b0` always means "the first bullet in the document".
pub fn rank_bullets(text: &str, job_text: &str) -> Vec<EvidenceBullet> {
    let vocab = JobVocabulary::new(text, job_text);
    if vocab.keywords.is_empty() {
        return Vec::new();
    }
    let mut bullets: Vec<EvidenceBullet> = parse_resume(text)
        .lines
        .iter()
        .filter(|line| matches!(line.kind, LineKind::Bullet))
        .enumerate()
        .map(|(i, line)| vocab.bullet(format!("b{i}"), &line.text))
        .collect();
    sort_weakest_first(&mut bullets);
    bullets
}
