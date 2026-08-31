//! `agent.call` → `agent.call.result` — ADR-038 §2's generic dispatch tier
//! (`agent call <namespace>:<command> --input '<json>'`). [`Effect::Read`]
//! AND [`Effect::Reversible`] rows dispatch directly through
//! [`tauri::Webview::on_message`] (Phase 4) — the caller can undo either
//! through the app, which is what those two classes mean. An
//! [`Effect::Irreversible`] row dispatches only after a `--confirm` ceremony
//! (Phase 3, ADR-038 §4): a call with no `confirm` refuses with
//! [`Refusal::ConfirmationRequired`], naming WHICH other read surface the
//! proof value comes from and NEVER the value itself; a wrong `confirm`
//! refuses with [`Refusal::ConfirmationMismatch`], which likewise never
//! discloses the expected value. [`Effect::NotExposed`] always refuses.
//!
//! ## Dispatch mechanism (verified against the vendored tauri 2.11.5
//! source, not docs.rs — ADR-038's own "verified" note)
//! `Webview::on_message` is `pub`; every `InvokeRequest` field is `pub`;
//! `AppHandle::invoke_key` is `pub` and its own doc names this EXACT use
//! ("Gets the invoke key that must be referenced when using
//! `crate::webview::InvokeRequest`"). Driving it this way runs the REAL,
//! registered command body in the app's own process against its single
//! managed state — so `limits::Limiter`/`charge_provider_daily` (which live
//! INSIDE command bodies, never in a wrapper — `commands/ai/mod.rs`) still
//! apply exactly as they do for the renderer. No codegen, no second copy of
//! any command's logic, no call-the-Rust-fn-directly shortcut that would
//! bypass those limits. The SAME mechanism resolves an `Irreversible` row's
//! proof value too (`proof::resolve` dispatches its `read_command` through
//! this exact path) — never a second implementation of a command's logic.
//!
//! `url` is the running app's OWN "main" `WebviewWindow`'s CURRENT url
//! (`WebviewWindow::url()`), never a guessed/hardcoded literal —
//! `on_message`'s private `is_local_url` only compares scheme+domain against
//! the app's own protocol origin, so reading the real webview's real address
//! is what makes this genuinely mirror what the renderer itself sends, on
//! every platform and dev-vs-prod combination, rather than hardcoding one of
//! `tauri://localhost` / `https://tauri.localhost` and silently breaking on
//! the other. `invoke_key` is read fresh off `AppHandle::invoke_key()` on
//! every call and NEVER logged/echoed/returned — its own doc: "DO NOT expose
//! this key to third party scripts as might grant access to the backend
//! from external URLs and iframes."

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeError, InvokeResponse, InvokeResponseBody};
use tauri::webview::InvokeRequest;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

use super::agent_cli::policy::{Effect, PolicyEntry, POLICY};

mod proof;

// ── `<namespace>:<command>` ⇄ policy row (derived, never hand-typed twice) ─

/// Split a [`PolicyEntry::path`] (e.g. `"commands::jobs::jobs_list"`, always
/// `module::fn` — at least one `::`) into `(namespace, command)`. `command`
/// is the bare trailing segment — the wire `cmd` Tauri actually registers
/// (confirmed against the TS client, `invoke('jobs_list', ...)`, never the
/// qualified path); `namespace` is the segment immediately before it.
/// Uniform across every row's shape (`commands::ai::ai_generate`,
/// `export::commands::documents_export_document`, `updater::updater_check`)
/// with no per-module special-casing — the SAME derivation both parses a
/// CLI token's expected shape and looks a row up, never two copies.
fn split_path(path: &str) -> (&str, &str) {
    let mut segments = path.rsplit("::");
    let command = segments.next().unwrap_or(path);
    let namespace = segments.next().unwrap_or("");
    (namespace, command)
}

/// The one [`PolicyEntry`] whose derived `(namespace, command)` matches
/// EXACTLY — never a fuzzy/partial match (a typo'd namespace on an
/// otherwise-real command name refuses rather than silently dispatching:
/// `command` alone already uniquely identifies a row, since
/// `generate_handler!` requires globally-unique command names, so a
/// namespace mismatch can only mean the caller typed the wrong one).
fn find_policy(namespace: &str, command: &str) -> Option<&'static PolicyEntry> {
    POLICY
        .iter()
        .find(|entry| split_path(entry.path) == (namespace, command))
}

// ── Refusals — distinct sentinel + detail per cause, one reply builder ─────

/// Every reason dispatch never reached (or never completed)
/// `Webview::on_message`. One enum, one `sentinel`/`detail` pair per
/// variant, one reply builder below — collapsing two of these into one
/// sentinel is exactly the defect this repo's own `agent_cli` module doc
/// says has already been fixed twice on this surface.
enum Refusal {
    /// No policy row matches this `(namespace, command)` pair at all.
    UnknownCommand,
    /// [`Effect::NotExposed`] — deliberately unreachable; carries that row's
    /// own stored reason.
    NotExposed(&'static str),
    /// `agent.call` arrived over a connection whose handshake `Origin`
    /// wasn't the CLI's — same class as `msg::AGENT_QUERY`'s origin gate.
    OriginRefused,
    /// The shared `agent.query`/`agent.call` throttle bucket is empty.
    RateLimited,
    /// The row's own command dispatch (`Webview::on_message`) never produced
    /// a usable reply (no "main" webview, its url couldn't be read, or the
    /// responder never fired) — a framework-level failure, never the
    /// caller's input, so the carried string is always one of
    /// [`invoke_command`]'s own fixed messages, never an echo of `input`.
    DispatchFailed(String),
    /// [`Effect::Irreversible`] with no `confirm` supplied — exit 4 (see
    /// `agent_cli::exit_code_for_reply`), distinct from every other refusal
    /// here (all exit 2). Names WHICH read surface + field the proof comes
    /// from and NEVER the value itself (ADR-038 §4).
    ConfirmationRequired(String),
    /// A `confirm` value that did not match the freshly-resolved proof. The
    /// detail is a FIXED string, never the hint and never the expected
    /// value — a mismatch must not leak anything a caller couldn't already
    /// have gotten from the `ConfirmationRequired` refusal alone.
    ConfirmationMismatch,
    /// The proof value itself could not be resolved (the read it depends on
    /// failed, or the targeted record doesn't exist) — distinct from a
    /// wrong-value mismatch so a caller can tell "you guessed wrong" apart
    /// from "the thing you're trying to act on isn't there".
    ProofUnavailable,
}

const ERR_UNKNOWN_COMMAND: &str = "unknown_command";
const ERR_NOT_EXPOSED: &str = "not_exposed";
const ERR_CLI_ONLY: &str = "cli_only";
const ERR_RATE_LIMITED: &str = "rate_limited";
const ERR_DISPATCH_FAILED: &str = "dispatch_failed";
/// `pub(super)` — [`super::agent_cli::exit_code_for_reply`] matches on this
/// EXACT sentinel to special-case exit 4, never a second hand-typed copy of
/// the string.
pub(super) const ERR_CONFIRMATION_REQUIRED: &str = "confirmation_required";
const ERR_CONFIRMATION_MISMATCH: &str = "confirmation_mismatch";
const ERR_PROOF_UNAVAILABLE: &str = "proof_unavailable";

/// Fixed sentinel — mirrors `agent_read::CLI_ONLY_MESSAGE` for the identical
/// gate, applied to the generic tier's own wire type.
const CLI_ONLY_MESSAGE: &str = "agent.call is only available to the ajh-tauri agent CLI";

impl Refusal {
    fn sentinel(&self) -> &'static str {
        match self {
            Refusal::UnknownCommand => ERR_UNKNOWN_COMMAND,
            Refusal::NotExposed(_) => ERR_NOT_EXPOSED,
            Refusal::OriginRefused => ERR_CLI_ONLY,
            Refusal::RateLimited => ERR_RATE_LIMITED,
            Refusal::DispatchFailed(_) => ERR_DISPATCH_FAILED,
            Refusal::ConfirmationRequired(_) => ERR_CONFIRMATION_REQUIRED,
            Refusal::ConfirmationMismatch => ERR_CONFIRMATION_MISMATCH,
            Refusal::ProofUnavailable => ERR_PROOF_UNAVAILABLE,
        }
    }

    /// Human/agent-readable detail. [`Refusal::ConfirmationRequired`] and
    /// [`Refusal::ConfirmationMismatch`] never carry the proof VALUE — see
    /// each variant's own doc; this is the one place both are rendered, so
    /// it is also the one place that guarantee could be broken, hence the
    /// dedicated tests in `agent_call::tests`.
    fn detail(&self) -> String {
        match self {
            Refusal::UnknownCommand => {
                "no policy row matches this <namespace>:<command> — run `agent schema` for the \
                 curated tier, or see docs/knowledge/decision-records/adr-038-* for the full \
                 command table"
                    .to_string()
            }
            Refusal::NotExposed(reason) => format!("not exposed to any CLI tier: {reason}"),
            Refusal::OriginRefused => CLI_ONLY_MESSAGE.to_string(),
            Refusal::RateLimited => super::agent_read::THROTTLED_MESSAGE.to_string(),
            Refusal::DispatchFailed(detail) => detail.clone(),
            Refusal::ConfirmationRequired(hint) => hint.clone(),
            Refusal::ConfirmationMismatch => {
                "the confirm value did not match — it is never disclosed by this refusal; \
                 re-read the source named in a fresh confirmation_required refusal for this \
                 same command"
                    .to_string()
            }
            Refusal::ProofUnavailable => {
                "could not resolve a proof value for this target — the referenced record may \
                 not exist, or the read it depends on failed"
                    .to_string()
            }
        }
    }
}

/// `dispatched`, never `ok` (ADR-038 §5): ~47 commands signal failure INSIDE
/// their own Ok payload, so this dispatcher cannot know whether the
/// underlying operation succeeded — only whether it ran. `data` is the
/// command's payload verbatim (no PII redaction — ADR-038's amendment to
/// ADR-0005, scoped to this generic tier by the owner's explicit decision).
fn call_result_reply(
    req_id: &str,
    namespace: &str,
    command: &str,
    outcome: Result<Value, Refusal>,
) -> String {
    let payload = match outcome {
        Ok(data) => json!({
            "dispatched": true,
            "namespace": namespace,
            "command": command,
            "data": data,
        }),
        Err(refusal) => json!({
            "dispatched": false,
            "namespace": namespace,
            "command": command,
            "error": refusal.sentinel(),
            "detail": refusal.detail(),
        }),
    };
    json!({
        "type": super::msg::AGENT_CALL_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Reply for an `agent.call` arriving over a connection whose handshake
/// `Origin` wasn't `auth::AGENT_CLI_ORIGIN` — mirrors
/// `agent_read::origin_refused_reply` exactly, one wire type over.
pub(super) fn origin_refused_reply(req_id: &str, payload: &Value) -> String {
    let (namespace, command) = payload_target(payload);
    call_result_reply(req_id, namespace, command, Err(Refusal::OriginRefused))
}

pub(super) fn throttled_reply(req_id: &str, payload: &Value) -> String {
    let (namespace, command) = payload_target(payload);
    call_result_reply(req_id, namespace, command, Err(Refusal::RateLimited))
}

fn payload_target(payload: &Value) -> (&str, &str) {
    let namespace = payload
        .get("namespace")
        .and_then(Value::as_str)
        .unwrap_or("");
    let command = payload.get("command").and_then(Value::as_str).unwrap_or("");
    (namespace, command)
}

/// The throttle key `agent.call` draws from — reuses
/// `BridgeState::try_acquire_agent`'s EXISTING two buckets (never a second
/// throttle instance): `autopilot_best_matches` is the one Read-effect
/// command in [`POLICY`] that runs the SAME uncapped clustering pass
/// `agent_read`'s `best-matches` resource already rate-limits tightly, so it
/// maps into that resource's own bucket key — sharing state so a caller
/// cannot double an allowance by alternating tiers. Every other command
/// falls into the shared cheap bucket (any key other than `"best-matches"`
/// does, by `AgentQueryThrottle::try_acquire_at`'s own construction). Note:
/// this throttle admits the TARGET command's own frame only — an
/// `Irreversible` row's proof-resolution read (`proof::resolve`) dispatches
/// a SECOND, internal command and is not separately throttled; bounded to
/// exactly one extra read per confirm attempt, so left as-is rather than
/// adding a second bucket for a cost this small.
pub(super) fn throttle_key(command: &str) -> &str {
    if command == "autopilot_best_matches" {
        "best-matches"
    } else {
        command
    }
}

// ── Fencing scraped job-posting text (a different axis from the raw-data
// decision above — ADR-038's own amendment paragraph) ──────────────────────

/// Read-effect commands whose response can carry raw, third-party-authored
/// SCRAPED JOB TEXT — audited by hand against each command's own body
/// (mirrors `policy`'s own per-row audit discipline; NOT derived from
/// `Effect::Read` itself, since most Read rows carry nothing scraped at
/// all). Every entry is commented with the field this routes to
/// [`crate::prompt_fence::fenced`] — the SAME primitive, tag, and cap
/// `agent_read::fence_description` uses for the curated `job` resource, so a
/// scraped posting reads as untrusted DATA on every surface it reaches.
const FENCE_DESCRIPTION_COMMANDS: &[&str] = &[
    // Returns one `scraping::types::JobPosting` (or null) — `description` is
    // the full scraped posting body fetched on demand.
    "scrape_resolve_url",
    // Returns the ENTIRE live postings cache as a raw array — every element
    // carries its own scraped `description`.
    "scrape_list_postings",
];

/// Fence every `description` string [`FENCE_DESCRIPTION_COMMANDS`] can carry,
/// in place — recurses through the WHOLE response tree (MEDIUM fix —
/// security review), not just a top-level object/array: the two commands on
/// this list are audited against their shape TODAY, but a future response
/// wrapped in e.g. `{"postings": [...]}` must not silently skip this guard
/// just because it added one more layer of nesting.
fn fence_scraped_fields(command: &str, data: &mut Value) {
    if !FENCE_DESCRIPTION_COMMANDS.contains(&command) {
        return;
    }
    fence_description_recursive(data);
}

/// Walk every object/array in `value`, fencing a `description` STRING key
/// wherever one appears — see [`fence_scraped_fields`]'s doc for why this is
/// recursive rather than one level deep.
fn fence_description_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(desc) = map.get("description").and_then(Value::as_str) {
                let fenced =
                    crate::prompt_fence::fenced("job_posting", desc, crate::prompt_fence::JOB_CAP);
                map.insert("description".to_string(), json!(fenced));
            }
            for v in map.values_mut() {
                fence_description_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                fence_description_recursive(item);
            }
        }
        _ => {}
    }
}

// ── Dispatch ─────────────────────────────────────────────────────────────

/// Drive one `Webview::on_message` round trip for `command`. `input` becomes
/// the invoke body verbatim — exactly what the renderer's own
/// `invoke(cmd, args)` sends (Tauri deserializes each top-level key into the
/// matching arg by name), so `--input '{"jobId":"..."}'` reaches the command
/// the same way a UI click would.
async fn invoke_command(app: &AppHandle, command: &str, input: Value) -> AppResult<Value> {
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
    Ok(match response {
        InvokeResponse::Ok(InvokeResponseBody::Json(s)) => {
            serde_json::from_str(&s).unwrap_or(Value::Null)
        }
        // No command on the Read-effect rows returns a raw byte body today,
        // but degrade rather than drop it if one ever does.
        InvokeResponse::Ok(InvokeResponseBody::Raw(bytes)) => json!(bytes),
        InvokeResponse::Err(InvokeError(v)) => v,
    })
}

/// Dispatch a `Read`/`Reversible` row: invoke it for real, then fence any
/// scraped text in its response.
async fn dispatch_direct(app: &AppHandle, command: &str, input: Value) -> Result<Value, Refusal> {
    let mut data = invoke_command(app, command, input)
        .await
        .map_err(|e| Refusal::DispatchFailed(e.to_string()))?;
    fence_scraped_fields(command, &mut data);
    Ok(data)
}

/// Dispatch an `Irreversible` row whose `confirm` is already known to be
/// present (the caller — [`dispatch`] — has already run it through
/// [`dispatchable`]): resolve the expected value FRESH via [`proof::resolve`]
/// and only then run the real command.
async fn dispatch_irreversible_confirmed(
    app: &AppHandle,
    command: &str,
    input: Value,
    source: super::agent_cli::policy::ProofSource,
    confirm: &str,
) -> Result<Value, Refusal> {
    let expected = proof::resolve(app, source, &input)
        .await
        .ok_or(Refusal::ProofUnavailable)?;
    if confirm != expected {
        return Err(Refusal::ConfirmationMismatch);
    }
    dispatch_direct(app, command, input).await
}

/// Pure gate: does `effect` permit `dispatch` to ATTEMPT a real command
/// invocation at all, given whether a `confirm` value was supplied — never
/// mind whether that attempt then succeeds. `dispatch` below calls this as
/// its own FIRST decision (never a parallel/shadow copy of the same logic),
/// so `extension_bridge::test`'s exhaustive walk over every real `POLICY`
/// row (`agent_call_gate_matches_every_policy_rows_declared_effect`) proves
/// something about THIS production routing, not a second implementation
/// that could silently drift from it. `pub(super)` — reachable from
/// `extension_bridge::test`, a sibling of this module, for exactly that
/// test; `Effect` and `bool` are both cheap `Copy` so this needs no
/// `AppHandle` and no I/O.
pub(super) fn dispatchable(effect: Effect, has_confirm: bool) -> bool {
    match effect {
        Effect::NotExposed(_) => false,
        Effect::Read | Effect::Reversible => true,
        Effect::Irreversible(_) => has_confirm,
    }
}

async fn dispatch(
    app: &AppHandle,
    namespace: &str,
    command: &str,
    input: Value,
    confirm: Option<&str>,
) -> Result<Value, Refusal> {
    let entry = find_policy(namespace, command).ok_or(Refusal::UnknownCommand)?;
    if !dispatchable(entry.effect, confirm.is_some()) {
        return Err(match entry.effect {
            Effect::NotExposed(reason) => Refusal::NotExposed(reason),
            // `dispatchable` only refuses an `Irreversible` row when
            // `confirm` is `None` — the ONLY other way it refuses.
            Effect::Irreversible(source) => Refusal::ConfirmationRequired(proof::hint(source)),
            Effect::Read | Effect::Reversible => {
                unreachable!("dispatchable() is true for Read/Reversible")
            }
        });
    }
    match entry.effect {
        Effect::Read | Effect::Reversible => dispatch_direct(app, command, input).await,
        Effect::Irreversible(source) => {
            // `dispatchable` already proved `confirm.is_some()` to reach here.
            let confirm = confirm.expect("dispatchable() guarantees Some for Irreversible here");
            dispatch_irreversible_confirmed(app, command, input, source, confirm).await
        }
        Effect::NotExposed(_) => unreachable!("dispatchable() is false for NotExposed"),
    }
}

/// Answer an authenticated, throttle-admitted, origin-checked `agent.call`.
/// Never panics — [`dispatch`] degrades to a [`Refusal`] on every failure
/// path (unknown command, wrong effect, or the dispatch itself erroring).
/// Logs the command identity + whether it dispatched (MEDIUM fix — security
/// review: every other privileged bridge path leaves an observability
/// record, this one didn't) — NEVER `input`/`confirm`/the response `data`,
/// every one of which can carry PII or a résumé/cover-letter body.
pub(super) async fn handle_agent_call(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let (namespace, command) = {
        let (ns, cmd) = payload_target(payload);
        (ns.to_string(), cmd.to_string())
    };
    let input = payload.get("input").cloned().unwrap_or_else(|| json!({}));
    let confirm = payload.get("confirm").and_then(Value::as_str);

    let span = crate::observability::Span::begin(
        "agent_call",
        format!("namespace={namespace} command={command}"),
    );
    let outcome = dispatch(app, &namespace, &command, input, confirm).await;
    let dispatched = outcome.is_ok();
    let reply = call_result_reply(req_id, &namespace, &command, outcome);
    span.end_with(&format!("dispatched={dispatched}"), dispatched);
    reply
}

#[cfg(test)]
mod tests;
