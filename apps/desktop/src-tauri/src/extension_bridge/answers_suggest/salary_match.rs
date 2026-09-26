//! Salary-shaped question recognition + the synthetic salary-expectation
//! suggestion it backs — split out of the parent module for the R8 line-budget
//! cap.

use std::collections::HashSet;

use crate::applications::normalize_question;

use super::matcher::{Suggestion, MAX_SUGGESTIONS};

/// Salary-ish keyword denylist for the Copy-only rule: a suggestion whose
/// (normalized) INPUT question OR matched candidate's SOURCE question
/// contains any of these must never offer "Fill this field" — pasting a
/// stored salary figure into the wrong context on a live form is exactly the
/// kind of silent mistake this feature must never make. "rate" is
/// deliberately NEVER listed bare — only as a multi-token phrase ("day
/// rate"/"hourly rate"/"pay rate") — because a bare "rate" would
/// false-positive "Rate your TypeScript skills".
///
/// **DACH (German) shapes** — the desktop's largest non-English user base.
/// Each is its own token (German compounds nouns rather than phrases), so a
/// bare `"gehalt"` does NOT catch `"gehaltsvorstellung"` etc. — every
/// compound actually seen on DACH forms is listed explicitly. Umlauts need
/// no ASCII-folded variant: `normalize_question` lowercases Unicode-aware
/// ("Ü" → "ü") and tokenizing splits on `!char::is_alphanumeric`, which keeps
/// "ü" IN the token — "vergütung" tokenizes to one token, matching this
/// list's entry byte-for-byte. A near-miss is deliberately left UNFLAGGED:
/// "Gehaltsabrechnung hochladen" ("upload payslip") tokenizes to
/// `{gehaltsabrechnung, hochladen}` — no exact-token match — which is
/// correct: it asks for a file upload, not a stated figure.
const SALARY_KEYWORDS: &[&str] = &[
    "salary",
    "compensation",
    "comp expectation",
    "pay expectation",
    "expected pay",
    "desired pay",
    "wage",
    "remuneration",
    "ctc",
    "income",
    "day rate",
    "hourly rate",
    "pay rate",
    "how much",
    "paid",
    "gehalt",
    "gehaltsvorstellung",
    "gehaltsvorstellungen",
    "gehaltswunsch",
    "bruttojahresgehalt",
    "jahresgehalt",
    "vergütung",
    "salärvorstellung",
];

/// Order-preserving counterpart of the matcher's `tokenize`, used ONLY by
/// [`is_salary_question`]'s multi-word phrase check: a `HashSet` can't tell
/// you "day" was immediately followed by "rate". Same split boundary, kept
/// in sequence.
fn tokenize_ordered(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect()
}

/// True when `normalized` (already `normalize_question`-lowercased/
/// whitespace-collapsed) contains a salary-ish keyword. Re-tokenized on any
/// non-alphanumeric boundary before the check — `normalize_question` only
/// collapses WHITESPACE, so a hyphen/slash question like "Day-rate" would
/// otherwise still carry the punctuation and silently miss "day rate". A
/// single-word keyword (e.g. "paid") must match a WHOLE token (else "unpaid"
/// false-positives); a multi-word keyword has no single token to match, so
/// it stays a substring-of-rejoined check.
///
/// Shared with [`super::super::answer_assist`], which routes a salary-shaped
/// `answer.assist` question through the salary machinery instead of a
/// generic grounded draft.
pub(in crate::extension_bridge) fn is_salary_question(normalized: &str) -> bool {
    let tokens = tokenize_ordered(normalized);
    let rejoined = tokens.join(" ");
    SALARY_KEYWORDS.iter().any(|kw| {
        if kw.contains(' ') {
            rejoined.contains(kw)
        } else {
            tokens.contains(kw)
        }
    })
}

/// Fixed source label for a synthetic salary suggestion — see
/// [`append_salary_expectation_suggestions`]'s doc for why `sourceCompany`
/// (not a new field) carries it.
const SAVED_EXPECTATION_SOURCE: &str = "Saved expectation";

/// After the real (stored-answer) matches, append one synthetic suggestion
/// per remaining salary-shaped question — the backend-readable
/// `job_preferences.salary_expectation` filling a gap NO stored
/// `ApplicationAnswer` covers. **Stored answer wins**: a question already
/// present in `existing` (by normalized text) is skipped entirely. Mutates
/// `existing` in place and respects [`MAX_SUGGESTIONS`] jointly with what's
/// already there.
///
/// Fields: `answer` is the saved expectation string VERBATIM; `source_company`
/// carries the fixed [`SAVED_EXPECTATION_SOURCE`] label (reusing the existing
/// "from your X application" wire field rather than adding a new one);
/// `source_question` echoes the scanned question itself; `score: 1.0` (not
/// matcher-derived); `salary: true` always — copy-only forever, the same
/// flag every stored salary match already forces.
pub(super) fn append_salary_expectation_suggestions(
    existing: &mut Vec<Suggestion>,
    questions: &[String],
    expectation: &str,
) {
    let mut covered: HashSet<String> = existing
        .iter()
        .map(|s| normalize_question(&s.question))
        .collect();
    for q in questions {
        if existing.len() >= MAX_SUGGESTIONS {
            break;
        }
        let norm_q = normalize_question(q);
        if norm_q.is_empty() || !covered.insert(norm_q.clone()) || !is_salary_question(&norm_q) {
            continue;
        }
        existing.push(Suggestion {
            question: q.clone(),
            answer: expectation.to_string(),
            source_company: Some(SAVED_EXPECTATION_SOURCE.to_string()),
            source_title: None,
            source_question: q.clone(),
            score: 1.0,
            salary: true,
        });
    }
}
