//! Reply shaping — [`AnswerAssistOk`]/[`answer_assist_reply`].

use serde_json::json;

use crate::error::AppResult;

/// The `answer.assist` success outcome — see [`super::super::msg::ANSWER_ASSIST_RESULT`] docs.
#[derive(Debug)]
pub(in crate::extension_bridge) struct AnswerAssistOk {
    pub(super) question: String,
    pub(super) draft: String,
    pub(super) sourced_web: bool,
    pub(super) sourced_brief: bool,
    pub(super) sourced_salary: bool,
}

/// Build the `answer.assist` reply. Discriminated union, mirroring
/// `match_result_reply`/`answers_suggest_reply`: `ok:true` can never carry
/// `error`, and vice versa.
pub(in crate::extension_bridge) fn answer_assist_reply(
    req_id: &str,
    outcome: AppResult<AnswerAssistOk>,
) -> String {
    let payload = match outcome {
        Ok(ok) => json!({
            "ok": true,
            "question": ok.question,
            "draft": ok.draft,
            "sourced": {
                "web": ok.sourced_web,
                "brief": ok.sourced_brief,
                "salary": ok.sourced_salary,
            },
        }),
        // Wire-error discipline: `outcome`'s `Err` is ALWAYS one of the fixed
        // sentinel consts by the time it reaches here — every call in
        // `resolve_answer_assist` that could carry dynamic content is mapped
        // through `to_draft_failed` at its OWN call site first. So
        // `e.to_string()` is safe to serialize verbatim: no dynamic/path/PII
        // content ever reaches the wire; the real cause is logged
        // desktop-side only.
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    };
    json!({
        "type": super::super::msg::ANSWER_ASSIST_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}
