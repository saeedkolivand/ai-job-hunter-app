//! The real `Webview::on_message` round trip, the irreversible-confirm ceremony, and the one
//! `dispatch` fn that routes a policy row through them — split out of `agent_call.rs` under the
//! R8 LOC cap.

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeError, InvokeResponse, InvokeResponseBody};
use tauri::webview::InvokeRequest;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

use super::super::agent_cli::policy::{Effect, ProofSource};
use super::dispatch_plan::{plan, Dispatch};
use super::policy_lookup::{find_policy, namespace_suggestion};
use super::proof;
use super::refusal::Refusal;
use super::reshape::{
    reshape_reply, restore_local_only_contact_fields, take_list_page_args,
    unfence_named_fields_recursive, CONTACT_PROFILE_SET_COMMAND,
};

// ── Dispatch ─────────────────────────────────────────────────────────────

/// What `Webview::on_message`'s callback handed back, translated into
/// [`dispatch_direct`]'s own vocabulary — split out so the translation
/// itself (`classify_response`) is a PURE fn, unit-testable without a live
/// `AppHandle` (this crate has no `tauri::test` mock-app harness; see
/// `documents::embedding`'s doc for the same constraint elsewhere).
pub(super) enum InvokeOutcome {
    /// The command body ran and returned its success payload.
    Success(Value),
    /// `InvokeResponse::Err` (HIGH fix — security review): the command body
    /// either legitimately ran and returned a typed `Err` (e.g.
    /// `documents_export_document` failing validation), OR Tauri rejected
    /// the call before the body ever ran at all — a missing/mistyped arg
    /// (`applications_delete` called without `keepDocuments`), an ACL
    /// denial, or an unregistered command name. Both cases serialize to the
    /// SAME shape (a bare string — `AppError::serialize` and Tauri's own
    /// ACL-rejection string are wire-indistinguishable), so this crate
    /// cannot tell them apart from the response alone — but BOTH must never
    /// be reported as `dispatched: true`; see [`Refusal::InvokeError`].
    CommandErr(Value),
}

/// Pure: `InvokeResponse` → [`InvokeOutcome`]. No `AppHandle`, no I/O — every
/// branch of [`invoke_command`]'s previous behaviour (folding
/// `InvokeResponse::Err` into a successful `Ok(Value)`) is what let a
/// Tauri-level rejection report `dispatched: true` for a command whose body
/// never ran; this split is what makes that mapping directly testable.
pub(in crate::extension_bridge::agent_call) fn classify_response(
    response: InvokeResponse,
) -> InvokeOutcome {
    match response {
        InvokeResponse::Ok(InvokeResponseBody::Json(s)) => {
            InvokeOutcome::Success(serde_json::from_str(&s).unwrap_or(Value::Null))
        }
        // No command on the Read-effect rows returns a raw byte body today,
        // but degrade rather than drop it if one ever does.
        InvokeResponse::Ok(InvokeResponseBody::Raw(bytes)) => InvokeOutcome::Success(json!(bytes)),
        InvokeResponse::Err(InvokeError(v)) => InvokeOutcome::CommandErr(v),
    }
}

/// Drive one `Webview::on_message` round trip for `command`. `input` becomes
/// the invoke body verbatim — exactly what the renderer's own
/// `invoke(cmd, args)` sends (Tauri deserializes each top-level key into the
/// matching arg by name), so `--input '{"jobId":"..."}'` reaches the command
/// the same way a UI click would.
pub(super) async fn invoke_command(
    app: &AppHandle,
    command: &str,
    input: Value,
) -> AppResult<InvokeOutcome> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::Config("main window unavailable".to_string()))?;
    let url = window
        .url()
        .map_err(|e| AppError::Message(format!("could not read the window url: {e}")))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    let request = InvokeRequest {
        cmd: command.to_string(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url,
        body: input.into(),
        headers: Default::default(),
        invoke_key: app.invoke_key().to_string(),
    };
    window.on_message(
        request,
        Box::new(move |_webview, _cmd, response, _callback, _error| {
            let _ = tx.send(response);
        }),
    );
    let response = rx
        .await
        .map_err(|_| AppError::Message("command dispatch never replied".to_string()))?;
    Ok(classify_response(response))
}

/// [`Refusal::InvokeError`]'s detail text — a bare JSON string (the common
/// case — both `AppError` and Tauri's own ACL-rejection serialize as one)
/// renders unquoted; anything else (rare — a future non-string command
/// error type) falls back to its JSON form rather than panicking.
pub(in crate::extension_bridge::agent_call) fn invoke_error_detail(v: &Value) -> String {
    v.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| v.to_string())
}

/// The impure half of the [`CONTACT_PROFILE_SET_COMMAND`] photo-restore:
/// read the stored profile, or `None` when the store is unmanaged. Factored
/// out of `dispatch_direct` (round-3 review, P-r3-AC-R3-F1) so it composes
/// with the pure `restore_local_only_contact_fields` against a REAL
/// `ContactProfileStore` in a test — this crate has no `tauri::test` mock
/// app, so a call site taking `&AppHandle` directly can't be exercised at
/// all; `commands/contact_profile.rs`'s own `_inner`/`Option<&Store>` split
/// documents the same gap and uses the same shape.
///
/// Reads through [`ContactProfileStore::try_get`], not `get` (agent-cli
/// review, P-r1-AC-R4-F3): `get` degrades a locked/busy read or a corrupt
/// stored row to `ContactProfile::default()`, which is indistinguishable
/// from "nothing stored" to `restore_local_only_contact_fields` and would
/// reproduce the round-1 CRITICAL (a whole-row-replace deleting `photo`)
/// on every such read. A real read failure refuses the dispatch instead.
pub(in crate::extension_bridge::agent_call) fn stored_profile_value(
    store: Option<&crate::contact_profile::ContactProfileStore>,
) -> Result<Option<Value>, Refusal> {
    let Some(store) = store else {
        return Ok(None);
    };
    let profile = store
        .try_get()
        .map_err(|e| Refusal::StateUnreadable(e.to_string()))?;
    Ok(serde_json::to_value(&profile).ok())
}

/// Invoke a command for real: take this layer's own paging arguments off
/// `input` ([`take_list_page_args`]), strip any fence wrapper the caller
/// echoed back into it ([`unfence_named_fields_recursive`]), dispatch, then
/// fence any scraped text in the response ([`fence::fence_scraped_fields`]), page it
/// ([`reshape::paginate_list_reply`]) and re-encode any raw byte field
/// ([`reshape::base64_byte_fields`]). Called directly for a `Read`/`Reversible`
/// row, and again at [`dispatch_irreversible_confirmed`]'s tail for a confirmed
/// `Irreversible` one — the ONE real-invocation chokepoint every dispatched
/// row funnels through, never a second copy of any of those steps.
///
/// The response side of that is [`reshape_reply`], which owns the ORDER the
/// three steps run in — extracted so the order is one pure, directly
/// testable fn rather than three statements whose sequence nothing pins.
async fn dispatch_direct(
    app: &AppHandle,
    command: &str,
    mut input: Value,
) -> Result<Value, Refusal> {
    let page_args = take_list_page_args(command, &mut input)?;
    unfence_named_fields_recursive(&mut input);
    // The CRITICAL fix for issue #1180's round-1 review, generalised in
    // round 2 (P-r2-R2-F2): a `contact_profile_set` whose payload omits a
    // local-only field (the only shape a `contact_profile_get` caller can
    // ever produce, since that reply already strips every such field) must
    // not silently delete it on this whole-row-replace write — for `photo`
    // today, and for whatever field is added to `ContactProfile` next
    // without a matching `CONTACT_PROFILE_AGENT_FIELDS` entry. Reads current
    // app state here via `stored_profile_value` (the impure half) and hands
    // the whole stored profile to the pure `restore_local_only_contact_fields`,
    // which does the actual merge.
    if command == CONTACT_PROFILE_SET_COMMAND {
        let stored_profile = stored_profile_value(
            app.try_state::<crate::contact_profile::ContactProfileStore>()
                .as_deref(),
        )?;
        restore_local_only_contact_fields(command, &mut input, stored_profile.as_ref());
    }
    let outcome = invoke_command(app, command, input)
        .await
        .map_err(|e| Refusal::DispatchFailed(e.to_string()))?;
    let data = match outcome {
        InvokeOutcome::Success(v) => v,
        // The command body either legitimately ran and returned a typed
        // `Err`, or Tauri rejected the call before the body ever ran (bad
        // args, an ACL denial, an unregistered command) — see
        // `Refusal::InvokeError`'s own doc for why these two are
        // wire-indistinguishable and both refuse rather than dispatch.
        InvokeOutcome::CommandErr(v) => return Err(Refusal::InvokeError(invoke_error_detail(&v))),
    };
    let data = reshape_reply(command, data, page_args);
    // A3-r1-AC-7 — a direct read of the grace window's own read command is exactly the recovery
    // path a `ConfirmationRequired` refusal's hint sends a caller to; refreshing here closes the
    // double-drift gap a single confirmation_required-time snapshot can't (see `proof::
    // refresh_from_read`'s own doc). A no-op for every command but that one.
    proof::refresh_from_read(command, &data);
    Ok(data)
}

/// The whole decision [`dispatch_irreversible_confirmed`] makes, with the
/// `AppHandle` factored out — the impure wrapper below only resolves the
/// proof and hands the result here, so the ceremony's two refusal paths are
/// directly testable (the crate has no Tauri mock, so nothing taking a
/// concrete `&AppHandle` can be).
///
/// `run` is called at most ONCE and ONLY on an accepted `confirm` — never
/// before the comparison, which is the property the tests mutation-check: a
/// version that ran first and compared after would dispatch an
/// irreversible command on a wrong `confirm`. It returns whatever the
/// caller's own run step produces (the async wrapper returns the UNAWAITED
/// future, so this core stays sync and pure).
///
/// The comparison itself is [`proof::accepted`] (issue #1162), not a bare `==`: an exact match on
/// the FRESH `resolved` value is still the ordinary case, but for a [`proof::grace_window_key`]-
/// eligible `source` (today, ONLY `ai_spend_summary`-backed rows — security review round A3-r1,
/// AC-1/SEC-1 CRITICAL), `confirm` may also match a snapshot [`super::dispatch`]/[`dispatch_direct`]
/// recorded, provided that snapshot's grace window hasn't closed — see that fn's own doc for why a
/// spend-based proof can legitimately move between disclosure and confirm. Every other row's
/// `source` has no grace window at all: only the exact fresh value is ever accepted.
pub(in crate::extension_bridge::agent_call) fn confirm_and_run<T>(
    source: ProofSource,
    resolved: Option<String>,
    confirm: &str,
    run: impl FnOnce() -> T,
) -> Result<T, Refusal> {
    let expected = resolved.ok_or(Refusal::ProofUnavailable)?;
    match proof::accepted(proof::grace_window_key(source), &expected, confirm) {
        Ok(()) => Ok(run()),
        Err(proof::SnapshotOutcome::Mismatch) => {
            Err(Refusal::ConfirmationMismatch { moved: false })
        }
        Err(proof::SnapshotOutcome::Expired) => Err(Refusal::ConfirmationMismatch { moved: true }),
    }
}

/// Dispatch an `Irreversible` row whose `confirm` is already known to be present (the caller —
/// [`dispatch`] — only reaches here via [`Dispatch::Confirmed`]): resolve the expected value
/// FRESH via [`proof::resolve`] and only then run the real command via [`confirm_and_run`].
///
/// ponytail: the proof binds the TARGET record only, never a caller-input flag — e.g.
/// `applications_delete`'s destructive `keepDocuments: false` cascade shares one proof string
/// with `keepDocuments: true` (A1-r1-SEC-2 MEDIUM, deferred to issue #1160: binding the proof to
/// a destructive-branch flag is a confirm-ceremony wire-format change).
async fn dispatch_irreversible_confirmed(
    app: &AppHandle,
    command: &str,
    input: Value,
    source: ProofSource,
    confirm: &str,
) -> Result<Value, Refusal> {
    let resolved = proof::resolve(app, source, &input).await;
    confirm_and_run(source, resolved, confirm, || {
        dispatch_direct(app, command, input)
    })?
    .await
}

pub(super) async fn dispatch(
    app: &AppHandle,
    namespace: &str,
    command: &str,
    input: Value,
    confirm: Option<&str>,
) -> Result<Value, Refusal> {
    let entry = find_policy(namespace, command)
        .ok_or_else(|| Refusal::UnknownCommand(namespace_suggestion(command)))?;
    match plan(entry, command, &input, confirm) {
        Ok(Dispatch::Direct) => dispatch_direct(app, command, input).await,
        Ok(Dispatch::Confirmed { source, confirm }) => {
            dispatch_irreversible_confirmed(app, command, input, source, confirm).await
        }
        // Issue #1162 — snapshot the CURRENT proof value right NOW, at the moment this
        // `confirmation_required` refusal is issued: this is the earliest point a grace window
        // can start, and doing it here (rather than lazily on the retry) means a caller that
        // reads the disclosed field and pastes it straight back is comparing against a value
        // this process itself resolved, never one it could have influenced. Best-effort — a
        // `None` (the target record doesn't exist yet, or its own read failed) changes nothing
        // about the refusal the caller already gets; it just means no snapshot lands. Only for a
        // [`proof::grace_window_key`]-eligible `source` (security review round A3-r1, AC-1/SEC-1
        // CRITICAL) — every per-target row never gets a snapshot at all, so it can never be
        // satisfied by anything but the fresh, just-resolved value.
        Err(Refusal::ConfirmationRequired(hint)) => {
            if let Effect::Irreversible(source) = entry.effect {
                if let Some(key) = proof::grace_window_key(source) {
                    if let Some(value) = proof::resolve(app, source, &input).await {
                        proof::remember(key, value);
                    }
                }
            }
            Err(Refusal::ConfirmationRequired(hint))
        }
        Err(other) => Err(other),
    }
}
