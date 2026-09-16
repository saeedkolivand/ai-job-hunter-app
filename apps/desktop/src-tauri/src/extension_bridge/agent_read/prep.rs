//! `prep` resource (PR4 — Prep tab, extension-round-design.md decision 6) — the extension side
//! panel's read of a job's existing per-job AI generations: the company brief, the AI-suggested
//! interview questions, and the salary answer, all stored on the SAME `AiGenerationRecord`
//! (`ai_generations/mod.rs`, keyed by normalized `job_url`) `documents` (PR2) already looks up via
//! `AiGenerationStore::find_for_job`. New file, same pattern as `documents.rs` (R8 relief).
//!
//! Unlike `documents` (presence-only — the actual bytes only ever cross the wire through
//! `document.export`), this resource returns the TEXT itself: it is the whole point of the tab,
//! it is the user's own already-generated data (not fresh AI spend), and it rides the same read
//! tier, the same Autofill gate (checked one hop up, in `handle_agent_query`'s caller) and the
//! same 256 KiB extension reply cap every other resource does.
//!
//! ## Caps — truncate-with-a-flag, not refuse, not silent (decision)
//! Every generated field here is already token-bounded at WRITE time by the prompts that produced
//! it (a handful of interview questions, one ~150-word brief, one short salary line) — these caps
//! are a defensive backstop, not a routine truncation path, the same "audited constant on a small
//! picker list" reasoning `documents.rs`'s `MAX_DOCUMENTS` uses. Unlike that silent `.take()`
//! though, a cap hit here sets `truncated: true` on the reply: this is the user's OWN prep
//! content, so silently dropping part of it (a question, or the tail of a brief) must be visible
//! rather than merely policy — the caller can always fetch the rest in-app. Never refuse the whole
//! call over one long field, which would hide everything ELSE that fits.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ai_generations::AiGenerationRecord;
use crate::error::{AppError, AppResult};

/// Cap on the number of interview questions returned — generous headroom over what the
/// interview-question generator actually produces (a handful per job).
const MAX_INTERVIEW_QUESTIONS: usize = 12;
/// Char cap on the company brief — the brief is a short synthesized paragraph (~150 words); this
/// is several times that.
const MAX_BRIEF_CHARS: usize = 4_000;
/// Char cap per interview-question `question`/`why` field.
const MAX_QUESTION_FIELD_CHARS: usize = 500;
/// Char cap on the salary answer.
const MAX_SALARY_ANSWER_CHARS: usize = 2_000;

/// Clamp `s` to at most `max` CHARS (never splits a multi-byte character), reporting whether it
/// cut anything — same discipline as `answer_assist::clamp_chars`, extended to report the cut so
/// callers can fold it into one `truncated` flag rather than guessing from length after the fact.
fn clamp_chars_reporting(s: &str, max: usize) -> (String, bool) {
    if s.chars().count() <= max {
        (s.to_string(), false)
    } else {
        (s.chars().take(max).collect(), true)
    }
}

fn non_empty(s: String) -> Option<String> {
    (!s.trim().is_empty()).then_some(s)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrepInterviewQuestion {
    question: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    why: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audience: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrepGeneration {
    has_company_brief: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    company_brief: Option<String>,
    interview_questions: Vec<PrepInterviewQuestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_answer: Option<String>,
    /// The schema has no `updated_at` column on `ai_generations` (see that module's doc) — this
    /// mirrors `documents::project_generation`'s own `updated_at: record.created_at`, the record's
    /// own creation/last-save timestamp standing in for it.
    updated_at: u64,
    /// `true` iff ANY cap above was hit for this reply — see the module doc's caps section.
    truncated: bool,
}

/// Pure projection of one generation record → the Prep tab's own shape — directly unit-testable
/// against a hand-built `AiGenerationRecord`, no `AppHandle`. `pub(super)` for its own tests.
pub(super) fn project_generation(record: &AiGenerationRecord) -> Value {
    let mut truncated = false;

    let (brief, brief_cut) = clamp_chars_reporting(record.company_brief.trim(), MAX_BRIEF_CHARS);
    truncated |= brief_cut;
    let has_company_brief = !brief.is_empty();

    let interview_questions: Vec<PrepInterviewQuestion> = record
        .interview_questions
        .iter()
        .take(MAX_INTERVIEW_QUESTIONS)
        .map(|q| {
            let (question, q_cut) =
                clamp_chars_reporting(q.question.trim(), MAX_QUESTION_FIELD_CHARS);
            let (why, why_cut) = clamp_chars_reporting(q.why.trim(), MAX_QUESTION_FIELD_CHARS);
            truncated |= q_cut || why_cut;
            PrepInterviewQuestion {
                question,
                why: non_empty(why),
                audience: non_empty(q.audience.clone()),
            }
        })
        .collect();
    if record.interview_questions.len() > MAX_INTERVIEW_QUESTIONS {
        truncated = true;
    }

    // The salary answer rides `application_answers` under the fixed question id `"salary"` (see
    // this module's doc) — not a dedicated column.
    let salary_answer = record
        .application_answers
        .iter()
        .find(|a| a.id == "salary")
        .map(|a| a.answer.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            let (clamped, cut) = clamp_chars_reporting(s, MAX_SALARY_ANSWER_CHARS);
            truncated |= cut;
            clamped
        });

    json!(PrepGeneration {
        has_company_brief,
        company_brief: has_company_brief.then_some(brief),
        interview_questions,
        salary_answer,
        updated_at: record.created_at,
        truncated,
    })
}

pub(super) fn prep_resource(app: &AppHandle, payload: &Value) -> AppResult<Value> {
    let url = payload
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }
    // Same lookup `documents_resource` uses — `find_for_job` matches on the SAME normalized
    // `job_url` every other per-job surface on this bridge does; no second normalization here.
    let generation = app
        .try_state::<crate::ai_generations::AiGenerationStore>()
        .and_then(|store| store.find_for_job(url))
        .map(|record| project_generation(&record))
        .unwrap_or(Value::Null);
    Ok(json!({ "generation": generation }))
}

#[cfg(test)]
mod tests;
