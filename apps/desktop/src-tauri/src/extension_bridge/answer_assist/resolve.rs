//! Core `answer.assist` resolve — [`resolve_answer_assist`].

use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::applications::ApplicationStore;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};
use crate::extension_bridge::answer_assist_parse::{
    assist_prompt_for_mode, clamp_bytes, clamp_chars, parse_mode, parse_question, parse_search_web,
    parse_url, validate_rewrite_fields,
};
use crate::extension_bridge::answer_assist_topic::{
    parse_topic, topic_question, topic_requires_draft,
};
use crate::pipeline::Completer;

use super::budgets::{ANSWER_ASSIST_RETRY_MAX_TOKENS, MAX_QUESTION_BYTES};
use super::compose::{compose_with_length_retry, BridgeComposeRound};
use super::errors::{check_ai_assist_gate, NO_PROVIDER_MESSAGE, NO_RESUME_MESSAGE};
use super::reply::AnswerAssistOk;
use super::AssistMode;

/// Core `answer.assist`: gate on the ai-assist opt-in FIRST, clamp the
/// question, resolve a provider from the persisted snapshot, resolve the
/// default résumé (draft mode only), validate rewrite mode's required fields
/// (see [`super::super::answer_assist_parse::resolve_rewrite_instruction`]) —
/// every one of these runs BEFORE the shared `"ai_research"` limiter acquire
/// below: per `limits::Limiter`'s own doc, a rate-window slot is consumed on
/// ACQUIRE and never released early, so a validation failure here returns
/// `Err` without spending a legitimate caller's limited slot. Routes
/// salary-shaped questions through the salary machinery (scraped range →
/// market lookup) and every other question through a grounded draft.
#[allow(clippy::too_many_arguments)]
pub(super) async fn resolve_answer_assist(
    app: &AppHandle,
    req_id: &str,
    r#gen: u64,
    ai_assist_enabled: bool,
    app_store: &ApplicationStore,
    doc_store: &DocumentStore,
    payload: &Value,
    registry: &super::super::stream::AssistStreamRegistry,
    sink: &mut dyn super::super::FrameSink,
) -> AppResult<AnswerAssistOk> {
    check_ai_assist_gate(ai_assist_enabled)?;

    let mode = parse_mode(payload);
    let topic = parse_topic(payload)?;
    topic_requires_draft(topic, mode)?;
    // A topic-driven request composes its own `question` server-side (see
    // `topic_question`'s doc) — any client-sent `question` is ignored, never
    // merely preferred, so a caller can't smuggle a different question in
    // under a topic's grounding.
    let question = match topic {
        Some(t) => topic_question(t).to_string(),
        None => clamp_bytes(parse_question(payload), MAX_QUESTION_BYTES),
    };
    if question.is_empty() {
        return Err(AppError::Validation("question is required".to_string()));
    }
    let url = parse_url(payload);
    let search_web = parse_search_web(payload);

    // Routing is backend-owned (task #16): resolve the active provider/model/
    // base_url from the `AiConfigStore` — the SAME source `ai_generate` uses —
    // never a renderer-supplied snapshot. This closes the persisted-base_url
    // SSRF the old `ai_assist` snapshot carried.
    let completer = Completer::from_active(app).map_err(|e| {
        tracing::debug!("answer_assist: provider resolution failed: {e}");
        AppError::Config(NO_PROVIDER_MESSAGE.to_string())
    })?;

    // Rewrite mode is a PURE TEXT TRANSFORM (see `answer_rewrite`'s module
    // doc) — it never grounds in the résumé, so it never requires one to
    // exist, unlike draft mode below.
    let resume_text = match mode {
        AssistMode::Draft => {
            let docs = doc_store.list();
            let resume = crate::extension_bridge::match_live::resolve_resume(&docs)
                .ok_or_else(|| AppError::Validation(NO_RESUME_MESSAGE.to_string()))?;
            resume.text.clone()
        }
        AssistMode::Rewrite => String::new(),
    };

    // Rewrite mode's required-field validation — moved here (BEFORE the
    // limiter acquire), mirroring the `question` gate above: a malformed
    // rewrite frame must never consume an `ai_research` rate-window slot at
    // zero provider spend. Computed once; the `AssistMode::Rewrite` arm below
    // consumes the result directly instead of re-validating.
    let rewrite_fields = match mode {
        AssistMode::Rewrite => Some(validate_rewrite_fields(payload)?),
        AssistMode::Draft => None,
    };

    // Bound spend for the rest of this call — the SAME bucket
    // `ai_lookup_salary`/`ai_research_company`/`ai_research_answer` share.
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    let _guard = limiter
        .acquire(
            "ai_research",
            crate::limits::AI_RESEARCH_RATE_MAX,
            crate::limits::AI_RESEARCH_CONCURRENCY_MAX,
        )
        .map_err(|e| super::errors::to_draft_failed("rate limited", e))?;

    let provider_id = completer.provider_id().as_str();

    // `registry.begin(req_id)` already ran, SYNCHRONOUSLY, before this
    // function was ever called (see `stream::spawn_answer_assist`'s doc: a
    // same-connection `assist.cancel` must never race ahead of `begin`
    // through `tokio::spawn`'s scheduling gap), so the `Pending` entry it
    // left behind is guaranteed to exist by this point.

    let (system, max_tokens) = assist_prompt_for_mode(mode);

    // Job/company/salary/web-search grounding + the rewrite user message
    // diverge by mode right here; everything below is shared again (the one
    // `compose_draft_stream` call and the reply shaping).
    let (user, company_brief, web_notes, salary_range) = match mode {
        AssistMode::Draft => {
            super::draft_grounding::ground_draft(
                app,
                req_id,
                r#gen,
                registry,
                app_store,
                &completer,
                &limiter,
                provider_id,
                topic,
                &question,
                &resume_text,
                url.as_deref(),
                search_web,
                payload,
            )
            .await?
        }
        AssistMode::Rewrite => {
            // Already validated above, BEFORE the limiter acquire; this arm
            // guarantees `Some`, so this never actually panics.
            let (existing_answer, instruction) =
                rewrite_fields.expect("validated before the limiter acquire, above");
            let user = crate::extension_bridge::answer_rewrite::build_rewrite_user_message(
                &existing_answer,
                &instruction,
            );
            (user, String::new(), String::new(), None)
        }
    };

    // The compose call itself — charged (per round-trip, inside
    // `compose_with_length_retry`) then streamed. A rejected charge is just
    // another `Err` this function returns — it does NOT `unregister` here;
    // `handle_answer_assist` is the SOLE unregister owner.
    //
    // A cheap effort tier (when this provider/model has one) is resolved ONCE
    // and used for every attempt — see `ANSWER_ASSIST_MAX_TOKENS`'s doc.
    let effort = completer.low_effort();
    let mut round = BridgeComposeRound {
        stream: crate::extension_bridge::stream::ComposeStream {
            app,
            completer: &completer,
            req_id,
            r#gen,
            registry,
            system,
            user: &user,
            sink,
            forwarded: String::new(),
        },
        limiter: &limiter,
        provider_id,
    };
    let draft = clamp_chars(
        compose_with_length_retry(
            &mut round,
            max_tokens,
            ANSWER_ASSIST_RETRY_MAX_TOKENS,
            effort,
        )
        .await?,
        super::budgets::DRAFT_CAP,
    );

    Ok(AnswerAssistOk {
        question,
        draft,
        sourced_web: !web_notes.trim().is_empty(),
        sourced_brief: !company_brief.is_empty(),
        sourced_salary: salary_range.is_some(),
    })
}
