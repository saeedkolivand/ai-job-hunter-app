//! The human-in-the-loop confirm gate — the safety core of the agent.
//!
//! Owns BOTH the confirmation bookkeeping ([`AgentGate`]/[`Decision`]) and the
//! suspend-and-execute mechanics for one Write tool call ([`resolve_write`]). The
//! controller loop (`super::controller`) calls into [`resolve_write`] for every
//! `ToolKind::Write` call it encounters; the turn-taking loop itself stays there.
//!
//! # Security invariants (verified by `tauri-security-reviewer`)
//!
//! - **Explicit approval for every side effect.** A [`ToolKind::Write`] tool call
//!   never executes on the model's say-so. [`resolve_write`] SUSPENDS the run, emits
//!   an `agent:step` of kind `confirm_request` carrying the exact tool + (clamped)
//!   args, and blocks on a [`oneshot`] until the user resolves it via
//!   `agent_confirm`. `Deny`, a timeout, and cancel ALL default to **NOT acting**.
//! - **Edited args may change CONTENT only — never routing/egress.** On
//!   [`Decision::ApproveEdited`] [`validate_edited_args`] re-validates the edited
//!   JSON against the tool's fixed schema (whitelist of declared keys) and
//!   re-asserts it carries no `provider`/`model`/`base_url`/`job_id` field
//!   ([`is_routing_egress_key`]). Those stay in the trusted [`ToolContext`] threaded
//!   from `agent_run`; a prompt-injected posting can steer the *content* the model
//!   proposes to write, never where a credentialed request is routed nor which
//!   job/application it mutates.
//! - **No arbitrary-fetch/email/shell/URL tool exists.** The whitelist is per-flow
//!   and every Write tool is app-INTERNAL (persist to a local store). The gate's
//!   whole point is that even these internal writes now require approval.
//! - **Prompt injection can REQUEST but never EXECUTE.** Hostile job/résumé text can
//!   make the model *ask* for a write; it can never satisfy the gate on the user's
//!   behalf — only a real `agent_confirm` IPC call (or a `resolve` in a test) can.
//! - **Display/edit fidelity.** Confirm-request args are clamped to the CALLED
//!   tool's own content cap ([`display_cap_for`]) — [`COVER_LETTER_CAP`] for
//!   `save_cover_letter`, [`SAVED_RESUME_CAP`] for `save_resume` — so the renderer
//!   shows/edits exactly what will be persisted: never a truncated preview, and
//!   never more than the tool will keep. An unknown tool falls back to the larger
//!   of the two, which preserves the never-show-less half of the guarantee.
//!
//! [`ToolKind::Write`]: super::tools::ToolKind
//! [`ToolContext`]: super::tools::ToolContext

use std::collections::HashMap;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::Value;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

use super::controller::{AgentEnv, AgentStep, AgentStepKind, ConfirmRequest};
use super::tools::{AgentTool, COVER_LETTER_CAP, SAVED_RESUME_CAP};

/// The user's verdict on one pending Write action, delivered from `agent_confirm`
/// back to the suspended controller loop over a [`oneshot`] channel. `Clone` is a
/// test convenience (retry a resolve); production moves it through the channel once.
#[derive(Debug, Clone)]
pub enum Decision {
    /// Run the Write handler with the model's ORIGINAL args, verbatim.
    Approve,
    /// Run the Write handler with user-EDITED args. [`validate_edited_args`]
    /// re-validates these against the tool schema and re-asserts they carry no
    /// routing/egress field before executing — edited args may change content only.
    ApproveEdited(Value),
    /// Do not act; push a "user declined" tool-result and continue the loop.
    Deny,
}

/// Managed Tauri state: the set of Write confirmations currently AWAITING a user
/// decision, keyed by `(jobId, callId)`. [`resolve_write`] [`register`]s a sender
/// before it suspends; `agent_confirm` [`resolve`]s it when the user answers;
/// [`resolve_write`] always [`remove`]s the entry once it resumes (via any branch —
/// approve/deny/timeout/cancel), so a resolved-or-abandoned call is never
/// double-delivered and the map can't leak entries.
///
/// The inner lock is only ever held for the duration of an insert/remove (never
/// across an `.await`), so there is no lock-across-suspend hazard.
///
/// [`register`]: AgentGate::register
/// [`resolve`]: AgentGate::resolve
/// [`remove`]: AgentGate::remove
#[derive(Default)]
pub struct AgentGate {
    pending: Mutex<HashMap<(String, String), oneshot::Sender<Decision>>>,
}

impl AgentGate {
    /// Record a pending confirmation and its wake-up channel. Called by
    /// [`resolve_write`] immediately before it suspends on the receiver.
    pub fn register(&self, job_id: String, call_id: String, tx: oneshot::Sender<Decision>) {
        self.pending.lock().insert((job_id, call_id), tx);
    }

    /// Deliver a user decision to a suspended run. Returns `true` when a pending
    /// call was found AND its waiter was still listening; `false` when there is no
    /// such pending call (already resolved / timed out / cancelled / unknown id) or
    /// the receiver was already dropped. Never panics — an unknown id is a benign
    /// `false`, which `agent_confirm` surfaces as `{ ok: false }`.
    pub fn resolve(&self, job_id: &str, call_id: &str, decision: Decision) -> bool {
        let tx = self
            .pending
            .lock()
            .remove(&(job_id.to_string(), call_id.to_string()));
        match tx {
            Some(tx) => tx.send(decision).is_ok(),
            None => false,
        }
    }

    /// Drop a pending entry unconditionally. Idempotent — removing an already-gone
    /// entry is a no-op. [`resolve_write`] calls this on EVERY resume branch so the
    /// map never retains a stale sender.
    pub fn remove(&self, job_id: &str, call_id: &str) {
        self.pending
            .lock()
            .remove(&(job_id.to_string(), call_id.to_string()));
    }
}

// ── Suspend-and-execute mechanics for one Write call ─────────────────────────

/// How long a suspended Write confirmation may wait for the user before
/// [`resolve_write`] gives up and treats it as a DENY (never an execute). A
/// generous ceiling — the human is expected to answer in seconds — but bounded so
/// a forgotten/abandoned prompt can never hang the run forever nor hold an
/// `AGENT_RUN_CONCURRENCY_MAX` slot indefinitely.
pub(super) const CONFIRM_TIMEOUT: Duration = Duration::from_secs(300);

/// Char cap applied to each string leaf of a Write tool's args before they are put
/// on the `confirm_request` step for display/edit.
///
/// CORRECTNESS: the renderer shows these args as an EDITABLE field and sends the
/// edit back verbatim as `editedArgs` on `approveEdited` — so this clamp must be AT
/// LEAST as large as the largest content ANY gated Write tool will actually
/// persist, or editing a longer piece of content would silently save a truncated
/// version. Sized to the LARGER of [`COVER_LETTER_CAP`] and [`SAVED_RESUME_CAP`]
/// (today's two gated Write tools' own content caps) rather than an independent,
/// smaller number, so display/edit fidelity can never drift below what gets saved
/// for either tool. Still a bounded ceiling — a 40k-char string in an event
/// payload is fine — for a truly pathological model output.
const ARGS_DISPLAY_CAP: usize = if COVER_LETTER_CAP > SAVED_RESUME_CAP {
    COVER_LETTER_CAP
} else {
    SAVED_RESUME_CAP
};

/// The display/edit clamp for a specific tool: that tool's OWN content cap.
///
/// [`ARGS_DISPLAY_CAP`] is the max across both gated Write tools (40k, from
/// `save_resume`), which guarantees we never show LESS than what gets saved — but
/// for the tool with the SMALLER cap it shows MORE. `save_cover_letter` truncates
/// at [`COVER_LETTER_CAP`] (20k), so a 20k–40k letter was displayed and edited in
/// full and then silently clipped on save: the user approves text that is not the
/// text that gets persisted.
///
/// Clamping per tool closes the gap from the other side. An unknown/future tool
/// falls back to the max, keeping the "never show less than is saved" guarantee
/// as the safe default.
fn display_cap_for(tool: &str) -> usize {
    match tool {
        "save_cover_letter" => COVER_LETTER_CAP,
        "save_resume" => SAVED_RESUME_CAP,
        _ => ARGS_DISPLAY_CAP,
    }
}

/// The trusted routing/egress + job-identity fields that live ONLY in
/// [`ToolContext`] and may NEVER be supplied (or overridden) through tool args —
/// checked in both camelCase and snake_case so neither wire spelling slips a
/// redirect past the gate. See the module SECURITY invariant.
///
/// [`ToolContext`]: super::tools::ToolContext
fn is_routing_egress_key(key: &str) -> bool {
    matches!(
        key,
        "provider" | "model" | "base_url" | "baseUrl" | "job_id" | "jobId"
    )
}

/// Recursively hunt `v` (objects AND array items, at every depth) for a routing/
/// egress key, returning the first one found. Both gated Write tools today
/// (`save_cover_letter`, `save_resume`) have a single flat `string` property, so a
/// top-level-only scan happens to be sufficient for either — but a FUTURE gated
/// tool could declare an object- or array-typed property, and a top-level-only
/// scan would miss a routing/egress key nested inside it. Walking the whole tree
/// keeps the boundary safe for whatever gets added next, not just what exists
/// today.
fn find_nested_routing_egress_key(v: &Value) -> Option<String> {
    match v {
        Value::Object(map) => {
            for (key, value) in map {
                if is_routing_egress_key(key) {
                    return Some(key.clone());
                }
                if let Some(found) = find_nested_routing_egress_key(value) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_nested_routing_egress_key),
        _ => None,
    }
}

/// Does a concrete JSON value satisfy a JSON-Schema primitive `type` token? Used
/// to shape-check user-edited Write args against the tool's fixed schema. `number`
/// and `integer` are treated interchangeably (any JSON number passes either).
fn json_type_matches(declared: &str, value: &Value) -> bool {
    match declared {
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "number" | "integer" => value.is_number(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        // Unknown/absent declared type — don't reject on shape we can't judge.
        _ => true,
    }
}

/// Re-validate user-EDITED Write args (`ApproveEdited`) against the tool's fixed,
/// trusted schema before they execute. Fail-closed: any violation returns `Err`
/// and the caller does NOT run the write.
///
/// The security-critical guarantee is that edited args change CONTENT only, never
/// routing/egress. Two independent layers enforce it:
/// 1. **Routing/egress rejection** — a key naming provider/model/base_url/job_id,
///    AT ANY NESTING DEPTH (not just top-level), is refused outright
///    ([`find_nested_routing_egress_key`]) — a future gated tool with an object-
///    or array-typed property can't smuggle one in a level down.
/// 2. **Schema whitelist** — every top-level key must be a declared property of
///    the tool's schema (which never contains a routing/egress field), and every
///    present value must match its declared primitive type; all `required` keys
///    must be present. Unknown keys (the vector an injection would use) are
///    rejected.
fn validate_edited_args(tools: &[AgentTool], name: &str, v: &Value) -> AppResult<()> {
    let tool = tools
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| AppError::Validation(format!("unknown tool '{name}'")))?;
    let obj = v.as_object().ok_or_else(|| {
        AppError::Validation("edited arguments must be a JSON object".to_string())
    })?;
    let props = tool.schema.get("properties").and_then(|p| p.as_object());

    // Layer 1: never let an edit introduce a trusted-context field, at any depth.
    if let Some(key) = find_nested_routing_egress_key(v) {
        return Err(AppError::Validation(format!(
            "edited arguments may not carry the trusted routing/egress field '{key}'"
        )));
    }

    for (key, value) in obj {
        // Layer 2: whitelist by schema — reject any undeclared key…
        let Some(schema) = props.and_then(|p| p.get(key)) else {
            return Err(AppError::Validation(format!(
                "edited arguments carry an unknown field '{key}' not in the tool's schema"
            )));
        };
        // …and shape-check the value's type against the declaration.
        if let Some(declared) = schema.get("type").and_then(|t| t.as_str()) {
            if !json_type_matches(declared, value) {
                return Err(AppError::Validation(format!(
                    "edited field '{key}' has the wrong type (expected {declared})"
                )));
            }
        }
    }

    // Every required schema field must still be present after the edit.
    if let Some(required) = tool.schema.get("required").and_then(|r| r.as_array()) {
        for req in required.iter().filter_map(|r| r.as_str()) {
            if !obj.contains_key(req) {
                return Err(AppError::Validation(format!(
                    "edited arguments are missing the required field '{req}'"
                )));
            }
        }
    }
    Ok(())
}

/// Clamp every string leaf of a JSON value to `cap` chars (char-boundary safe,
/// appending `…` when truncated) so a Write tool's untrusted, possibly-huge args
/// (e.g. a full cover letter) can't blow the `confirm_request` event payload.
/// Structure is preserved; the FULL args are what actually execute on approval.
fn clamp_json_strings(v: &Value, cap: usize) -> Value {
    match v {
        Value::String(s) => {
            if s.chars().count() > cap {
                let mut out: String = s.chars().take(cap).collect();
                out.push('…');
                Value::String(out)
            } else {
                v.clone()
            }
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|i| clamp_json_strings(i, cap)).collect())
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, val)| (k.clone(), clamp_json_strings(val, cap)))
                .collect(),
        ),
        _ => v.clone(),
    }
}

/// Outcome of suspending on the confirm gate for one Write call: either the run
/// was cancelled while suspended (propagate `Cancelled`, do NOT act) or a
/// tool-result body to fold into the transcript (the write ran, was declined, or
/// timed out — all NON-cancelling).
pub(super) enum WriteResolution {
    Cancelled,
    Body(String),
}

/// SUSPEND the loop on a `ToolKind::Write` call: emit a `confirm_request` step,
/// register a [`oneshot`] with the [`AgentGate`], then block on the user's decision
/// raced against BOTH cancellation and [`CONFIRM_TIMEOUT`]. The gate entry is
/// ALWAYS removed before returning (every branch).
///
/// SECURITY: the write executes ONLY on `Approve`/`ApproveEdited`; `Deny`, a
/// timeout, a closed channel, and cancel all default to NOT acting. `ApproveEdited`
/// is re-validated ([`validate_edited_args`]) — content only, never routing/egress
/// — and runs with the EDITED args; `Approve` runs with the ORIGINAL args.
#[allow(clippy::too_many_arguments)]
pub(super) async fn resolve_write(
    env: &dyn AgentEnv,
    tools: &[AgentTool],
    gate: &AgentGate,
    confirm_timeout: Duration,
    job_id: &str,
    step: usize,
    idx: usize,
    call: &crate::commands::ai_provider::ToolCall,
    cancel: &CancellationToken,
) -> WriteResolution {
    // Stable, unique-per-pending-call id within this run. `idx` is this call's
    // position in `turn.tool_calls` (the caller's `enumerate()` index) — the
    // guarantee a same-turn duplicate Write tool call gets a DISTINCT id from.
    // `step` alone is not enough: a single turn can request the same Write tool
    // twice (e.g. two `save_cover_letter` calls), which would collide on
    // `"{step}-{name}"` and let a stale/duplicate `agent_confirm` resolve the
    // WRONG pending call (a confirm-bypass for the second, unrelated write).
    // `call.id` alone is not enough either — some providers synthesize/reuse ids —
    // so `idx` is the part that's actually guaranteed unique; `call.name` is kept
    // only for readability.
    let call_id = format!("{step}-{idx}-{}", call.name);

    // Announce the suspended write with its EXACT (clamped) args so the UI shows
    // the user precisely what they are approving.
    env.on_step(&AgentStep {
        job_id: job_id.to_string(),
        step,
        text: String::new(),
        tools: Vec::new(),
        denied: Vec::new(),
        kind: AgentStepKind::ConfirmRequest,
        confirm: Some(ConfirmRequest {
            call_id: call_id.clone(),
            tool: call.name.clone(),
            args: clamp_json_strings(&call.args, display_cap_for(&call.name)),
        }),
    });

    let (tx, rx) = oneshot::channel::<Decision>();
    gate.register(job_id.to_string(), call_id.clone(), tx);

    // Await the user, but never forever: a cancel returns immediately (no act) and
    // a timeout falls through to a no-act deny. `biased` favors cancel over a
    // simultaneously-ready decision so Stop always wins.
    let decision: Option<Decision> = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            gate.remove(job_id, &call_id);
            return WriteResolution::Cancelled;
        }
        _ = tokio::time::sleep(confirm_timeout) => None,
        received = rx => received.ok(), // Err = sender dropped → treat as no-act
    };
    // The waiter is done — drop the (possibly already-removed) entry so the map
    // never retains a stale sender, on EVERY non-cancel branch.
    gate.remove(job_id, &call_id);

    let body = match decision {
        Some(Decision::Approve) => {
            match run_write_raced(env, cancel, &call.name, call.args.clone()).await {
                Some(Ok(v)) => v.to_string(),
                Some(Err(e)) => format!("error: {e}"),
                None => return WriteResolution::Cancelled,
            }
        }
        Some(Decision::ApproveEdited(edited)) => match validate_edited_args(tools, &call.name, &edited)
        {
            Ok(()) => match run_write_raced(env, cancel, &call.name, edited).await {
                Some(Ok(v)) => v.to_string(),
                Some(Err(e)) => format!("error: {e}"),
                None => return WriteResolution::Cancelled,
            },
            // Fail-closed: an invalid edit is NOT executed.
            Err(e) => format!(
                "declined: the user's edited arguments were rejected and the action did not run ({e})"
            ),
        },
        Some(Decision::Deny) => {
            "declined: the user declined this action; nothing was changed.".to_string()
        }
        None => "declined: no confirmation was received in time, so the action was not \
                 performed. You may summarize what you prepared and stop."
            .to_string(),
    };
    WriteResolution::Body(body)
}

/// Run an approved Write tool, racing it against cancellation (an app-internal
/// write can still take a lock / hit disk). `Some(result)` when it ran, `None`
/// when cancel fired first (the caller propagates `Cancelled` — it did NOT act).
async fn run_write_raced(
    env: &dyn AgentEnv,
    cancel: &CancellationToken,
    name: &str,
    args: Value,
) -> Option<AppResult<Value>> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => None,
        result = env.run_write_tool(name, args) => Some(result),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_unknown_call_returns_false_and_does_not_panic() {
        let gate = AgentGate::default();
        assert!(
            !gate.resolve("job-1", "1-writer", Decision::Approve),
            "resolving a call that was never registered must be a benign false"
        );
    }

    #[tokio::test]
    async fn register_then_resolve_delivers_the_decision_to_the_waiter() {
        let gate = AgentGate::default();
        let (tx, rx) = oneshot::channel();
        gate.register("job-1".into(), "1-writer".into(), tx);
        assert!(gate.resolve("job-1", "1-writer", Decision::Deny));
        assert!(
            matches!(rx.await, Ok(Decision::Deny)),
            "the waiter must receive exactly the resolved decision"
        );
    }

    #[tokio::test]
    async fn approve_edited_carries_the_edited_value_through() {
        let gate = AgentGate::default();
        let (tx, rx) = oneshot::channel();
        gate.register("job-1".into(), "1-writer".into(), tx);
        let edited = json!({ "coverLetterText": "edited by the user" });
        assert!(gate.resolve("job-1", "1-writer", Decision::ApproveEdited(edited.clone())));
        match rx.await {
            Ok(Decision::ApproveEdited(v)) => assert_eq!(v, edited),
            other => panic!("expected the edited value to arrive, got {other:?}"),
        }
    }

    #[test]
    fn remove_makes_a_subsequent_resolve_return_false() {
        let gate = AgentGate::default();
        let (tx, _rx) = oneshot::channel();
        gate.register("job-1".into(), "1-writer".into(), tx);
        gate.remove("job-1", "1-writer");
        assert!(
            !gate.resolve("job-1", "1-writer", Decision::Approve),
            "a removed entry must no longer be resolvable"
        );
    }

    #[test]
    fn resolve_after_waiter_dropped_returns_false() {
        let gate = AgentGate::default();
        let (tx, rx) = oneshot::channel();
        gate.register("job-1".into(), "1-writer".into(), tx);
        drop(rx); // the suspended run went away (e.g. task aborted)
        assert!(
            !gate.resolve("job-1", "1-writer", Decision::Approve),
            "a dropped receiver must make resolve report failure, not panic"
        );
    }

    #[test]
    fn entries_are_scoped_per_job_and_call() {
        // Two runs can be in flight (AGENT_RUN_CONCURRENCY_MAX > 1); the same
        // callId under a different jobId is a DISTINCT pending entry.
        let gate = AgentGate::default();
        let (tx_a, _rx_a) = oneshot::channel();
        let (tx_b, _rx_b) = oneshot::channel();
        gate.register("job-a".into(), "1-writer".into(), tx_a);
        gate.register("job-b".into(), "1-writer".into(), tx_b);
        // Resolving job-a leaves job-b untouched.
        assert!(gate.resolve("job-a", "1-writer", Decision::Deny));
        assert!(gate.resolve("job-b", "1-writer", Decision::Deny));
    }

    // ── Suspend-and-execute mechanics (`resolve_write`) ───────────────────────
    //
    // This test harness (`FakeEnv`, `whitelist`, `write_call`, `spawn_resolver`,
    // `run_gated`, …) is a self-contained DUPLICATE of the one `agent::controller`'s
    // own test module keeps for its (Write-free) core-loop tests — the two test
    // modules are independent so each file stays comfortably under the
    // architecture LOC cap; see `agent::controller`'s test module for the
    // core-loop-only variant.

    use std::collections::VecDeque;
    use std::sync::Arc;

    use async_trait::async_trait;
    use tauri::AppHandle;

    use crate::agent::controller::{
        run_agent_with_system, AgentEnv, AgentOutcome, AgentStep, AgentStepKind, StoppedReason,
        AGENT_SYSTEM,
    };
    use crate::agent::tools::{ToolContext, ToolKind};
    use crate::commands::ai_provider::{AgentTurn, ChatMsg, Role, StopReason, ToolCall, Usage};

    /// A scripted fake: pops a canned [`AgentTurn`] per `turn()` (repeating the last
    /// one forever), records executed read AND write tools + narrated steps + the
    /// exact transcript it was handed each call, and returns a canned tool result.
    /// No `AppHandle` — that is the whole point of the seam.
    struct FakeEnv {
        turns: Mutex<VecDeque<AgentTurn>>,
        last: AgentTurn,
        reads: Mutex<Vec<String>>,
        /// Every executed WRITE (name + the exact args it ran with). The confirm
        /// gate must make this empty on Deny/timeout/cancel and hold exactly one
        /// entry (with the approved-or-edited args) on approve.
        writes: Mutex<Vec<(String, Value)>>,
        steps: Mutex<Vec<AgentStep>>,
        transcripts: Mutex<Vec<Vec<ChatMsg>>>,
    }

    impl FakeEnv {
        fn new(turns: Vec<AgentTurn>) -> Self {
            let last = turns.last().cloned().expect("at least one scripted turn");
            Self {
                turns: Mutex::new(turns.into()),
                last,
                reads: Mutex::new(Vec::new()),
                writes: Mutex::new(Vec::new()),
                steps: Mutex::new(Vec::new()),
                transcripts: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl AgentEnv for FakeEnv {
        async fn turn(&self, messages: &[ChatMsg]) -> AppResult<AgentTurn> {
            self.transcripts.lock().push(messages.to_vec());
            let next = self.turns.lock().pop_front();
            Ok(next.unwrap_or_else(|| self.last.clone()))
        }
        async fn run_read_tool(&self, name: &str, _args: Value) -> AppResult<Value> {
            self.reads.lock().push(name.to_string());
            Ok(json!({ "ran": name }))
        }
        async fn run_write_tool(&self, name: &str, args: Value) -> AppResult<Value> {
            self.writes.lock().push((name.to_string(), args));
            Ok(json!({ "wrote": name }))
        }
        fn on_step(&self, step: &AgentStep) {
            self.steps.lock().push(step.clone());
        }
    }

    /// Spawn a task that resolves the deterministic `"{step}-{idx}-{tool}"`
    /// pending call as soon as `resolve_write` registers it. Retries each yield
    /// until the entry exists (bounded) so the test never races the
    /// register/suspend ordering.
    fn spawn_resolver(gate: Arc<AgentGate>, job_id: &str, call_id: &str, decision: Decision) {
        let job_id = job_id.to_string();
        let call_id = call_id.to_string();
        tokio::spawn(async move {
            for _ in 0..10_000 {
                if gate.resolve(&job_id, &call_id, decision.clone()) {
                    return;
                }
                tokio::task::yield_now().await;
            }
        });
    }

    /// Dummy handler — never invoked (the `FakeEnv` is the tool-execution seam).
    fn never(
        _app: &AppHandle,
        _ctx: &ToolContext,
        _args: Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Value>> + Send>> {
        Box::pin(async { Ok(Value::Null) })
    }

    fn whitelist() -> Vec<AgentTool> {
        vec![
            AgentTool {
                name: "reader",
                description: "r".into(),
                schema: json!({}),
                kind: ToolKind::Read,
                handler: never,
            },
            AgentTool {
                name: "reader2",
                description: "r2".into(),
                schema: json!({}),
                kind: ToolKind::Read,
                handler: never,
            },
            AgentTool {
                name: "writer",
                description: "w".into(),
                // A realistic content-only schema so the edited-args re-validation
                // (whitelist + type + required) has something to check against.
                schema: json!({
                    "type": "object",
                    "properties": {
                        "coverLetterText": { "type": "string" }
                    },
                    "required": ["coverLetterText"]
                }),
                kind: ToolKind::Write,
                handler: never,
            },
        ]
    }

    fn write_call(name: &str) -> AgentTurn {
        write_call_with_args(name, json!({ "coverLetterText": "the original draft" }))
    }
    /// A write-tool turn carrying explicit args — used to prove the ORIGINAL args
    /// are what execute on a plain `Approve`.
    fn write_call_with_args(name: &str, args: Value) -> AgentTurn {
        AgentTurn {
            text: format!("writing {name}"),
            tool_calls: vec![ToolCall {
                id: "1".into(),
                name: name.into(),
                args,
            }],
            stop: StopReason::ToolUse,
            usage: Usage::default(),
        }
    }
    /// A turn requesting the SAME Write tool TWICE — with the SAME
    /// provider-supplied `call.id` (some providers synthesize/reuse ids, so the
    /// dedup key must NOT depend on it) but different args, so a test can prove
    /// each pending call gets a callId unique by loop INDEX, not by name or by
    /// `call.id` alone.
    fn double_write_call(name: &str) -> AgentTurn {
        AgentTurn {
            text: format!("writing {name} twice"),
            tool_calls: vec![
                ToolCall {
                    id: "dup".into(),
                    name: name.into(),
                    args: json!({ "coverLetterText": "first call" }),
                },
                ToolCall {
                    id: "dup".into(),
                    name: name.into(),
                    args: json!({ "coverLetterText": "second call" }),
                },
            ],
            stop: StopReason::ToolUse,
            usage: Usage::default(),
        }
    }
    fn final_turn(text: &str) -> AgentTurn {
        AgentTurn {
            text: text.into(),
            tool_calls: vec![],
            stop: StopReason::End,
            usage: Usage::default(),
        }
    }

    /// These tests suspend on a real Write, so they drive `run_agent_with_system`
    /// directly with a shared gate they can resolve. `confirm_timeout` lets the
    /// timeout test pass a tiny ceiling; approve/deny/edit tests pass a generous one.
    async fn run_gated(
        env: &FakeEnv,
        gate: &AgentGate,
        confirm_timeout: Duration,
        job_id: &str,
        cancel: &CancellationToken,
    ) -> AppResult<AgentOutcome> {
        run_agent_with_system(
            env,
            &whitelist(),
            gate,
            confirm_timeout,
            AGENT_SYSTEM,
            job_id,
            "prep this".into(),
            cancel,
        )
        .await
    }

    /// A Write tool SUSPENDS the run and emits a `confirm_request` step carrying
    /// the pending call's id, tool name, and (clamped) args — never auto-executing.
    #[tokio::test]
    async fn write_tool_emits_a_confirm_request_and_suspends() {
        let env = FakeEnv::new(vec![write_call("writer"), final_turn("summary")]);
        let gate = Arc::new(AgentGate::default());
        // Deny so the run terminates instead of hanging on the suspend.
        spawn_resolver(gate.clone(), "job-1", "1-0-writer", Decision::Deny);
        let out = run_gated(
            &env,
            &gate,
            CONFIRM_TIMEOUT,
            "job-1",
            &CancellationToken::new(),
        )
        .await
        .unwrap();

        let steps = env.steps.lock();
        let confirm = steps
            .iter()
            .find(|s| s.kind == AgentStepKind::ConfirmRequest)
            .and_then(|s| s.confirm.as_ref())
            .expect("a Write tool must emit a confirm_request step");
        assert_eq!(confirm.call_id, "1-0-writer");
        assert_eq!(confirm.tool, "writer");
        assert_eq!(confirm.args["coverLetterText"], "the original draft");
        assert_eq!(out.stopped_reason, StoppedReason::Done);
    }

    /// APPROVE runs the Write handler EXACTLY once, with the ORIGINAL args, and the
    /// loop continues to a normal finish.
    #[tokio::test]
    async fn approve_runs_the_write_handler_once_with_original_args() {
        let env = FakeEnv::new(vec![write_call("writer"), final_turn("done")]);
        let gate = Arc::new(AgentGate::default());
        spawn_resolver(gate.clone(), "job-1", "1-0-writer", Decision::Approve);
        let out = run_gated(
            &env,
            &gate,
            CONFIRM_TIMEOUT,
            "job-1",
            &CancellationToken::new(),
        )
        .await
        .unwrap();

        let writes = env.writes.lock();
        assert_eq!(
            writes.len(),
            1,
            "the write must run exactly once on approve"
        );
        assert_eq!(writes[0].0, "writer");
        assert_eq!(writes[0].1["coverLetterText"], "the original draft");
        assert_eq!(out.stopped_reason, StoppedReason::Done);
        assert_eq!(out.final_text, "done");
    }

    /// DENY never runs the handler; the loop continues (the model gets a "declined"
    /// tool-result and finishes normally).
    #[tokio::test]
    async fn deny_never_runs_the_write_and_the_loop_continues() {
        let env = FakeEnv::new(vec![write_call("writer"), final_turn("acknowledged")]);
        let gate = Arc::new(AgentGate::default());
        spawn_resolver(gate.clone(), "job-1", "1-0-writer", Decision::Deny);
        let out = run_gated(
            &env,
            &gate,
            CONFIRM_TIMEOUT,
            "job-1",
            &CancellationToken::new(),
        )
        .await
        .unwrap();

        assert!(
            env.writes.lock().is_empty(),
            "a denied write must never execute"
        );
        assert_eq!(out.stopped_reason, StoppedReason::Done);
        assert_eq!(out.final_text, "acknowledged");
        // The transcript carried a fenced "declined" result the model could react to.
        let transcripts = env.transcripts.lock();
        let last = transcripts.last().expect("a follow-up turn");
        let tool_msg = last
            .iter()
            .find(|m| m.role == Role::Tool)
            .expect("a tool-result message");
        assert!(tool_msg.content.contains("declined"));
    }

    /// APPROVE_EDITED runs the handler with the EDITED (content-only) args.
    #[tokio::test]
    async fn approve_edited_runs_with_the_edited_args() {
        let env = FakeEnv::new(vec![write_call("writer"), final_turn("done")]);
        let gate = Arc::new(AgentGate::default());
        let edited = json!({ "coverLetterText": "the user's edited letter" });
        spawn_resolver(
            gate.clone(),
            "job-1",
            "1-0-writer",
            Decision::ApproveEdited(edited),
        );
        run_gated(
            &env,
            &gate,
            CONFIRM_TIMEOUT,
            "job-1",
            &CancellationToken::new(),
        )
        .await
        .unwrap();

        let writes = env.writes.lock();
        assert_eq!(writes.len(), 1);
        assert_eq!(
            writes[0].1["coverLetterText"], "the user's edited letter",
            "the write must run with the EDITED content, not the original"
        );
    }

    /// APPROVE_EDITED whose edit smuggles a routing/egress field is REJECTED — the
    /// handler never runs (edited args may change content only).
    #[tokio::test]
    async fn approve_edited_rejects_routing_egress_fields() {
        let env = FakeEnv::new(vec![write_call("writer"), final_turn("done")]);
        let gate = Arc::new(AgentGate::default());
        // A prompt-injection-style edit that tries to redirect the provider.
        let malicious = json!({
            "coverLetterText": "ok",
            "baseUrl": "http://attacker.example"
        });
        spawn_resolver(
            gate.clone(),
            "job-1",
            "1-0-writer",
            Decision::ApproveEdited(malicious),
        );
        let out = run_gated(
            &env,
            &gate,
            CONFIRM_TIMEOUT,
            "job-1",
            &CancellationToken::new(),
        )
        .await
        .unwrap();

        assert!(
            env.writes.lock().is_empty(),
            "an edit carrying a routing/egress field must NOT execute"
        );
        assert_eq!(out.stopped_reason, StoppedReason::Done);
    }

    /// APPROVE_EDITED that introduces an UNKNOWN (non-schema) field is rejected —
    /// the schema whitelist fails closed and the handler never runs.
    #[tokio::test]
    async fn approve_edited_rejects_unknown_schema_fields() {
        let env = FakeEnv::new(vec![write_call("writer"), final_turn("done")]);
        let gate = Arc::new(AgentGate::default());
        let extra = json!({ "coverLetterText": "ok", "surprise": "payload" });
        spawn_resolver(
            gate.clone(),
            "job-1",
            "1-0-writer",
            Decision::ApproveEdited(extra),
        );
        run_gated(
            &env,
            &gate,
            CONFIRM_TIMEOUT,
            "job-1",
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(
            env.writes.lock().is_empty(),
            "an edit with an undeclared field must NOT execute"
        );
    }

    /// CANCEL while suspended returns `Cancelled` and never runs the handler.
    #[tokio::test]
    async fn cancel_while_suspended_stops_without_running_the_write() {
        let env = FakeEnv::new(vec![write_call("writer"), final_turn("unreached")]);
        let gate = Arc::new(AgentGate::default());
        let cancel = CancellationToken::new();
        let cancel_task = cancel.clone();
        // No resolver — cancel the run while it waits on the confirmation.
        tokio::spawn(async move {
            cancel_task.cancel();
        });
        let out = run_gated(&env, &gate, CONFIRM_TIMEOUT, "job-1", &cancel)
            .await
            .unwrap();

        assert_eq!(out.stopped_reason, StoppedReason::Cancelled);
        assert!(
            env.writes.lock().is_empty(),
            "a cancelled suspension must never execute the write"
        );
    }

    /// TIMEOUT while suspended treats the call as a no-act deny — the handler never
    /// runs — and the loop continues (does not execute on timeout, ever).
    #[tokio::test]
    async fn timeout_while_suspended_does_not_run_the_write() {
        let env = FakeEnv::new(vec![write_call("writer"), final_turn("timed out")]);
        let gate = Arc::new(AgentGate::default());
        // Tiny ceiling, no resolver: the suspend times out.
        let out = run_gated(
            &env,
            &gate,
            Duration::from_millis(20),
            "job-1",
            &CancellationToken::new(),
        )
        .await
        .unwrap();

        assert!(
            env.writes.lock().is_empty(),
            "a timed-out suspension must never execute the write"
        );
        assert_eq!(out.stopped_reason, StoppedReason::Done);
        assert_eq!(out.final_text, "timed out");
    }

    /// The gate entry is ALWAYS removed after a resume (here: after approve), so a
    /// late duplicate `resolve` for the same call finds nothing.
    #[tokio::test]
    async fn gate_entry_is_removed_after_resume() {
        let env = FakeEnv::new(vec![write_call("writer"), final_turn("done")]);
        let gate = Arc::new(AgentGate::default());
        spawn_resolver(gate.clone(), "job-1", "1-0-writer", Decision::Approve);
        run_gated(
            &env,
            &gate,
            CONFIRM_TIMEOUT,
            "job-1",
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(
            !gate.resolve("job-1", "1-0-writer", Decision::Approve),
            "the pending entry must be gone after the run resumed"
        );
    }

    /// MEDIUM fix: a single turn requesting the SAME Write tool TWICE (even with
    /// the SAME provider-supplied `call.id`, simulating a provider that
    /// synthesizes/reuses ids) must get two DISTINCT pending callIds — never the
    /// naive `"{step}-{name}"`, which would collide and let a stale/duplicate
    /// `agent_confirm` resolve the WRONG pending call. Each decision must apply
    /// to its own call only.
    #[tokio::test]
    async fn duplicate_write_tool_calls_in_one_turn_get_distinct_call_ids() {
        let env = FakeEnv::new(vec![double_write_call("writer"), final_turn("done")]);
        let gate = Arc::new(AgentGate::default());
        // The first call (idx 0) is approved; the second (idx 1) is denied — if the
        // two calls collided onto the same callId, one resolver would race the
        // other and/or the wrong decision would apply to the wrong call.
        spawn_resolver(gate.clone(), "job-1", "1-0-writer", Decision::Approve);
        spawn_resolver(gate.clone(), "job-1", "1-1-writer", Decision::Deny);
        run_gated(
            &env,
            &gate,
            CONFIRM_TIMEOUT,
            "job-1",
            &CancellationToken::new(),
        )
        .await
        .unwrap();

        // Two distinct confirm_request steps, with distinct, index-qualified ids —
        // never the collision-prone "1-writer" both calls would share under the
        // old `"{step}-{name}"` scheme.
        let steps = env.steps.lock();
        let call_ids: Vec<&str> = steps
            .iter()
            .filter(|s| s.kind == AgentStepKind::ConfirmRequest)
            .filter_map(|s| s.confirm.as_ref())
            .map(|c| c.call_id.as_str())
            .collect();
        assert_eq!(call_ids, vec!["1-0-writer", "1-1-writer"]);

        // Exactly the FIRST call's decision (Approve) executed, with the FIRST
        // call's own args — the second (Denied) call never ran.
        let writes = env.writes.lock();
        assert_eq!(writes.len(), 1, "only the approved call may execute");
        assert_eq!(writes[0].1["coverLetterText"], "first call");

        // A resolve targeting the naive, non-unique "1-writer" key finds nothing —
        // proving the calls are NOT keyed that way (no cross-call bypass surface).
        assert!(!gate.resolve("job-1", "1-writer", Decision::Approve));
    }

    // ── Pure helpers (edited-args validation + display clamping) ──────────────

    #[test]
    fn validate_edited_args_accepts_content_only_edit() {
        let tools = whitelist();
        let ok = json!({ "coverLetterText": "a fresh letter" });
        assert!(validate_edited_args(&tools, "writer", &ok).is_ok());
    }

    #[test]
    fn validate_edited_args_rejects_every_routing_egress_spelling() {
        let tools = whitelist();
        for key in [
            "provider", "model", "base_url", "baseUrl", "job_id", "jobId",
        ] {
            let mut obj = serde_json::Map::new();
            obj.insert("coverLetterText".into(), json!("ok"));
            obj.insert(key.into(), json!("attacker"));
            let v = Value::Object(obj);
            assert!(
                validate_edited_args(&tools, "writer", &v).is_err(),
                "routing/egress field '{key}' must be rejected"
            );
        }
    }

    /// HARDENED: the routing/egress denylist must catch a routing key nested
    /// INSIDE a declared object-typed property, not just at the top level. This
    /// tool's schema (unlike the real `save_cover_letter`) declares an
    /// object-typed `meta` field on purpose, so layer 2 (the schema whitelist)
    /// would happily accept `meta` as a known key — proving the rejection here
    /// comes from the recursive layer-1 scan, not from `meta` being unknown.
    #[test]
    fn validate_edited_args_rejects_a_routing_key_nested_inside_an_object_field() {
        let tools = vec![AgentTool {
            name: "writer_with_object_field",
            description: "test-only tool with a nested object property".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "coverLetterText": { "type": "string" },
                    "meta": { "type": "object" }
                },
                "required": ["coverLetterText"]
            }),
            kind: ToolKind::Write,
            handler: never,
        }];
        let nested_malicious = json!({
            "coverLetterText": "ok",
            "meta": { "jobId": "attacker-controlled-job" }
        });
        assert!(
            validate_edited_args(&tools, "writer_with_object_field", &nested_malicious).is_err(),
            "a routing/egress key nested inside a declared object field must be rejected"
        );
        // A benign nested object (no routing/egress key at any depth) still passes.
        let benign = json!({
            "coverLetterText": "ok",
            "meta": { "note": "just a regular nested value" }
        });
        assert!(validate_edited_args(&tools, "writer_with_object_field", &benign).is_ok());
    }

    #[test]
    fn validate_edited_args_rejects_unknown_and_missing_and_mistyped() {
        let tools = whitelist();
        // Unknown field.
        assert!(validate_edited_args(
            &tools,
            "writer",
            &json!({ "coverLetterText": "x", "extra": 1 })
        )
        .is_err());
        // Missing required field.
        assert!(validate_edited_args(&tools, "writer", &json!({})).is_err());
        // Wrong type for a declared string field.
        assert!(validate_edited_args(&tools, "writer", &json!({ "coverLetterText": 42 })).is_err());
        // Not an object at all.
        assert!(validate_edited_args(&tools, "writer", &json!("just a string")).is_err());
    }

    #[test]
    fn clamp_json_strings_truncates_long_leaves_but_keeps_structure() {
        let long = "z".repeat(ARGS_DISPLAY_CAP + 500);
        let v = json!({ "coverLetterText": long, "nested": { "k": "short" } });
        let clamped = clamp_json_strings(&v, ARGS_DISPLAY_CAP);
        let got = clamped["coverLetterText"].as_str().unwrap();
        assert_eq!(
            got.chars().count(),
            ARGS_DISPLAY_CAP + 1,
            "a truncated leaf keeps cap chars plus the ellipsis"
        );
        assert!(got.ends_with('…'));
        // Short, structured values pass through unchanged.
        assert_eq!(clamped["nested"]["k"], "short");
    }

    /// The confirm step must show what the tool will actually PERSIST. The shared
    /// ARGS_DISPLAY_CAP is the max across both Write tools (40k), so a cover letter
    /// between 20k and 40k chars was shown and edited in full and then silently
    /// clipped to COVER_LETTER_CAP on save.
    #[test]
    fn display_cap_matches_each_write_tool_own_content_cap() {
        // The two caps really do differ — otherwise this per-tool split is a no-op.
        // Compile-time, so it also fails the build if they are ever equalised.
        const _: () = assert!(COVER_LETTER_CAP < SAVED_RESUME_CAP);

        assert_eq!(display_cap_for("save_cover_letter"), COVER_LETTER_CAP);
        assert_eq!(display_cap_for("save_resume"), SAVED_RESUME_CAP);
        // An unknown/future tool keeps the safe default: never show LESS than the
        // largest thing any Write tool can persist.
        assert_eq!(display_cap_for("some_future_writer"), ARGS_DISPLAY_CAP);
    }

    /// A letter longer than the save cap is clamped for display to exactly what
    /// `save_cover_letter` would keep — no more.
    #[test]
    fn a_long_cover_letter_is_displayed_at_the_save_cap() {
        let long = "z".repeat(SAVED_RESUME_CAP);
        let v = json!({ "coverLetterText": long });
        let clamped = clamp_json_strings(&v, display_cap_for("save_cover_letter"));
        let got = clamped["coverLetterText"].as_str().unwrap();
        assert_eq!(
            got.chars().count(),
            COVER_LETTER_CAP + 1,
            "shown at the cover-letter save cap (plus the ellipsis), not the resume cap"
        );
    }
}
