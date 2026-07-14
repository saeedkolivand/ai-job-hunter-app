//! "Save my answers from this page" (`answers.save` → `answers.result`) —
//! append newly-captured application-form answers onto the matched
//! Application's answer list. Split out of `mod.rs` per the R8 LOC cap
//! (mirrors `status_update.rs`'s module split); `resolve_*`/`handle_*` pure/
//! impure split mirrors `resolve_applied_check`/`handle_applied_check` and
//! `resolve_status_update`/`handle_status_update`.
//!
//! **Consent-gate boundary**: this verb WRITES freshly-captured page-derived
//! text into the local store, so — unlike `applied.check`/`status.update`
//! (read-only / an exact-match write to the user's OWN existing metadata, no
//! fresh page content) — it rides the SAME assisted-autofill opt-in as
//! `profile.get`/`fill` (`BridgeState::autofill_enabled`): capture and fill
//! are the two directions of the one PII-adjacent consent gate (extension
//! roadmap PR-5, plan decision 4).
//!
//! **Never auto-creates**: a `url` with no matching Application is a fixed
//! refusal telling the user to import the job first — this verb only
//! appends onto an existing pursuit, exactly like `status.update` never
//! creates a row for a `saved → applied` click.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::msg;
use crate::ai_generations::ApplicationAnswer;
use crate::applications::{normalize_job_url, ApplicationStore};
use crate::error::{AppError, AppResult};

/// Per-question / per-answer byte caps, char-boundary safe (mirrors
/// `applications::clamp_job_description`'s discipline) — untrusted
/// page-derived text is clamped at this store boundary, never dropped
/// wholesale. A question/label is short (a form label); an answer can run to
/// a paragraph but not a full essay.
const MAX_QUESTION_BYTES: usize = 1_000;
const MAX_ANSWER_BYTES: usize = 8_000;

/// Hard cap on the number of `{question, answer}` entries a single
/// `answers.save` call may carry — a pathological page (or a buggy/hostile
/// collector) can't force an unbounded write; extra entries are silently
/// dropped, not rejected (mirrors the `MAX_EXTRA_LINKS` cap-not-reject style
/// in `mod.rs`).
const MAX_ANSWERS_PER_CALL: usize = 50;

/// Clamp `s` to at most `max` bytes, cutting on a UTF-8 char boundary so the
/// stored text is always valid UTF-8. Truncate (never reject) — same
/// discipline as `applications::clamp_job_description`, duplicated here as a
/// tiny pure helper rather than exported cross-module (that cap is a
/// distinct constant/concern owned by `applications`).
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

/// The `answers.save` outcome — see [`msg::ANSWERS_SAVE`] docs. `title`/
/// `company` ride the WIRE reply (unlike `status.update`'s `StatusUpdateOk`,
/// which keeps them notification-only): the handler already loaded the
/// Application row for this verb, so surfacing them here is the smaller
/// change than threading the popup's separately-fetched `applied.check`
/// state through to this confirmation — see the PR-5 handoff.
#[derive(Debug)]
pub(super) struct AnswersSaveOk {
    pub(super) application_id: String,
    /// Newly-added count (never overwrites an existing answer).
    pub(super) saved: usize,
    /// Dedup-dropped count (already present, or blank after clamping).
    pub(super) skipped: usize,
    pub(super) title: Option<String>,
    pub(super) company: Option<String>,
}

/// Build the `answers.save` reply. Like `status_result_reply`, this verb's
/// errors ARE user-facing (a deliberate click, not a passive background
/// check) — the popup must render the `error` text, never fold it away.
pub(super) fn answers_result_reply(req_id: &str, outcome: AppResult<AnswersSaveOk>) -> String {
    let payload = match outcome {
        Ok(ok) => {
            let mut obj = serde_json::Map::new();
            obj.insert("ok".to_string(), json!(true));
            obj.insert("applicationId".to_string(), json!(ok.application_id));
            obj.insert("saved".to_string(), json!(ok.saved));
            obj.insert("skipped".to_string(), json!(ok.skipped));
            if let Some(t) = ok.title {
                obj.insert("title".to_string(), json!(t));
            }
            if let Some(c) = ok.company {
                obj.insert("company".to_string(), json!(c));
            }
            Value::Object(obj)
        }
        // Wire-error discipline: fixed sentinel text only (no dynamic/path/PII
        // content) — detailed context belongs in the desktop log, not on the wire.
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    };
    json!({
        "type": msg::ANSWERS_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Parse + clamp the incoming `answers` array off the payload, then cap at
/// [`MAX_ANSWERS_PER_CALL`] entries. Returns `(capped_list, raw_len)` where
/// `raw_len` is the count of well-formed entries BEFORE the per-call cap —
/// the caller derives `skipped` from it so an overflow past
/// `MAX_ANSWERS_PER_CALL` is counted as skipped instead of vanishing
/// silently (the cap used to apply via `.take()` before this count was
/// taken, so entries beyond it appeared in neither `saved` nor `skipped`).
/// A malformed entry (missing/non-string `question`/`answer`) is dropped,
/// not rejected — a blank question or answer (after trimming) is dropped
/// too, since the collector should never send one but the store boundary
/// re-validates independently rather than trusting the page-derived input.
/// Neither of these drops counts toward `raw_len`/`skipped`: they were never
/// well-formed captures to begin with.
fn parse_answers(payload: &Value) -> (Vec<ApplicationAnswer>, usize) {
    let well_formed: Vec<ApplicationAnswer> = payload
        .get("answers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let question = entry.get("question")?.as_str()?.trim();
                    let answer = entry.get("answer")?.as_str()?.trim();
                    if question.is_empty() || answer.is_empty() {
                        return None;
                    }
                    Some(ApplicationAnswer {
                        // Ignored on write — `ApplicationStore::merge_answers`
                        // assigns a fresh id to every newly-added answer.
                        id: String::new(),
                        question: clamp_bytes(question.to_string(), MAX_QUESTION_BYTES),
                        answer: clamp_bytes(answer.to_string(), MAX_ANSWER_BYTES),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let raw_len = well_formed.len();
    let mut capped = well_formed;
    capped.truncate(MAX_ANSWERS_PER_CALL);
    (capped, raw_len)
}

/// Core `answers.save`: gate on the autofill opt-in (refusal mirrors
/// `resolve_profile`'s fixed sentinel — see [`super::AUTOFILL_OFF_MESSAGE`]),
/// normalize + match `url` the SAME way `resolve_applied_check`/
/// `resolve_status_update` do, then merge the (clamped, capped) captured
/// answers onto the matched Application via
/// [`ApplicationStore::merge_answers`] — NEVER `upsert_internal`'s meta path
/// (`ApplicationStore::merge_answers_by_question`), which lets `incoming`
/// win for a matching question; that's right for an in-app rewrite but wrong
/// here, where a stray re-capture must never clobber an answer the user
/// already reviewed. No match → a fixed sentinel telling the user to import
/// the job first; this verb never auto-creates.
pub(super) fn resolve_answers_save(
    store: &ApplicationStore,
    autofill_enabled: bool,
    payload: &Value,
) -> AppResult<AnswersSaveOk> {
    if !autofill_enabled {
        return Err(AppError::Validation(
            super::AUTOFILL_OFF_MESSAGE.to_string(),
        ));
    }

    let url = payload
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }

    let canonical = crate::scraping::scrape_url::canonical_job_url(&url);
    let effective_url = canonical.as_deref().unwrap_or(url.as_str());
    let normalized = normalize_job_url(effective_url);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "url is not a valid http(s) URL".to_string(),
        ));
    }

    let app = store.find_by_job_url(&normalized).ok_or_else(|| {
        AppError::Validation(
            "couldn't find a saved job for this page — import it first".to_string(),
        )
    })?;

    let (incoming, raw_len) = parse_answers(payload);
    let saved = store.merge_answers(&app.id, incoming).map_err(|e| {
        // Wire-error discipline: never let a raw store error (path/SQL detail)
        // reach `answers_result_reply` — log it, reply a fixed sentinel.
        log::warn!("[extension_bridge] answers.save store error: {e}");
        AppError::Storage("could not save these answers".to_string())
    })?;

    Ok(AnswersSaveOk {
        application_id: app.id,
        saved,
        // `raw_len` (well-formed entries BEFORE the per-call cap) so an
        // over-cap capture and a dedup-drop both land in `skipped` — see
        // `parse_answers`.
        skipped: raw_len.saturating_sub(saved),
        title: (!app.title.trim().is_empty()).then_some(app.title),
        company: (!app.company.trim().is_empty()).then_some(app.company),
    })
}

/// Answer an authenticated `answers.save`: resolve against the local
/// `ApplicationStore` (gated on the autofill opt-in) and return a
/// ready-to-send `answers.result` reply. No notification/status-event tail —
/// unlike `status.update`, this verb never touches status, and captured page
/// text is never echoed into a notification (the wire reply above already
/// carries the only page-adjacent text the popup renders — fixed counts plus
/// the already-trusted title/company snapshot).
pub(super) fn handle_answers_save(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let enabled = app
        .try_state::<super::BridgeState>()
        .map(|s| s.autofill_enabled())
        .unwrap_or(false);
    let outcome = app
        .try_state::<ApplicationStore>()
        .ok_or_else(|| AppError::Config("applications store unavailable".to_string()))
        .and_then(|store| resolve_answers_save(store.inner(), enabled, payload));
    answers_result_reply(req_id, outcome)
}
