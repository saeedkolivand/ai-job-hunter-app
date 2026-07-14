//! "Suggest answers for this form" (`answers.suggest` → `answers.suggest.result`)
//! — the headline replay verb (extension roadmap PR 6). Fuzzy-match each EMPTY
//! question label the popup's questions-mode collector scanned against EVERY
//! stored [`ApplicationAnswer`] across ALL applications, and return the best
//! per-question match. Split out of `mod.rs` per the R8 LOC cap (mirrors
//! `answers_save.rs`/`status_update.rs`'s module split); `resolve_*`/`handle_*`
//! pure/impure split mirrors those siblings.
//!
//! **Consent-gate boundary**: a suggestion carries the user's OWN past answer
//! text desktop→extension — the same PII-adjacent consent class as
//! `profile.get`'s contact fields — so it rides the SAME assisted-autofill
//! opt-in (`BridgeState::autofill_enabled`), never a separate gate.
//!
//! **Read-only, no new store method**: `applications/mod.rs` sits at the R8
//! hard LOC cap, so this module reads via the ALREADY-public, read-only
//! `ApplicationStore::list()` rather than adding a store method there — the
//! read lives here, in the bridge module, exactly as the PR-6 handoff flagged.
//!
//! **Pure matcher**: [`match_questions`] takes plain [`AnswerCandidate`]
//! literals (no store, no `AppHandle`, no timing) — normalize
//! ([`crate::applications::normalize_question`], shared with `answers.save`'s
//! dedup) + token-Jaccard similarity, thresholded, tied-break by score then
//! most-recent `updated_at`. Deterministic: the same inputs always produce the
//! same outputs — no AI, no egress, no randomness.

use std::collections::HashSet;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::msg;
use crate::applications::{normalize_question, ApplicationStore};
use crate::error::{AppError, AppResult};

/// Hard cap on the number of questions a single `answers.suggest` call may
/// carry (mirrors `answers_save::MAX_ANSWERS_PER_CALL`) — extras are silently
/// dropped, never rejected outright.
const MAX_QUESTIONS_PER_CALL: usize = 50;

/// Per-question byte cap, char-boundary safe (mirrors `answers_save`'s
/// `MAX_QUESTION_BYTES`) — untrusted page-derived label text is clamped at
/// this boundary, never dropped wholesale.
const MAX_QUESTION_BYTES: usize = 1_000;

/// Overall cap on the number of suggestions returned in one reply — a
/// pathological form (or a hostile collector) can't force an unbounded list.
const MAX_SUGGESTIONS: usize = 20;

/// Minimum token-Jaccard similarity for a candidate to be suggested at all.
/// Tuned empirically against the regression pairs in `import_tests.rs`: 0.4 is
/// the highest threshold that still matches short-vs-verbose paraphrases like
/// "Notice period" vs "What is your notice period?" (score 0.4 — 2 shared
/// tokens over a 5-token union) and "Why do you want to work here?" vs "Why
/// do you want this role?" (score 0.44 — {why,do,you,want} over a 9-token
/// union), while unrelated questions like "What is your salary expectation?"
/// vs "Do you have a driver's license?" still score 0.0 and never match.
const MIN_SCORE: f64 = 0.4;

/// Salary-ish keyword denylist for the Copy-only rule: a suggestion whose
/// (normalized) INPUT question contains any of these must never offer "Fill
/// this field" — pasting a stored salary figure into the wrong context on a
/// live form is exactly the kind of silent mistake this feature must never
/// make. Checked against the scanned question text, not the stored answer.
/// "rate" is deliberately NEVER listed bare — only as a salary-shaped
/// multi-token phrase ("day rate"/"hourly rate"/"pay rate") — because a bare
/// "rate" would false-positive an unrelated "Rate your TypeScript skills"
/// question.
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
];

/// Clamp `s` to at most `max` bytes, cutting on a UTF-8 char boundary — same
/// discipline as `answers_save::clamp_bytes` (duplicated here as a tiny pure
/// helper rather than exported cross-module; that cap is this verb's own).
fn clamp_bytes(mut s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s
}

/// Order-preserving counterpart of [`tokenize`], used ONLY by
/// [`is_salary_question`]'s multi-word phrase check: `tokenize`'s `HashSet`
/// can't tell you "day" was immediately followed by "rate", so it can't back
/// a substring check like `"day rate"`. Same split boundary (any
/// non-alphanumeric char), kept in sequence.
fn tokenize_ordered(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect()
}

/// True when `normalized` (already `normalize_question`-lowercased/
/// whitespace-collapsed) contains a salary-ish keyword. Re-tokenized on any
/// non-alphanumeric boundary and rejoined with single spaces before the
/// check — `normalize_question` only collapses WHITESPACE, so a hyphen/slash
/// question like "Day-rate"/"day/rate" would otherwise still carry the
/// literal punctuation and silently miss the "day rate" phrase.
fn is_salary_question(normalized: &str) -> bool {
    let rejoined = tokenize_ordered(normalized).join(" ");
    SALARY_KEYWORDS.iter().any(|kw| rejoined.contains(kw))
}

/// Matcher-LOCAL tokenizer (NOT `normalize_question` — that stays untouched
/// since `answers.save`'s dedup depends on its exact output): split on any
/// non-alphanumeric character rather than whitespace, so trailing/embedded
/// punctuation never fractures a token from its bare form elsewhere — "notice
/// period?" tokenizes to the SAME `"period"` token as "notice period", where a
/// naive whitespace split would leave a dangling `"period?"` that can never
/// match. Empty splits (consecutive punctuation) are dropped.
fn tokenize(s: &str) -> HashSet<&str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Token-Jaccard similarity between two ALREADY-NORMALIZED strings (see
/// [`tokenize`]): the size of their token-set intersection over their union.
/// `0.0` when either side is empty (no tokens) or they share no token; `1.0`
/// for two token-identical non-empty strings.
fn jaccard(a: &str, b: &str) -> f64 {
    let ta = tokenize(a);
    let tb = tokenize(b);
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let inter = ta.intersection(&tb).count();
    let union = ta.union(&tb).count();
    inter as f64 / union as f64
}

/// One matchable candidate — the flat projection this module needs from a
/// stored `Application` + `ApplicationAnswer`, decoupled from both so
/// [`match_questions`] is unit-testable with plain literals (no SQLite store,
/// no `updated_at` timing race). `normalized_question` is computed once here
/// (not per comparison) so matching a batch of questions against many
/// candidates stays O(candidates) normalizations, not O(questions ×
/// candidates).
pub(super) struct AnswerCandidate<'a> {
    answer: &'a str,
    normalized_question: String,
    company: &'a str,
    title: &'a str,
    updated_at: u64,
}

impl<'a> AnswerCandidate<'a> {
    pub(super) fn new(
        question: &'a str,
        answer: &'a str,
        company: &'a str,
        title: &'a str,
        updated_at: u64,
    ) -> Self {
        Self {
            answer,
            normalized_question: normalize_question(question),
            company,
            title,
            updated_at,
        }
    }
}

/// One matched suggestion — see [`msg::ANSWERS_SUGGEST_RESULT`] docs.
#[derive(Debug, PartialEq)]
pub(super) struct Suggestion {
    pub(super) question: String,
    pub(super) answer: String,
    pub(super) source_company: Option<String>,
    pub(super) source_title: Option<String>,
    pub(super) score: f64,
    /// Copy-only when true — see [`SALARY_KEYWORDS`].
    pub(super) salary: bool,
}

/// Pure matcher: for each (deduped-by-normalized-text) entry of `questions`,
/// find the best-scoring `candidates` entry at/above [`MIN_SCORE`] — ties
/// broken by score desc, then `updated_at` desc (the most recently updated
/// application wins) — and emit at most one [`Suggestion`] per question,
/// capped overall at [`MAX_SUGGESTIONS`]. Deterministic: the same
/// `questions`/`candidates` always produce the same output.
pub(super) fn match_questions(
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

        let mut best: Option<(&AnswerCandidate, f64)> = None;
        for c in candidates {
            let score = jaccard(&norm_q, &c.normalized_question);
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
            out.push(Suggestion {
                question: q.clone(),
                answer: c.answer.to_string(),
                source_company: (!c.company.trim().is_empty()).then(|| c.company.to_string()),
                source_title: (!c.title.trim().is_empty()).then(|| c.title.to_string()),
                score,
                salary: is_salary_question(&norm_q),
            });
        }
    }

    out
}

/// Parse + clamp the incoming `questions` array off the payload, capped at
/// [`MAX_QUESTIONS_PER_CALL`] (mirrors `answers_save::parse_answers`). A
/// non-string / blank entry is dropped, not rejected.
fn parse_questions(payload: &Value) -> Vec<String> {
    payload
        .get("questions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| entry.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| clamp_bytes(s.to_string(), MAX_QUESTION_BYTES))
                .take(MAX_QUESTIONS_PER_CALL)
                .collect()
        })
        .unwrap_or_default()
}

/// Core `answers.suggest`: gate on the autofill opt-in (same fixed sentinel as
/// `profile.get`/`answers.save` — see [`super::AUTOFILL_OFF_MESSAGE`]), then
/// fuzzy-match the (clamped, capped) incoming `questions` against EVERY
/// answer on EVERY stored Application via [`ApplicationStore::list`] — pure
/// local Rust, no AI, no egress. Read-only: never writes.
pub(super) fn resolve_answers_suggest(
    store: &ApplicationStore,
    autofill_enabled: bool,
    payload: &Value,
) -> AppResult<Vec<Suggestion>> {
    if !autofill_enabled {
        return Err(AppError::Validation(
            super::AUTOFILL_OFF_MESSAGE.to_string(),
        ));
    }

    let questions = parse_questions(payload);
    if questions.is_empty() {
        return Ok(Vec::new());
    }

    let apps = store.list();
    let candidates: Vec<AnswerCandidate> = apps
        .iter()
        .flat_map(|app| {
            app.answers.iter().map(move |a| {
                AnswerCandidate::new(
                    &a.question,
                    &a.answer,
                    &app.company,
                    &app.title,
                    app.updated_at,
                )
            })
        })
        .collect();

    Ok(match_questions(&questions, &candidates))
}

/// Build the `answers.suggest` reply. Mirrors `answers_result_reply` — a
/// discriminated union so success/failure fields can never mix.
pub(super) fn answers_suggest_reply(req_id: &str, outcome: AppResult<Vec<Suggestion>>) -> String {
    let payload = match outcome {
        Ok(suggestions) => {
            let arr: Vec<Value> = suggestions
                .into_iter()
                .map(|s| {
                    let mut obj = serde_json::Map::new();
                    obj.insert("question".to_string(), json!(s.question));
                    obj.insert("answer".to_string(), json!(s.answer));
                    if let Some(c) = s.source_company {
                        obj.insert("sourceCompany".to_string(), json!(c));
                    }
                    if let Some(t) = s.source_title {
                        obj.insert("sourceTitle".to_string(), json!(t));
                    }
                    obj.insert("score".to_string(), json!(s.score));
                    obj.insert("salary".to_string(), json!(s.salary));
                    Value::Object(obj)
                })
                .collect();
            json!({ "ok": true, "suggestions": arr })
        }
        // Wire-error discipline: fixed sentinel text only (no dynamic/path/PII
        // content) — detailed context belongs in the desktop log, not on the wire.
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    };
    json!({
        "type": msg::ANSWERS_SUGGEST_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Answer an authenticated `answers.suggest`: resolve against the local
/// `ApplicationStore` (gated on the autofill opt-in) and return a
/// ready-to-send `answers.suggest.result` reply.
pub(super) fn handle_answers_suggest(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let enabled = app
        .try_state::<super::BridgeState>()
        .map(|s| s.autofill_enabled())
        .unwrap_or(false);
    let outcome = app
        .try_state::<ApplicationStore>()
        .ok_or_else(|| AppError::Config("applications store unavailable".to_string()))
        .and_then(|store| resolve_answers_suggest(store.inner(), enabled, payload));
    answers_suggest_reply(req_id, outcome)
}
