//! `voice.restated_point` — a letter that makes the same point twice in other words.
//!
//! Mirrors the prompt's "make each point once" rule. Word repetition is NOT the
//! target (the prompt tells the model to repeat the clear word rather than cycle
//! synonyms): sentences that merely share key terms stay quiet, only sentences
//! whose remaining content words largely coincide fire.
//!
//! Content words come from [`Analysis::tokens`], the stopword-filtered tokenizer
//! the posting comparisons use, so there is no second tokenizer to drift. Those
//! tokens are keyed to the POSTING's language: an English ad with a German letter
//! compares unstemmed, which can only make sets less alike (quieter, never
//! louder). The posting's own vocabulary (role title, company, stack) is removed
//! from every sentence first: an opening and a closing that both name "Senior
//! Rust Engineer at Acme" are not a restatement.
//!
//! *Recall limit, stated:* this is lexical overlap, so a pure-synonym paraphrase
//! ("mentored" / "coached", with every other word changed) is not caught. That is
//! the accepted trade for the family: a wrong warning costs more than a missed one.
//! Advisory like the rest of it.

use std::collections::HashSet;

use super::text::jaccard;
use super::{issue, sentences, Analysis, ContentIssue, VOICE_RESTATED_POINT};
use crate::applications::clamp_to_bytes;
use crate::documents::evidence::has_curated_function_words;

/// Sentences a letter needs before a repeat is a signal; mirrors the burstiness
/// gate's reasoning (a short letter has little room to pad).
pub const MIN_SENTENCES_FOR_RESTATEMENT: usize = 8;

/// Above this many sentences the input is not a letter; the pairwise scan is
/// O(n²), so it goes quiet instead, as `duplicates::MAX_DUP_BULLETS` bounds the
/// duplicate scan.
pub const MAX_SENTENCES_FOR_RESTATEMENT: usize = 80;

/// Content words each sentence needs, AFTER the posting vocabulary is removed,
/// before it can be a restatement. Greetings, sign-offs and fragments fall below it.
pub const MIN_CONTENT_WORDS_FOR_RESTATEMENT: usize = 5;

/// Jaccard overlap of two sentences' remaining content words at or above which
/// they read as one point said twice. Jaccard, not overlap-coefficient: the
/// latter scores a short sentence that reuses a longer one's terms as a perfect
/// match. 0.7 (about 70% of the combined vocabulary shared) keeps two parallel
/// sentences that state DIFFERENT facts in one frame ("the first quarter ... payment
/// services" / "the second quarter ... reporting services", about 0.6) quiet.
pub const MIN_RESTATEMENT_JACCARD: f64 = 0.7;

/// Per-sentence budget in the evidence, so both sentences and the pair count fit
/// the issue-evidence clamp.
const EVIDENCE_SENTENCE_BYTES: usize = 170;

pub(super) fn restated_point_issues(ctx: &Analysis) -> Vec<ContentIssue> {
    // The same language gate `ats`/`consistency` use for function-word checks.
    if !has_curated_function_words(&ctx.lang) || ctx.language_mismatch {
        return Vec::new();
    }
    let all = sentences(ctx.input.generated);
    if !(MIN_SENTENCES_FOR_RESTATEMENT..=MAX_SENTENCES_FOR_RESTATEMENT).contains(&all.len()) {
        return Vec::new();
    }
    let sets: Vec<HashSet<String>> = all
        .iter()
        .map(|s| {
            ctx.tokens(s)
                .into_iter()
                .filter(|t| !ctx.job_keywords.contains(t))
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
    let quote = |s: &str| clamp_to_bytes(s.to_string(), EVIDENCE_SENTENCE_BYTES);
    vec![issue(
        VOICE_RESTATED_POINT,
        None,
        "Two sentences make the same point in different words. Merge them into one sentence \
         that says the point once."
            .to_string(),
        Some(format!(
            "{} / {} ({pairs} pair(s))",
            quote(&all[i]),
            quote(&all[j])
        )),
    )]
}
