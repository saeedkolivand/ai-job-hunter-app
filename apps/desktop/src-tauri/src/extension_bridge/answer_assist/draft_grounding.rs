//! Draft-mode grounding (company brief / salary lookup / opt-in web-search
//! notes) + the grounded user message. Rewrite mode never reaches this: it
//! has no résumé/job/company/salary grounding at all (see `answer_rewrite`'s
//! module doc).

use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::applications::{normalize_question, Application, ApplicationStore};
use crate::error::{AppError, AppResult};
use crate::extension_bridge::answer_assist_parse::parse_draft_instruction;
use crate::extension_bridge::answer_assist_topic::{research_company_brief, AssistTopic};
use crate::extension_bridge::answers_suggest::is_salary_question;
use crate::pipeline::Completer;
use crate::salary_research::SalaryRange;

use super::context::resolve_context;
use super::grounding::{
    abort_if_cancelled_early, fetch_web_notes, resolve_salary_range, until_cancelled,
};

/// Ground a DRAFT-mode `answer.assist` request and build its user message —
/// the `AssistMode::Draft` half of `resolve_answer_assist`'s mode match.
/// Returns `(user, company_brief, web_notes, salary_range)`: the first is the
/// composed prompt; the other three feed back into the reply's `sourced`
/// flags (see [`super::reply::AnswerAssistOk`]).
#[allow(clippy::too_many_arguments)]
pub(super) async fn ground_draft(
    app: &AppHandle,
    req_id: &str,
    r#gen: u64,
    registry: &super::super::stream::AssistStreamRegistry,
    app_store: &ApplicationStore,
    completer: &Completer,
    limiter: &crate::limits::Limiter,
    provider_id: &str,
    topic: Option<AssistTopic>,
    question: &str,
    resume_text: &str,
    url: Option<&str>,
    search_web: bool,
    payload: &Value,
) -> AppResult<(String, String, String, Option<SalaryRange>)> {
    let app_ctx: Option<Application> = resolve_context(app_store, url);
    let job_description = app_ctx
        .as_ref()
        .map(|a| a.job_description.clone())
        .unwrap_or_default();
    let mut company_brief = app_ctx
        .as_ref()
        .map(|a| a.brief.clone())
        .filter(|b| !b.trim().is_empty())
        .unwrap_or_default();

    // The `company-brief` topic's grounding step (see
    // `answer_assist_topic::research_company_brief`) is a billable pre-register
    // round trip — see `grounding::abort_if_cancelled_early`'s doc for why it
    // needs this same spend guard.
    if topic == Some(AssistTopic::CompanyBrief) && company_brief.trim().is_empty() {
        abort_if_cancelled_early(registry, req_id, r#gen)?;
        company_brief = research_company_brief(completer, &job_description, app_ctx.as_ref()).await;
    }

    let is_salary = is_salary_question(&normalize_question(question));
    let salary_range = if is_salary {
        // Same spend guard as the company-brief grounding above, same reason.
        abort_if_cancelled_early(registry, req_id, r#gen)?;
        // Resolved once here (the same wiring `ai_lookup_salary_reasoned` uses) so a
        // repeat lookup for the same role/company/currency hits the SAME `salary_range`
        // `KvCache` namespace — both the plain salary-question flow AND the `salary-answer`
        // topic (which routes here too) get the 7-day cache instead of re-spending.
        let cache = app.try_state::<crate::pipeline::cache::KvCache>();
        match until_cancelled(
            registry,
            req_id,
            r#gen,
            resolve_salary_range(
                completer,
                limiter,
                provider_id,
                cache.as_deref(),
                app_ctx.as_ref(),
            ),
        )
        .await
        {
            Some(v) => v,
            None => return Err(AppError::Message("Job cancelled".to_string())),
        }
    } else {
        None
    };
    let web_notes = if search_web {
        // Third billable grounding step, same guard, same reason.
        abort_if_cancelled_early(registry, req_id, r#gen)?;
        match until_cancelled(
            registry,
            req_id,
            r#gen,
            fetch_web_notes(completer, limiter, provider_id, question, app_ctx.as_ref()),
        )
        .await
        {
            Some(v) => v,
            None => return Err(AppError::Message("Job cancelled".to_string())),
        }
    } else {
        String::new()
    };

    let user = super::prompt::build_user_message(
        question,
        resume_text,
        &job_description,
        &company_brief,
        &web_notes,
        salary_range.as_ref(),
        // The Regenerate box's typed text (optional — empty means no
        // block, `parse_draft_instruction` already trimmed + clamped
        // it, so it arrives bounded and hostile-parse-clean).
        &parse_draft_instruction(payload),
    );
    Ok((user, company_brief, web_notes, salary_range))
}
