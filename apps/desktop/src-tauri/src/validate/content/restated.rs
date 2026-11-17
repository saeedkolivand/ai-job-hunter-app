//! `voice.restated_point` — a letter that makes the same point twice in other words.
//!
//! Mirrors the prompt's "make each point once" rule. Word repetition is NOT the
//! target (the prompt tells the model to repeat the clear word rather than cycle
//! synonyms): sentences that merely share key terms stay quiet, only sentences
//! whose remaining content words largely coincide fire.
//!
//! Content words come from [`Analysis::tokens`], the stopword-filtered tokenizer
//! the posting comparisons use, so there is no second tokenizer to drift. Before
//! pairing, two kinds of term are removed from every sentence because sharing
//! them proves nothing: the posting's own vocabulary (role title, company, stack:
//! an opening and a closing that both name "Senior Rust Engineer at Acme" are
//! not a restatement) and any term the letter uses in three or more sentences
//! (its subject, e.g. "Kubernetes"). Advisory like the rest of the family.

use std::collections::{HashMap, HashSet};

use super::text::jaccard;
use super::voice::MIN_SENTENCES_FOR_BURSTINESS;
use super::{issue, sentences, Analysis, ContentIssue, VOICE_RESTATED_POINT};
use crate::documents::evidence::has_curated_function_words;

/// Content words each sentence needs, AFTER the removals above, before it can be
/// a restatement. Greetings, sign-offs and fragments fall below it.
pub const MIN_CONTENT_WORDS_FOR_RESTATEMENT: usize = 5;

/// Jaccard overlap of two sentences' remaining content words at or above which
/// they read as one point said twice. Jaccard, not overlap-coefficient: the
/// latter scores a short sentence that reuses a longer one's terms as a perfect
/// match. 0.6 means roughly two thirds of the combined vocabulary is shared.
pub const MIN_RESTATEMENT_JACCARD: f64 = 0.6;

/// A term used in this many sentences is the letter's subject, not a point.
pub const LETTER_WIDE_TERM_SENTENCES: usize = 3;

pub(super) fn restated_point_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    // Only where function-word data is curated: the other stopword lists are
    // job-ad filler and leave ordinary words in, which inflates overlap.
    if !has_curated_function_words(&ctx.lang) || ctx.language_mismatch {
        return Vec::new();
    }
    let all = sentences(ctx.input.generated);
    if all.len() < MIN_SENTENCES_FOR_BURSTINESS {
        return Vec::new();
    }
    let raw: Vec<HashSet<String>> = all.iter().map(|s| ctx.tokens(s)).collect();
    let mut spread: HashMap<&str, usize> = HashMap::new();
    for token in raw.iter().flatten() {
        *spread.entry(token.as_str()).or_default() += 1;
    }
    let sets: Vec<HashSet<String>> = raw
        .iter()
        .map(|set| {
            set.iter()
                .filter(|t| !ctx.job_keywords.contains(*t))
                .filter(|t| spread[t.as_str()] < LETTER_WIDE_TERM_SENTENCES)
                .cloned()
                .collect()
        })
        .collect();
    let mut pairs = 0usize;
    let mut best: Option<(f64, usize, usize)> = None;
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            let (a, b) = (&sets[i], &sets[j]);
            if a.len() < MIN_CONTENT_WORDS_FOR_RESTATEMENT
                || b.len() < MIN_CONTENT_WORDS_FOR_RESTATEMENT
            {
                continue;
            }
            let score = jaccard(a, b);
            if score >= MIN_RESTATEMENT_JACCARD {
                pairs += 1;
                if best.is_none_or(|(top, ..)| score > top) {
                    best = Some((score, i, j));
                }
            }
        }
    }
    let Some((_, i, j)) = best else {
        return Vec::new();
    };
    vec![issue(
        VOICE_RESTATED_POINT,
        None,
        "Two sentences make the same point in different words. Say each point once and cut \
         the restatement."
            .to_string(),
        Some(format!("\"{}\" / \"{}\" ({pairs} pair(s))", all[i], all[j])),
    )]
}
