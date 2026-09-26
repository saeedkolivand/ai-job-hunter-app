//! The fixed-sentinel error vocabulary for `answer.assist` + the pure gate/
//! collapse helpers around it.

use crate::error::AppError;

/// Fixed sentinel — the SEPARATE ai-assist opt-in is off. Never the
/// `AUTOFILL_OFF_MESSAGE` text — these are two distinct consent gates.
pub(crate) const AI_ASSIST_OFF_MESSAGE: &str =
    "AI answer drafting is off. Turn it on in AI Job Hunter → Settings → Browser extension.";

/// Fixed sentinel — the opt-in is on but no usable provider was ever
/// snapshotted (never configured, or resolution otherwise fails).
///
/// Together with [`AI_ASSIST_OFF_MESSAGE`] this is one of the TWO refusal
/// sentinels a client is allowed to RECOGNIZE rather than merely display
/// (ADR-044 decision 8 — the sentinel is the code; there is no `code`
/// field), mirrored in `packages/shared/src/ipc/extension-protocol-constants.ts`
/// and pinned to these exact strings by `message_type_constants_match_ts`.
pub(in crate::extension_bridge) const NO_PROVIDER_MESSAGE: &str =
    "No AI provider is set up for answer drafting. Open AI Job \
     Hunter → Settings → AI, choose a provider, then turn AI answer drafting back on in \
     Settings → Browser extension.";

/// Fixed sentinel — no résumé to ground the draft in.
pub(super) const NO_RESUME_MESSAGE: &str = "Add a resume in AI Job Hunter first, then try again.";

/// Fixed sentinel — a downstream limiter/provider call failed for ANY reason.
/// Calls past this point in [`super::resolve::resolve_answer_assist`] (the
/// rate/concurrency guard, the daily charge, the compose call) can carry
/// dynamic content in their `AppError` (raw provider HTTP/API text, an
/// endpoint, a rate-limit message) — none of which belongs on the wire.
/// Every one is mapped through [`to_draft_failed`] to this ONE fixed string
/// before it reaches [`super::reply::answer_assist_reply`]; the real cause
/// is logged desktop-side only. Distinct from
/// [`AI_ASSIST_OFF_MESSAGE`]/[`NO_PROVIDER_MESSAGE`]/[`NO_RESUME_MESSAGE`]
/// (refusal reasons the user can act on) — this is a generic
/// "something downstream failed".
pub(super) const DRAFT_FAILED_MESSAGE: &str = "Could not draft an answer. Please retry.";

/// Fixed sentinel — the compose failed for a reason retrying cannot fix: the
/// provider rejected our credentials. A 401/403 maps to [`AppError::Config`]
/// (`AppError::retriable()` classifies it `false`), so collapsing it into
/// [`DRAFT_FAILED_MESSAGE`]'s "Please retry." told the user to do the one
/// thing guaranteed to fail — and spend another unit of the daily budget
/// doing it. Same no-dynamic-content discipline as its sibling.
pub(super) const DRAFT_CONFIG_FAILED_MESSAGE: &str =
    "Your AI provider rejected the request — check the API key in AI Job Hunter → Settings → AI.";

/// Fixed sentinel — `req_id` already names an ACTIVE (`Pending`/`Running`)
/// stream on this connection (see [`super::super::stream::AssistStreamRegistry::begin`]).
/// A client reusing an in-flight reqId is rejected outright rather than
/// silently orphaning the original job.
pub(in crate::extension_bridge) const DUPLICATE_REQUEST_MESSAGE: &str =
    "This request is already in progress.";

/// The `answer.assist` consent gate in isolation: refuse with the fixed
/// [`AI_ASSIST_OFF_MESSAGE`] when the opt-in is off. Pure (no `AppHandle`) so
/// the gate itself is directly unit-testable — mirrors
/// `match_live::check_autofill_gate`'s isolation.
pub(super) fn check_ai_assist_gate(enabled: bool) -> Result<(), AppError> {
    if enabled {
        Ok(())
    } else {
        Err(AppError::Validation(AI_ASSIST_OFF_MESSAGE.to_string()))
    }
}

/// Collapse a downstream error that MAY carry dynamic content (see
/// [`DRAFT_FAILED_MESSAGE`]) to that one fixed sentinel, logging the real
/// cause desktop-side only (provider ids/rate-limit windows only, never a
/// URL/request body). Pure — directly unit-testable.
pub(super) fn to_draft_failed(context: &str, e: AppError) -> AppError {
    // Decided BEFORE the log line so the branch reads off the typed error, not
    // its rendered text.
    let non_retriable_config = matches!(e, AppError::Config(_));
    tracing::warn!("answer_assist: {context}: {e}");
    if non_retriable_config {
        return AppError::Provider(DRAFT_CONFIG_FAILED_MESSAGE.to_string());
    }
    AppError::Provider(DRAFT_FAILED_MESSAGE.to_string())
}
