//! The posting's vocabulary, resolved once with the language discipline every
//! résumé↔posting surface in this codebase shares.
//!
//! Split out of `evidence/mod.rs` (R8's hard LOC cap); everything here moved
//! verbatim.

use std::collections::{HashMap, HashSet};

use rust_stemmers::Stemmer;

use crate::documents::keywords::{
    apply_stemmer, detect_locale_tag, display_forms_for_lang, keywords_for_lang,
    keywords_normalized_for_lang, keywords_normalized_list_for_lang, languages_align, make_stemmer,
};

use super::EvidenceBullet;

/// The posting's vocabulary, resolved once with the language discipline every
/// résumé↔posting surface in this codebase shares.
///
/// Symmetric normalization, exactly as `score_one` does it: stem BOTH sides
/// with the JD-derived stemmer when the languages align, leave BOTH
/// normalized-only when they diverge. Stemming one side alone mutates
/// language-neutral tech tokens on that side only and matches neither set.
///
/// The résumé's language is DETECTED rather than read from a stored locale —
/// this path scores generator output that was never persisted, so there is no
/// `DocumentRecord::locale` to consult.
pub(super) struct JobVocabulary {
    aligned: bool,
    stemmer: Stemmer,
    pub(super) keywords: HashSet<String>,
    /// Stem → readable, unstemmed display form for every posting keyword.
    display: HashMap<String, String>,
    /// Keyword → how many times the POSTING states it. See
    /// [`posting_weights`].
    weights: HashMap<String, usize>,
    /// The posting's own detected language tag — picks the [`function_words`]
    /// list the present/absent split is filtered with.
    pub(super) lang: &'static str,
}

/// How often the posting states each of its own keywords, keyed exactly like
/// [`JobVocabulary::keywords`] — the relevance signal the skills split is
/// ordered by.
///
/// `keywords` is a `HashSet`, so term frequency is discarded by the time the
/// split runs; this recovers it from the posting text. Term frequency rather
/// than first-occurrence position because a requirements list repeats what the
/// role is actually about, while position mostly reflects where the boilerplate
/// ends — and it costs one extra walk of a text this function already tokenizes.
///
/// Both halves come from the kernel rather than being transcribed:
/// `keywords_normalized_list` is its own duplicate-preserving form (same
/// tokenizer, synonym collapse and filter as the set, so a count can never
/// disagree with membership), and the fold onto stems goes through
/// `apply_stemmer`, so the `SHORT_TECH_TERMS` bypass that keeps "aws" from
/// stemming to "aw" is applied once, where it is defined. Stemming runs per
/// DISTINCT token, the same order of work `display_forms` already does.
fn posting_weights(
    job_text: &str,
    lang: &str,
    stemmer: &Stemmer,
    aligned: bool,
) -> HashMap<String, usize> {
    let mut normalized: HashMap<String, usize> = HashMap::new();
    for token in keywords_normalized_list_for_lang(job_text, lang) {
        *normalized.entry(token).or_default() += 1;
    }
    if !aligned {
        return normalized; // The unaligned vocabulary is unstemmed too.
    }
    let mut out: HashMap<String, usize> = HashMap::new();
    for (token, n) in normalized {
        let stem = apply_stemmer(HashSet::from([token]), stemmer)
            .into_iter()
            .next()
            .unwrap_or_default();
        *out.entry(stem).or_default() += n;
    }
    out
}

impl JobVocabulary {
    pub(super) fn new(resume_text: &str, job_text: &str) -> Self {
        let aligned = languages_align(job_text, detect_locale_tag(resume_text));
        let stemmer = make_stemmer(job_text);
        let lang = detect_locale_tag(job_text);
        let keywords = if aligned {
            keywords_for_lang(job_text, lang, &stemmer)
        } else {
            keywords_normalized_for_lang(job_text, lang)
        };
        // Display forms are keyed on whatever the JD side produced, so they must
        // be built the same way — a stemmed map would miss every unstemmed hit.
        let display = if aligned {
            display_forms_for_lang(job_text, lang, &stemmer)
        } else {
            keywords_normalized_for_lang(job_text, lang)
                .into_iter()
                .map(|t| (t.clone(), t))
                .collect()
        };
        let weights = posting_weights(job_text, lang, &stemmer, aligned);
        Self {
            aligned,
            stemmer,
            keywords,
            display,
            weights,
            lang,
        }
    }

    /// How often the posting states `token`; `0` for anything it never said.
    pub(super) fn weight(&self, token: &str) -> usize {
        self.weights.get(token).copied().unwrap_or(0)
    }

    /// The readable form of a keyword — the unstemmed token the posting used,
    /// falling back to the key itself.
    pub(super) fn readable(&self, token: &str) -> String {
        self.display
            .get(token)
            .cloned()
            .unwrap_or_else(|| token.to_string())
    }

    /// This side's tokens, normalized the same way the posting's were.
    ///
    /// Pinned to `self.lang` rather than re-detected from `text` — `text` here
    /// is a single résumé bullet, and `whatlang` reading a short line in
    /// isolation is unreliable (it can name a different language than the
    /// whole posting did). See `documents::keywords::keywords_normalized_list_for_lang`.
    pub(super) fn tokens(&self, text: &str) -> HashSet<String> {
        if self.aligned {
            keywords_for_lang(text, self.lang, &self.stemmer)
        } else {
            keywords_normalized_for_lang(text, self.lang)
        }
    }

    /// Score one line: the readable posting keywords it carries.
    pub(super) fn bullet(&self, id: String, text: &str) -> EvidenceBullet {
        let mut hits: Vec<String> = self
            .tokens(text)
            .intersection(&self.keywords)
            .map(|stem| self.readable(stem))
            .collect();
        hits.sort();
        EvidenceBullet {
            id,
            score: hits.len() as f64,
            hits,
            text: text.to_string(),
        }
    }
}
