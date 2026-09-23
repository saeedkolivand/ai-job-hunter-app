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

use super::{msg, BridgeState};
use crate::ai_generations::ApplicationAnswer;
use crate::applications::{normalize_job_url, ApplicationStore};
use crate::error::{AppError, AppResult};

impl BridgeState {
    /// Read both `answers.save` consent gates — the assisted-autofill opt-in
    /// (`autofill_enabled`) and the AUTO-only `saveAnswersOnSubmit` opt-in
    /// (`save_answers_on_submit_enabled`) — and run `f` with them, all under ONE hold of
    /// `optin_write_lock`: the SAME lock every consent setter (`set_autofill_enabled`/
    /// `set_ai_assist`/`set_autotrack_enabled`/`set_save_answers_on_submit_enabled`) already
    /// shares.
    ///
    /// Closes a TOCTOU the plain "read `save_on_submit_enabled`, then merge" order left open
    /// (PR #1209 review): a `settings.set` disabling the switch could land between the check and
    /// [`crate::applications::ApplicationStore::merge_answers`], so an AUTO-flagged capture could
    /// still be persisted after the user turned the switch off. Holding the setters' own lock
    /// across `f` forces that `settings.set` to BLOCK until this whole check-and-merge finishes,
    /// rather than reaching for a second, verb-local lock (a second lock would need its own
    /// ordering argument against this one; reusing the existing one needs none).
    ///
    /// Safe against deadlock with the setters: they only ever touch `optin_write_lock` — never
    /// [`ApplicationStore`]'s own `conn` mutex, which `f`'s merge acquires internally — so
    /// `optin_write_lock → conn` is the only nesting order either side ever takes. No path
    /// acquires `conn` first and `optin_write_lock` second to invert it.
    pub(super) fn with_answers_save_consent_locked<T>(&self, f: impl FnOnce(bool, bool) -> T) -> T {
        let _guard = self.optin_write_lock.lock();
        f(
            self.autofill_enabled(),
            self.save_answers_on_submit_enabled(),
        )
    }
}

/// Per-question / per-answer byte caps, char-boundary safe (mirrors
/// `applications::clamp_job_description`'s discipline) — untrusted
/// page-derived text is clamped at this store boundary, never dropped
/// wholesale. A question/label is short (a form label); an answer can run to
/// a paragraph but not a full essay.
const MAX_QUESTION_BYTES: usize = 1_000;
const MAX_ANSWER_BYTES: usize = 8_000;

/// Refusal text for an AUTO-flagged `answers.save` (`auto: true`, the submit-watch injected
/// entry's synchronous capture) while the dedicated `saveAnswersOnSubmit` opt-in is off — a fixed
/// sentinel, mirrors `status_update::AUTOTRACK_OFF_MESSAGE`'s wording style.
const SAVE_ANSWERS_ON_SUBMIT_OFF_MESSAGE: &str =
    "Save answers on submit is off. Turn it on in AI Job Hunter → Settings → Browser extension.";

/// Refusal text when `auto` is PRESENT but not a JSON boolean — see
/// [`auto_flag_is_malformed`]'s doc for why this must be a hard refusal, never a silent downgrade.
const MALFORMED_AUTO_FLAG_MESSAGE: &str = "malformed answers.save request: auto must be a boolean";

/// Whether the `auto` field is PRESENT on the payload but not a JSON boolean (a string, number,
/// `null`, object, or array). A malformed `auto` must be a HARD refusal, never a silent downgrade
/// to "manual" via [`is_auto_answers_save`]'s `Value::as_bool().unwrap_or(false)`: a manual save
/// is gated ONLY on the (weaker) assisted-autofill opt-in, not on the dedicated
/// `saveAnswersOnSubmit` opt-in this verb's AUTO path exists to require — so a malformed `auto`
/// silently reading as `false` would let a would-be automated capture through on the wrong,
/// weaker consent class, defeating the very gate [`auto_save_refused`] exists to enforce. `auto`
/// absent is unaffected — byte-identical to today (still defaults to manual).
///
/// `status_update::is_auto_status_update` has the IDENTICAL `Value::as_bool().unwrap_or(false)`
/// shape and is NOT hardened by a sibling of this function here — pre-existing, out of scope for
/// this change.
pub(super) fn auto_flag_is_malformed(payload: &Value) -> bool {
    matches!(payload.get("auto"), Some(v) if !v.is_boolean())
}

/// The `auto` flag on an `answers.save` payload (default false when absent) — `true` marks the
/// AUTOMATED submit-time capture from the `saveAnswersOnSubmit` opt-in, as opposed to the
/// deliberate popup "Save my answers" click. Mirrors `status_update::is_auto_status_update`
/// exactly, one write verb over. Callers MUST check [`auto_flag_is_malformed`] first — this
/// function's own `unwrap_or(false)` treats a malformed value exactly like an absent one, which is
/// correct ONLY once the malformed case has already been refused upstream.
pub(super) fn is_auto_answers_save(payload: &Value) -> bool {
    payload
        .get("auto")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Whether an AUTO `answers.save` must be REFUSED: an auto-flagged save is honored only while the
/// dedicated `saveAnswersOnSubmit` opt-in is on — the decisive server-side boundary (PR4). A
/// non-auto (deliberate popup click) save is never refused here — it keeps its existing behaviour,
/// gated only by the pre-existing Autofill opt-in below. Mirrors
/// `status_update::auto_write_refused` exactly: the extension's own client-side check (whether it
/// even arms the submit-watch capture) is defense-in-depth only — this is the real gate.
pub(super) fn auto_save_refused(payload: &Value, save_on_submit_enabled: bool) -> bool {
    is_auto_answers_save(payload) && !save_on_submit_enabled
}

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
        log::warn!(
            "[extension_bridge] answers.save store error: {}",
            crate::observability::sanitize_reason(&e.to_string())
        );
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
    // Hard refusal, BEFORE any opt-in check (PR4 hardening): a present-but-non-boolean `auto`
    // must never silently degrade to "manual" — see `auto_flag_is_malformed`'s doc.
    if auto_flag_is_malformed(payload) {
        return answers_result_reply(
            req_id,
            Err(AppError::Validation(
                MALFORMED_AUTO_FLAG_MESSAGE.to_string(),
            )),
        );
    }
    let Some(state) = app.try_state::<BridgeState>() else {
        return answers_result_reply(
            req_id,
            Err(AppError::Config("bridge state unavailable".to_string())),
        );
    };
    let Some(store) = app.try_state::<ApplicationStore>() else {
        return answers_result_reply(
            req_id,
            Err(AppError::Config(
                "applications store unavailable".to_string(),
            )),
        );
    };
    // Consent check + merge run under ONE hold of `optin_write_lock` — see
    // `BridgeState::with_answers_save_consent_locked`'s doc for why this closes the TOCTOU a
    // separate read-then-merge left open.
    let outcome =
        state.with_answers_save_consent_locked(|autofill_enabled, save_on_submit_enabled| {
            // Defense-in-depth precedent (PR4, mirrors `status_update::auto_write_refused`): an
            // AUTO-flagged save (fired synchronously by the submit-watch injected entry, not a user
            // click) is honored ONLY while the dedicated `saveAnswersOnSubmit` opt-in is on. A
            // non-auto save is unaffected — it keeps today's byte-identical behaviour, gated only by
            // `autofill_enabled` below.
            if auto_save_refused(payload, save_on_submit_enabled) {
                return Err(AppError::Validation(
                    SAVE_ANSWERS_ON_SUBMIT_OFF_MESSAGE.to_string(),
                ));
            }
            resolve_answers_save(store.inner(), autofill_enabled, payload)
        });
    answers_result_reply(req_id, outcome)
}
