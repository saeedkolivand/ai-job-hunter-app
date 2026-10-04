//! Keyword extraction for ATS matching.
//!
//! The pipeline is split so cached tokens stay language-agnostic:
//! keywords_normalized does lowercase + synonym-collapse + filter (NO
//! stemming) and is what we persist per document; apply_stemmer stems a
//! normalized set with a stemmer whose language is detected at match time.
//! This lets the same cached resume tokens match a JD in any language.
//!
//! One kernel, split by responsibility (R8): `lexicon` / `stopwords_*` (the curated tables),
//! `language` (stemmer + stopword selection and language identity), `posting` (a job posting
//! into scoring text). This file holds the tokenizer, the stemming helpers and the coverage math.

use std::collections::{HashMap, HashSet};

use rust_stemmers::Stemmer;

mod language;
mod lexicon;
mod posting;
mod stopwords_germanic;
mod stopwords_romance;
#[cfg(test)]
mod tests;

pub use language::{
    detect_locale_tag, detected_language, languages_align, make_stemmer, MIN_DETECTION_CONFIDENCE,
};
pub use lexicon::{SHORT_TECH_TERMS, STOPWORDS, SYNONYMS};
pub use posting::{description_is_blank, markdown_to_plain, posting_text_blob};

use language::{language_profile, stopwords_for_lang};

/// Normalize text to a language-agnostic keyword set: lowercase,
/// synonym-normalized, filtered - but NOT stemmed. Tokens shorter than 4 chars
/// are dropped unless they are in SHORT_TECH_TERMS; stopwords are excluded.
/// The slash is kept in tokenization so ci/cd survives as a single token.
///
/// Store this in the DB - apply apply_stemmer at match time to stay
/// language-agnostic (the stemmer language is detected from the JD, not the
/// resume, so caching a pre-stemmed set would bake in the wrong language).
///
/// A thin `collect()` over [`keywords_normalized_list`] so the set form and the
/// occurrence-counting list form can never drift apart.
pub fn keywords_normalized(text: &str) -> HashSet<String> {
    keywords_normalized_list(text).into_iter().collect()
}

/// Duplicate-preserving, document-ordered form of [`keywords_normalized`] — the
/// SAME tokenizer, synonym collapse and filter, returning every surviving token
/// instead of deduplicating them.
///
/// Exists for the consumers that must count *repeats* rather than membership
/// (the ATS keyword-density check in `validate::content::ats`). Deliberately the
/// single implementation of the pipeline, with `keywords_normalized` delegating
/// to it: a second tokenizer written "just to count" is exactly the fork the
/// keyword kernel exists to prevent.
pub fn keywords_normalized_list(text: &str) -> Vec<String> {
    // Same text, same detection call site `make_stemmer` uses (via
    // `language_profile`) — the stopword language can never disagree with the
    // stemmer language for this call. Correct for a caller that tokenizes ONE
    // self-contained document (a whole résumé or JD); see
    // [`keywords_normalized_list_for_lang`] for the short-fragment case where
    // this per-call detection is NOT safe.
    normalize_list_with_stopwords(text, language_profile(text).1)
}

/// The [`keywords_normalized_list`] pipeline, but the stopword LANGUAGE is
/// pinned to an explicit ISO-639-1 tag rather than re-detected from `text`.
///
/// For a caller that already resolved ONE language decision for a whole
/// document (e.g. `validate::content::Analysis::lang`, or
/// `DocumentTokens`/`Analysis`'s stemmer) and then tokenizes many SHORT
/// per-line, per-title or per-bullet fragments of it: `whatlang` reading an
/// isolated short line in isolation is unreliable ("Kenntnisse in Rust,
/// Python, Kubernetes, Terraform und Kafka" reads as Estonian at confidence
/// 0.23, not German), and a per-call re-detection can silently pick a
/// DIFFERENT stopword list than the one the whole document resolved to. A
/// filler word filtered out of the document-level set (and so absent from its
/// stem→readable [`display_forms`] map) would then survive un-filtered from
/// the line-level call and leak out as a raw, unreadable stem instead of
/// being suppressed. Mirrors [`keywords_normalized_list`] exactly — same
/// shared tokenizer, only the stopword SOURCE differs — so the two can never
/// diverge on tokenization itself, only on which language's filler they drop.
pub fn keywords_normalized_list_for_lang(text: &str, lang: &str) -> Vec<String> {
    normalize_list_with_stopwords(text, stopwords_for_lang(lang))
}

/// 3-letter English month abbreviations that scraped chart/axis labels glue to a short digit run
/// ("1sep", "2025mar") — see [`is_chart_date_label`] (issue #1223).
const CHART_MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// True when the token contains a `/`. The tokenizer's slash-tolerant split keeps URL/selector
/// crumbs like "/company/harvey" as ONE token, which is scraped-chrome noise, not a skill. Safe
/// to reject here: the slashed [`SYNONYMS`] entries ("ci/cd" → "cicd", "c/c++" → "cpp") are
/// canonicalized one pipeline step above this filter, so no real keyword is lost (issue #1223).
fn has_path_separator(s: &str) -> bool {
    s.contains('/')
}

/// Split a token into leading digit run, core, and trailing digit run ("2025mar" →
/// ("2025", "mar", ""); "es2015" → ("", "es", "2015")). An all-digit token (prefix_len ≥ end)
/// yields `(s, "", "")` — the pure-numeric filter above already drops those, but this never
/// index-panics on them regardless.
fn split_digit_runs(s: &str) -> (&str, &str, &str) {
    let prefix_len = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let suffix_len = s.len() - s.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let end = s.len() - suffix_len;
    if prefix_len >= end {
        return (s, "", "");
    }
    (&s[..prefix_len], &s[prefix_len..end], &s[end..])
}

/// True for tokens shaped like scraped chart/axis date labels: 1-4 ASCII digits glued to one of
/// [`CHART_MONTHS`], in either order ("1sep", "2025mar", "mar2026") — the rotate-graph/chart
/// chrome some job-ads' embedded salary/hiring charts put in the DOM, which the tokenizer keeps
/// as single alphanumeric tokens and which would otherwise surface as made-up skills on the
/// missing-keyword chips ("1sep" reads as a skill). Tokens with no digit run, a digit run longer
/// than 4 (a real version token like "es2015" has exactly 4 — but its core "es" is not a month),
/// or a non-month core are untouched (oauth2, es2015, react17 all survive).
fn is_chart_date_label(s: &str) -> bool {
    let (prefix, core, suffix) = split_digit_runs(s);
    let digit_chars = prefix.len() + suffix.len();
    digit_chars > 0 && digit_chars <= 4 && CHART_MONTHS.contains(&core)
}

fn normalize_list_with_stopwords(text: &str, stopwords: &[&str]) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '+' && c != '#' && c != '/')
        .map(|w| w.to_lowercase())
        .filter(|w| !w.is_empty())
        // Synonym lookup runs on the raw lowercased token (before trim) so
        // entries like c-plus-plus map to cpp and still match - trim
        // would otherwise strip the trailing plus and make them dead code.
        .map(|w| {
            SYNONYMS
                .iter()
                .find(|(alias, _)| *alias == w.as_str())
                .map(|(_, canon)| canon.to_string())
                .unwrap_or(w)
        })
        .map(|w| w.trim_matches(|c: char| c == '+' || c == '#').to_string())
        .filter(|w| {
            let s = w.as_str();
            !w.is_empty()
                && (w.len() > 3 || SHORT_TECH_TERMS.contains(&s))
                && !stopwords.contains(&s)
                // Pure-numeric tokens (postcodes, bare years) carry no keyword
                // signal on either side. Mixed alphanumeric tech tokens (c4, s3,
                // oauth2, es2015) are untouched - at least one char isn't a digit.
                && !s.chars().all(|c| c.is_ascii_digit())
                // Issue #1223: missing-keyword chips must come from
                // job-description CONTENT only, so scraped-chrome noise is a
                // defensive floor here. Slash-shaped tokens (`/company/harvey`
                // survives the tokenizer's slash-tolerant split as one token)
                // and chart/axis date labels (`1sep`, `2025mar`) are DOM chrome,
                // not skills. Safe to reject slash-tokens here because the
                // slashed synonyms (`ci/cd` → `cicd`, `c/c++` → `cpp`) were
                // canonicalized one step above, so no real keyword is lost.
                && !has_path_separator(s)
                && !is_chart_date_label(s)
        })
        .collect()
}

/// [`keywords_normalized`], with the stopword language pinned explicitly —
/// see [`keywords_normalized_list_for_lang`].
pub fn keywords_normalized_for_lang(text: &str, lang: &str) -> HashSet<String> {
    keywords_normalized_list_for_lang(text, lang)
        .into_iter()
        .collect()
}

/// Stem a pre-normalized keyword set using the given stemmer.
/// SHORT_TECH_TERMS bypass stemming so e.g. the English Snowball plural rule
/// does not corrupt acronyms (aws becomes aw).
pub fn apply_stemmer(tokens: HashSet<String>, stemmer: &Stemmer) -> HashSet<String> {
    tokens
        .into_iter()
        .map(|w| {
            if SHORT_TECH_TERMS.contains(&w.as_str()) {
                w
            } else {
                stemmer.stem(&w).into_owned()
            }
        })
        .collect()
}

/// Convenience: normalize + stem in one call (used for JD keywords at match
/// time, and as the cache-miss fallback for resumes).
pub fn keywords(text: &str, stemmer: &Stemmer) -> HashSet<String> {
    apply_stemmer(keywords_normalized(text), stemmer)
}

/// [`keywords`], with the stopword language pinned explicitly — see
/// [`keywords_normalized_list_for_lang`]. `stemmer` is still whatever the
/// caller already built (typically also from the SAME resolved language, via
/// [`make_stemmer`] on the whole document); this only changes which stopword
/// list filters `text` before stemming.
pub fn keywords_for_lang(text: &str, lang: &str, stemmer: &Stemmer) -> HashSet<String> {
    apply_stemmer(keywords_normalized_for_lang(text, lang), stemmer)
}

/// Map each stemmed JD keyword to a human-readable display form, so the gaps
/// surfaced to the user read as real words ("kubernetes", "developer") instead
/// of Snowball stems ("kubernet", "develop").
///
/// The display form is the *unstemmed, normalized* token (lowercase, synonyms
/// collapsed) that stems to that key — synonym collapse means e.g. a `k8s` gap
/// surfaces as `kubernetes`. Best-effort: original casing from the raw JD is not
/// preserved (normalization lowercases), and if two distinct tokens stem to the
/// same key the first one encountered wins. The map keys are exactly the members
/// of `keywords(job_text, stemmer)`, so every gap has an entry.
pub fn display_forms(job_text: &str, stemmer: &Stemmer) -> HashMap<String, String> {
    display_forms_from(keywords_normalized(job_text), stemmer)
}

/// [`display_forms`], with the stopword language pinned explicitly — see
/// [`keywords_normalized_list_for_lang`]. Needed alongside
/// [`keywords_for_lang`]/[`keywords_normalized_for_lang`]: a caller that
/// tokenizes under an explicit lang MUST build its display map the same way,
/// or a word the explicit-lang tokenizer keeps (because the auto-detected
/// language would have dropped it, or vice versa) gets a display entry that
/// doesn't match what was actually tokenized.
pub fn display_forms_for_lang(
    job_text: &str,
    lang: &str,
    stemmer: &Stemmer,
) -> HashMap<String, String> {
    display_forms_from(keywords_normalized_for_lang(job_text, lang), stemmer)
}

fn display_forms_from(normalized: HashSet<String>, stemmer: &Stemmer) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    // Iterate a sorted Vec, not the HashSet, so the `or_insert` winner for two
    // tokens sharing a stem is deterministic across runs.
    let mut tokens: Vec<_> = normalized.into_iter().collect();
    tokens.sort();
    for token in tokens {
        let stem = if SHORT_TECH_TERMS.contains(&token.as_str()) {
            token.clone()
        } else {
            stemmer.stem(&token).into_owned()
        };
        map.entry(stem).or_insert(token);
    }
    map
}

/// Replace each stemmed gap with its readable [`display_forms`] entry, falling
/// back to the stem itself if no mapping exists (should not happen, since the
/// map is keyed on the same JD keyword set). Order is preserved.
pub fn readable_gaps(gaps: &[String], display: &HashMap<String, String>) -> Vec<String> {
    gaps.iter()
        .map(|g| display.get(g).cloned().unwrap_or_else(|| g.clone()))
        .collect()
}

/// Keyword-coverage of a job's keyword set by a résumé's keyword set: the share
/// of job keywords (0–100, rounded) that also appear in the résumé, plus the
/// up-to-15 sorted missing keywords (`gaps`). Single source of the coverage
/// formula shared by the Jobs-page ATS sub-score ([`coverage_score`] /
/// `commands::match_resume::score_one`) and the headless Autopilot ranker.
/// Both sides are expected to be stemmed with the SAME (JD-derived) stemmer.
///
/// Returns `None` when the job keyword set is empty (sparse/unparseable posting)
/// so callers can distinguish "no extractable keywords" from "0% match".
pub fn keyword_coverage(
    job: &HashSet<String>,
    resume: &HashSet<String>,
) -> Option<(f64, Vec<String>)> {
    if job.is_empty() {
        return None;
    }
    let mut gaps: Vec<String> = job.difference(resume).cloned().collect();
    gaps.sort();
    let matched = job.len() - gaps.len();
    let coverage = (matched as f64 / job.len() as f64 * 100.0).round();
    gaps.truncate(15);
    Some((coverage, gaps))
}

/// Embedding-free keyword-coverage match score (0–100) of a résumé against a
/// job's text. This is the SAME kernel as the Jobs-page ATS sub-score: detect
/// the stemmer language from the JD, extract+stem both sides, and report the
/// share of job keywords covered by the résumé. No embedding / API calls — safe
/// for the headless Autopilot scheduler.
///
/// Returns only the coverage percentage; callers that also need the missing
/// keywords should build the keyword sets and call [`keyword_coverage`].
pub fn coverage_score(resume_text: &str, job_text: &str) -> f64 {
    let stemmer = make_stemmer(job_text);
    let job_kw = keywords(job_text, &stemmer);
    let resume_kw = keywords(resume_text, &stemmer);
    // None → no extractable JD keywords; return 0.0 for the headless ranker
    // (Autopilot filters by minMatchScore, so 0.0 safely excludes sparse postings).
    keyword_coverage(&job_kw, &resume_kw).map_or(0.0, |(cov, _)| cov)
}
