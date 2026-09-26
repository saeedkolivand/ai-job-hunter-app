//! "Suggest answers for this form" (`answers.suggest` → `answers.suggest.result`)
//! — the headline replay verb. Fuzzy-match each EMPTY question label the
//! popup's questions-mode collector scanned against EVERY stored
//! [`ApplicationAnswer`] across ALL applications, and return the best
//! per-question match.
//!
//! **Consent-gate boundary**: a suggestion carries the user's OWN past answer
//! text desktop→extension — the same PII-adjacent consent class as
//! `profile.get`'s contact fields — so it rides the SAME assisted-autofill
//! opt-in (`BridgeState::autofill_enabled`), never a separate gate.
//!
//! **Read-only**: reads via the already-public `ApplicationStore::list()`
//! rather than adding a store method.
//!
//! Split into [`matcher`] (the pure token-Jaccard matching engine) and
//! [`salary_match`] (salary-shaped-question recognition + the synthetic
//! salary-expectation suggestion).

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use self::salary_match::append_salary_expectation_suggestions;
use super::answer_assist_parse::clamp_bytes;
use super::msg;
use crate::applications::ApplicationStore;
use crate::error::{AppError, AppResult};

mod matcher;
mod salary_match;

// Re-exports so `answer_assist` (salary-shaped question routing) and `import_tests.rs` keep
// resolving `answers_suggest::X` unchanged now that these live one module deeper.
pub(in crate::extension_bridge) use self::matcher::{match_questions, AnswerCandidate, Suggestion};
pub(super) use self::salary_match::is_salary_question;

/// Hard cap on the number of questions a single `answers.suggest` call may
/// carry (mirrors `answers_save::MAX_ANSWERS_PER_CALL`) — extras are silently
/// dropped, never rejected outright.
const MAX_QUESTIONS_PER_CALL: usize = 50;

/// Per-question byte cap, char-boundary safe (mirrors `answers_save`'s
/// `MAX_QUESTION_BYTES`) — untrusted page-derived label text is clamped at
/// this boundary, never dropped wholesale.
const MAX_QUESTION_BYTES: usize = 1_000;

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

/// Core `answers.suggest`: gate on the autofill opt-in (see
/// [`super::AUTOFILL_OFF_MESSAGE`]), then fuzzy-match the (clamped, capped)
/// incoming `questions` against EVERY answer on EVERY stored Application via
/// [`ApplicationStore::list`] — pure local Rust, no AI, no egress, never
/// writes.
///
/// `salary_expectation` is the backend-readable
/// `job_preferences.salary_expectation` (may be absent/blank). When present
/// it appends a synthetic suggestion for each remaining salary-shaped
/// question no stored answer already covers — see
/// [`salary_match::append_salary_expectation_suggestions`].
pub(super) fn resolve_answers_suggest(
    store: &ApplicationStore,
    autofill_enabled: bool,
    salary_expectation: Option<&str>,
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

    let mut suggestions = match_questions(&questions, &candidates);
    if let Some(expectation) = salary_expectation.map(str::trim).filter(|s| !s.is_empty()) {
        append_salary_expectation_suggestions(&mut suggestions, &questions, expectation);
    }
    Ok(suggestions)
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
                    obj.insert("sourceQuestion".to_string(), json!(s.source_question));
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
/// ready-to-send `answers.suggest.result` reply. The backend-readable salary
/// expectation (Task #30) rides the SAME managed state fetch pattern as the
/// opt-in — an absent `JobPreferencesStore` (start-up failure) just means no
/// synthetic row, never an error.
pub(super) fn handle_answers_suggest(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let enabled = app
        .try_state::<super::BridgeState>()
        .map(|s| s.autofill_enabled())
        .unwrap_or(false);
    let salary_expectation = app
        .try_state::<crate::job_preferences::JobPreferencesStore>()
        .and_then(|s| s.get().salary_expectation);
    let outcome = app
        .try_state::<ApplicationStore>()
        .ok_or_else(|| AppError::Config("applications store unavailable".to_string()))
        .and_then(|store| {
            resolve_answers_suggest(
                store.inner(),
                enabled,
                salary_expectation.as_deref(),
                payload,
            )
        });
    answers_suggest_reply(req_id, outcome)
}
