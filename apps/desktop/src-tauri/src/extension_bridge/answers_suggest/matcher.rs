//! The pure token-Jaccard question matcher — [`AnswerCandidate`]/[`Suggestion`]/
//! [`match_questions`]. Takes plain literals (no store, no `AppHandle`, no
//! timing) so it's directly unit-testable; deterministic.

use std::collections::HashSet;

use crate::applications::normalize_question;

use super::salary_match::is_salary_question;

/// Overall cap on the number of suggestions returned in one reply — a
/// pathological form (or a hostile collector) can't force an unbounded list.
pub(super) const MAX_SUGGESTIONS: usize = 20;

/// Minimum token-Jaccard similarity for a candidate to be suggested at all.
/// Tuned empirically against the regression pairs in `import_tests.rs`: 0.4 is
/// the highest threshold that still matches short-vs-verbose paraphrases like
/// "Notice period" vs "What is your notice period?" (score 0.4) and "Why do
/// you want to work here?" vs "Why do you want this role?" (score 0.44),
/// while unrelated questions still score 0.0.
///
/// **Chosen mitigation for the cross-question footgun, NOT stopword
/// filtering or a different threshold**: two unrelated questions can still
/// share enough filler words ("what is your") to cross this threshold.
/// Stopword-stripping risks breaking the short-paraphrase matches this value
/// was tuned against, so instead: (1) [`Suggestion::source_question`] always
/// carries the matched candidate's ORIGINAL question text so a cross-question
/// match is visually self-evident, and (2) [`match_questions`] flags `salary`
/// when EITHER side of the match is salary-shaped — a stored salary answer
/// can never slip out as fillable just because it matched on filler words.
const MIN_SCORE: f64 = 0.4;

/// Matcher-LOCAL tokenizer (NOT `normalize_question` — that stays untouched
/// since `answers.save`'s dedup depends on its exact output): split on any
/// non-alphanumeric character rather than whitespace, so trailing/embedded
/// punctuation never fractures a token — "notice period?" tokenizes to the
/// SAME `"period"` token as "notice period". Returns OWNED strings so the
/// result can be cached on [`AnswerCandidate`] past the lifetime of the
/// `String` it was tokenized from.
fn tokenize(s: &str) -> HashSet<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Token-Jaccard similarity between two ALREADY-TOKENIZED sets: the size of
/// their intersection over their union. `0.0` when either side is empty;
/// `1.0` for identical non-empty sets. Takes sets rather than raw strings so
/// a batch of questions scored against many candidates tokenizes each side
/// exactly once, not once per pair.
fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count();
    let union = a.union(b).count();
    inter as f64 / union as f64
}

/// One matchable candidate — the flat projection this module needs from a
/// stored `Application` + `ApplicationAnswer`, decoupled from both so
/// [`match_questions`] is unit-testable with plain literals. `tokens` is
/// normalized + tokenized ONCE here (not per comparison), so matching a batch
/// of questions against many candidates tokenizes each candidate
/// O(candidates) times, not O(questions × candidates).
pub(in crate::extension_bridge) struct AnswerCandidate<'a> {
    /// The RAW (un-normalized) question text this answer was originally
    /// stored under — kept so a match can surface it verbatim as
    /// [`Suggestion::source_question`] and so the salary guard can check it
    /// independently of the scanned input question (see `MIN_SCORE`'s doc).
    question: &'a str,
    answer: &'a str,
    tokens: HashSet<String>,
    company: &'a str,
    title: &'a str,
    updated_at: u64,
}

impl<'a> AnswerCandidate<'a> {
    pub(in crate::extension_bridge) fn new(
        question: &'a str,
        answer: &'a str,
        company: &'a str,
        title: &'a str,
        updated_at: u64,
    ) -> Self {
        Self {
            question,
            answer,
            tokens: tokenize(&normalize_question(question)),
            company,
            title,
            updated_at,
        }
    }
}

/// One matched suggestion — see [`super::msg::ANSWERS_SUGGEST_RESULT`] docs. `pub(in
/// crate::extension_bridge)` (struct + every field) — same reach as [`AnswerCandidate`].
#[derive(Debug, PartialEq)]
pub(in crate::extension_bridge) struct Suggestion {
    pub(in crate::extension_bridge) question: String,
    pub(in crate::extension_bridge) answer: String,
    pub(in crate::extension_bridge) source_company: Option<String>,
    pub(in crate::extension_bridge) source_title: Option<String>,
    /// The matched candidate's ORIGINAL (raw, un-normalized) question text —
    /// always present, never the scanned `question` above. Surfaced by the
    /// popup as "answered as: '…'" so a cross-question match (two questions
    /// similar enough on filler words to cross [`MIN_SCORE`] but about
    /// different things) is visually self-evident rather than silent.
    pub(in crate::extension_bridge) source_question: String,
    pub(in crate::extension_bridge) score: f64,
    /// Copy-only when true — see `salary_match::SALARY_KEYWORDS`. True when EITHER the
    /// scanned input question OR the matched candidate's own
    /// `source_question` is salary-shaped, so a stored salary answer can
    /// never surface as fillable just because it matched under an unrelated
    /// label (see the mitigation note on [`MIN_SCORE`]).
    pub(in crate::extension_bridge) salary: bool,
}

/// Pure matcher: for each (deduped-by-normalized-text) entry of `questions`,
/// find the best-scoring `candidates` entry at/above [`MIN_SCORE`] — ties
/// broken by score desc, then `updated_at` desc (the most recently updated
/// application wins) — and emit at most one [`Suggestion`] per question,
/// capped overall at [`MAX_SUGGESTIONS`]. Deterministic: the same
/// `questions`/`candidates` always produce the same output.
pub(in crate::extension_bridge) fn match_questions(
    questions: &[String],
    candidates: &[AnswerCandidate],
) -> Vec<Suggestion> {
    let mut seen_normalized: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for q in questions {
        if out.len() >= MAX_SUGGESTIONS {
            break;
        }
        let norm_q = normalize_question(q);
        if norm_q.is_empty() || !seen_normalized.insert(norm_q.clone()) {
            continue; // blank, or an effective duplicate of an earlier question
        }
        let q_tokens = tokenize(&norm_q);

        let mut best: Option<(&AnswerCandidate, f64)> = None;
        for c in candidates {
            let score = jaccard(&q_tokens, &c.tokens);
            if score < MIN_SCORE {
                continue;
            }
            best = match best {
                None => Some((c, score)),
                Some((_, best_score)) if score > best_score => Some((c, score)),
                Some((prev, best_score))
                    if score == best_score && c.updated_at > prev.updated_at =>
                {
                    Some((c, score))
                }
                other => other,
            };
        }

        if let Some((c, score)) = best {
            // Either side salary-shaped forces Copy-only — a stored salary
            // answer must never surface as fillable just because it matched
            // under an unrelated scanned label (see MIN_SCORE's doc).
            let salary =
                is_salary_question(&norm_q) || is_salary_question(&normalize_question(c.question));
            out.push(Suggestion {
                question: q.clone(),
                answer: c.answer.to_string(),
                source_company: (!c.company.trim().is_empty()).then(|| c.company.to_string()),
                source_title: (!c.title.trim().is_empty()).then(|| c.title.to_string()),
                source_question: c.question.to_string(),
                score,
                salary,
            });
        }
    }

    out
}
