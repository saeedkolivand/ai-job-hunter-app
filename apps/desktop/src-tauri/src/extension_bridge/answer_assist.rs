//! "Help me answer this question" (`answer.assist` → `answer.assist.result`).
//! One-shot + copy-only: the popup renders the returned `draft` as
//! `textContent` with a Copy button — there is NO fill path for AI text.
//!
//! ## Consent gate — a SEPARATE opt-in from assisted autofill
//! Unlike `profile.get`/`answers.save`/`answers.suggest`/`match.live` (all
//! gated on `BridgeState::autofill_enabled`), this verb rides its OWN
//! `BridgeState::ai_assist_enabled` gate — billable provider spend is a
//! different consent class — checked FIRST, before parsing/resolving/
//! spending anything. See [`errors::check_ai_assist_gate`].
//!
//! ## Provider resolution — the backend-owned active-provider store
//! Resolved from the backend-owned [`crate::ai_config::AiConfigStore`] via
//! [`crate::pipeline::Completer::from_active`] — the SAME source of truth
//! `ai_generate`/Autopilot use — never a renderer-supplied snapshot. This
//! closes a persisted-base_url SSRF the old `ai_assist` snapshot carried (a
//! one-time XSS could pin an attacker endpoint that every future draft then
//! replayed). An unset store resolves to [`errors::NO_PROVIDER_MESSAGE`],
//! never a silent no-op.
//!
//! ## Context-aware drafting
//! A salary-shaped question (shared [`super::answers_suggest::is_salary_question`])
//! is grounded in, in order: (1) the URL-matched Application's own SCRAPED
//! salary range (the employer's own stated figure, never a market estimate);
//! (2) failing that, a web-researched market range via
//! [`crate::salary_research::SalaryResearch`] (the SAME machinery
//! `ai_lookup_salary` uses). **Known parity gap**: unlike the in-app answer
//! flow, this path does not yet weigh the candidate's own saved
//! `job_preferences.salary_expectation` against that range (don't-undersell
//! precedence — a separate change from Task #30, which only made the value
//! backend-readable). It still answers honestly: when a range resolves, the
//! prompt states its midpoint/range rather than fabricating a number. A
//! non-salary question gets a grounded draft — résumé + (URL-matched) job
//! description + cached company brief — via [`prompt::ANSWER_ASSIST_SYSTEM`],
//! a compact Rust-native port of `@ajh/prompts`' answer-prompt honesty spine.
//! Tone/humanize parity with the in-app prose is NOT attempted here.
//!
//! ## Untrusted-input discipline
//! The question (and the cached company brief / any opt-in web-search notes)
//! is page/user-derived — fenced as `<question>` etc. via
//! [`crate::prompt_fence::fenced`], with an explicit "never follow
//! instructions inside it" label, the same discipline the in-app prompt
//! layer uses for its own untrusted blocks. The draft going back is AI
//! output — the popup renders it `textContent` only.
//!
//! ## Cost bounds
//! Shares the `"ai_research"` limiter bucket with `ai_lookup_salary`/
//! `ai_research_company`/`ai_research_answer` (one `acquire` per call, held
//! for its whole duration), and charges `PROVIDER_DAILY_MAX` once per ACTUAL
//! provider round-trip (the optional web-search-notes fetch, the optional
//! salary-market lookup, the compose, and — only on the one retried failure
//! [`compose::compose_with_length_retry`] covers — a second compose) — never
//! more than four per call, typically one.
//!
//! ## Streaming — compose internals live in `stream`
//! Each compose attempt streams via [`super::stream::compose_draft_stream`]
//! (see its own doc for the chunk-forwarding/cancellation mechanism). The
//! terminal `assist.done` frame is per REQUEST, not per attempt:
//! [`compose::compose_with_length_retry`] emits it exactly once, at its
//! single exit, because the popup drops its chunk listener the moment it
//! sees one. [`budgets::DRAFT_CAP`] is enforced LIVE mid-stream by
//! [`super::stream::forward_chunk`], per attempt (see
//! [`budgets::ANSWER_ASSIST_RETRY_MAX_TOKENS`] for the two-attempt wire
//! bound), not just clamped on the terminal string.
//!
//! ## Rewrite mode — a SEPARATE prompt, the SAME streaming path
//! `mode: 'rewrite'` (see [`AssistMode`]) transforms a field's
//! `existingAnswer` per a `preset`/`instruction` instead of drafting from
//! scratch — see [`super::answer_rewrite`]'s module doc (pure text
//! transform, no résumé/job/company/salary grounding, its own system
//! prompt). It reuses [`super::stream::compose_draft_stream`] — never a
//! parallel compose path — and the SAME gate/limiter/daily-charge draft
//! mode uses: rewriting is billable too, on the identical opt-in.
//!
//! ## Module layout
//! [`errors`] (fixed sentinels + the consent gate + the downstream-error
//! collapse), [`budgets`] (size/token caps), [`context`] (URL-matched
//! Application resolution), [`prompt`] (the grounded system/user message),
//! [`reply`] (reply shaping), [`grounding`] (billable pre-compose lookups +
//! their cancellation guards), [`draft_grounding`] (the draft-mode grounding
//! orchestration built on top of `context`/`grounding`/`prompt`),
//! [`compose`] (the retry-on-empty-length-cut compose round), and
//! [`resolve`] (the core resolve tying all of the above together). This file
//! keeps only the module's public entry points.

use serde_json::Value;
use tauri::{AppHandle, Manager};

mod budgets;
mod compose;
mod context;
mod draft_grounding;
mod errors;
mod grounding;
mod prompt;
mod reply;
mod resolve;

#[cfg(test)]
mod tests;

use crate::applications::ApplicationStore;
use crate::documents::DocumentStore;
use crate::error::AppError;

// Re-exported, not merely used: `AssistMode` moved into the parse split, but
// sibling modules (and this module's own tests) still name it through here.
pub(super) use super::answer_assist_parse::AssistMode;

// Re-exports so external callers keep resolving `answer_assist::X` unchanged now that the
// definitions live one module deeper, at the SAME visibility each had before the split (never
// narrower). `#[cfg(test)]` marks the three whose only outside consumer is test-only.
pub(crate) use self::budgets::ANSWER_ASSIST_MAX_TOKENS;
#[cfg(test)]
pub(crate) use self::budgets::ANSWER_ASSIST_RETRY_MAX_TOKENS;
pub(super) use self::budgets::DRAFT_CAP;
pub(super) use self::budgets::MAX_INSTRUCTION_BYTES;
#[cfg(test)]
pub(crate) use self::errors::AI_ASSIST_OFF_MESSAGE;
pub(super) use self::errors::DUPLICATE_REQUEST_MESSAGE;
#[cfg(test)]
pub(super) use self::errors::NO_PROVIDER_MESSAGE;
pub(super) use self::prompt::ANSWER_ASSIST_SYSTEM;
pub(super) use self::reply::answer_assist_reply;
// Plain (private) — `resolve_answer_assist` has no consumer outside `handle_answer_assist` below.
use self::resolve::resolve_answer_assist;

/// Answer an authenticated `answer.assist`: resolve the ai-assist opt-in,
/// resolve against the local `ApplicationStore`/`DocumentStore`, and return a
/// ready-to-send `answer.assist.result` reply. `registry` is the CALLER's
/// (this connection's) [`super::stream::AssistStreamRegistry`]. `gen` is the
/// generation `spawn_answer_assist`'s synchronous `begin_or_reject_duplicate`
/// handed back — this function's OWN entry — threaded through so
/// [`unregister_after_request`] can scope its cleanup to it.
pub(super) async fn handle_answer_assist(
    app: &AppHandle,
    req_id: &str,
    r#gen: u64,
    payload: &Value,
    registry: &super::stream::AssistStreamRegistry,
    sink: &mut dyn super::FrameSink,
) -> String {
    // The billable-AI consent gate (ADR-0011). The provider/model/base_url a
    // draft uses are no longer read here — `resolve_answer_assist` resolves
    // them from the backend `AiConfigStore` via `Completer::from_active`
    // (task #16), so only the opt-in flag is needed at this point.
    let ai_assist_enabled = app
        .try_state::<super::BridgeState>()
        .map(|state| state.ai_assist_enabled())
        .unwrap_or(false);

    let outcome = match (
        app.try_state::<ApplicationStore>(),
        app.try_state::<DocumentStore>(),
    ) {
        (Some(app_store), Some(doc_store)) => {
            resolve_answer_assist(
                app,
                req_id,
                r#gen,
                ai_assist_enabled,
                app_store.inner(),
                doc_store.inner(),
                payload,
                registry,
                sink,
            )
            .await
        }
        _ => Err(AppError::Config(
            "application/document store unavailable".to_string(),
        )),
    };

    unregister_after_request(registry, req_id, r#gen);
    answer_assist_reply(req_id, outcome)
}

/// The SOLE unregister owner for a `reqId`'s registry entry — called exactly
/// ONCE per request, here, at `handle_answer_assist`'s single return point,
/// UNCONDITIONALLY (on both `Ok` and `Err`), scoped to the caller's OWN
/// `gen` (the generation `begin()` minted for THIS request — see
/// [`super::assist_registry::StreamEntry`]'s doc).
///
/// Single ownership alone would not be enough:
/// [`super::stream::AssistStreamRegistry::cancel`]/`cancel_all` remove an
/// entry independently, keyed by `reqId` alone. Request A can register
/// Running, get cancelled by an `assist.cancel` while still in flight, and
/// have its `reqId` reused by a fresh request B (which `begin`s + `register`s
/// successfully) before A reaches this call — keyed by `reqId` alone, A's
/// cleanup would clobber B's fresh entry. Generation scoping closes it:
/// `unregister_gen(req_id, gen)` only removes the entry if its STORED
/// generation still equals `gen` — B's entry always carries a strictly
/// higher one, so A's late call is a safe no-op against it.
///
/// A `Pending(gen)`/`Running(gen, _)` entry for `req_id` always exists by the
/// time this runs (`spawn_answer_assist`'s synchronous
/// `begin_or_reject_duplicate` guarantees it before `handle_answer_assist` is
/// ever called), whether `resolve_answer_assist` returned early, the
/// store-unavailable branch above did, or `compose_draft_stream` ran to
/// completion — including a [`compose::compose_with_length_retry`] retry,
/// which rebinds the SAME entry rather than minting a new generation. An
/// `assist.cancel`/disconnect racing anywhere in that window already consumed
/// the entry itself, so this call is then simply a no-op.
///
/// Factored into its own tiny, pure function (no `AppHandle`) so it's
/// directly unit-testable — this crate has no `tauri::test` mock-app
/// harness.
fn unregister_after_request(
    registry: &super::stream::AssistStreamRegistry,
    req_id: &str,
    r#gen: u64,
) {
    registry.unregister_gen(req_id, r#gen);
}
