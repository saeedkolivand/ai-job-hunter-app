//! The two on-demand Prep-tab drafts — an optional `topic` on an otherwise-normal DRAFT-mode
//! `answer.assist` request. Chosen over reusing `preset` (rewrite-only quick-action ids with their
//! own, unrelated meaning) so the two surfaces can never collide. When present,
//! `answer_assist::resolve_answer_assist` composes the `question` SERVER-SIDE (any client-sent
//! `question` is ignored) via [`topic_question`] and grounds it through the EXACT SAME draft
//! pipeline every other `answer.assist` call uses — same gate, limiter, registry, streaming frames:
//!
//! * `salary-answer`'s wording is not incidental — it is a whole token
//!   `answers_suggest::is_salary_question` recognizes ("salary"), routing it through the EXISTING
//!   salary-shaped grounding with no topic-specific salary code at all.
//! * `company-brief` rides [`research_company_brief`] below when the matched Application has no
//!   cached brief yet — the EXACT `CompanyResearch` enricher `ai_research_company` uses, never a
//!   second implementation.

use serde_json::Value;

use super::answer_assist::AssistMode;
use crate::applications::Application;
use crate::error::{AppError, AppResult};
use crate::pipeline::Completer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AssistTopic {
    CompanyBrief,
    SalaryAnswer,
}

/// Fixed sentinel for an unrecognized `topic` — flows through the SAME validation-error reply
/// shape every other malformed field on this verb already uses (e.g. "existingAnswer is
/// required"), never a new sentinel class.
const INVALID_TOPIC_MESSAGE: &str = "topic must be \"company-brief\" or \"salary-answer\"";

/// Fixed sentinel — `topic` is draft-only; it names nothing to compose in rewrite mode, which
/// already carries its own `existingAnswer`/preset.
pub(super) const TOPIC_REQUIRES_DRAFT_MESSAGE: &str = "topic is only valid when mode is \"draft\"";

pub(super) fn parse_topic(payload: &Value) -> AppResult<Option<AssistTopic>> {
    match payload.get("topic").and_then(|v| v.as_str()) {
        None => Ok(None),
        Some("company-brief") => Ok(Some(AssistTopic::CompanyBrief)),
        Some("salary-answer") => Ok(Some(AssistTopic::SalaryAnswer)),
        Some(_) => Err(AppError::Validation(INVALID_TOPIC_MESSAGE.to_string())),
    }
}

/// Reject a `topic` outside draft mode (see [`TOPIC_REQUIRES_DRAFT_MESSAGE`]) — extracted out of
/// `answer_assist::resolve_answer_assist`'s inline `if` so this branch is directly unit-testable
/// (the crate has no `tauri::test` mock-app harness), mirroring [`parse_topic`]'s own shape.
pub(super) fn topic_requires_draft(topic: Option<AssistTopic>, mode: AssistMode) -> AppResult<()> {
    if topic.is_some() && mode != AssistMode::Draft {
        Err(AppError::Validation(
            TOPIC_REQUIRES_DRAFT_MESSAGE.to_string(),
        ))
    } else {
        Ok(())
    }
}

/// The server-composed `question` for a topic-driven draft — the client sends no `question` at
/// all for these two buttons. Deliberately generic (never names the company here): the matched
/// Application's job description/company context already reaches the compose call as grounding
/// via `answer_assist::build_user_message`'s `job_posting`/`company_research` blocks, so the
/// question only needs to name WHAT to produce.
pub(super) fn topic_question(topic: AssistTopic) -> &'static str {
    match topic {
        AssistTopic::CompanyBrief => {
            "Give me a company brief for this role's employer — what they do, their mission or \
             focus, and anything notable about their current priorities that would help me \
             prepare to apply."
        }
        AssistTopic::SalaryAnswer => "What are your salary expectations for this role?",
    }
}

/// Fetch a fresh company brief through [`crate::cover_letter::research::CompanyResearch`] — the
/// SAME enricher `ai_research_company` calls, so this topic shares its 7-day cache and its
/// `admit_research`/`charge_daily` accounting rather than duplicating either. Degrades to `""` on
/// any failure (`enrich_with` never errors) — the draft still generates, just without a fresh
/// brief to ground in, same as today's ungrounded fallback.
pub(super) async fn research_company_brief(
    completer: &Completer,
    job_description: &str,
    app_ctx: Option<&Application>,
) -> String {
    let deadline = crate::commands::ai_provider::timeouts::research_deadline(None);
    let company_override = app_ctx
        .map(|a| a.company.as_str())
        .filter(|c| !c.trim().is_empty());
    let role_override = app_ctx
        .map(|a| a.title.as_str())
        .filter(|t| !t.trim().is_empty());
    crate::cover_letter::research::CompanyResearch
        .enrich_with(
            completer,
            job_description,
            company_override,
            role_override,
            deadline,
        )
        .await
        .content
}

#[cfg(test)]
mod tests;
