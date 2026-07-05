//! The human-in-the-loop confirm gate (Phase 3) — the safety core of the agent.
//!
//! # Security invariants (verified by `tauri-security-reviewer`)
//!
//! - **Explicit approval for every side effect.** A [`ToolKind::Write`] tool call
//!   never executes on the model's say-so. The controller SUSPENDS the run, emits
//!   an `agent:step` of kind `confirm_request` carrying the exact tool + (clamped)
//!   args, and blocks on a [`oneshot`] until the user resolves it via
//!   `agent_confirm`. `Deny`, a timeout, and cancel ALL default to **NOT acting**.
//! - **Edited args may change CONTENT only — never routing/egress.** On
//!   [`Decision::ApproveEdited`] the controller re-validates the edited JSON against
//!   the tool's fixed schema (whitelist of declared keys) and re-asserts it carries
//!   no `provider`/`model`/`base_url`/`job_id` field. Those stay in the trusted
//!   [`ToolContext`] threaded from `agent_run`; a prompt-injected posting can steer
//!   the *content* the model proposes to write, never where a credentialed request
//!   is routed nor which job/application it mutates.
//! - **No arbitrary-fetch/email/shell/URL tool exists.** The whitelist is per-flow
//!   and every Write tool is app-INTERNAL (persist to a local store). The gate's
//!   whole point is that even these internal writes now require approval.
//! - **Prompt injection can REQUEST but never EXECUTE.** Hostile job/résumé text can
//!   make the model *ask* for a write; it can never satisfy the gate on the user's
//!   behalf — only a real `agent_confirm` IPC call (or a `resolve` in a test) can.
//!
//! [`ToolKind::Write`]: super::tools::ToolKind
//! [`ToolContext`]: super::tools::ToolContext

use std::collections::HashMap;

use parking_lot::Mutex;
use serde_json::Value;
use tokio::sync::oneshot;

/// The user's verdict on one pending Write action, delivered from `agent_confirm`
/// back to the suspended controller loop over a [`oneshot`] channel. `Clone` is a
/// test convenience (retry a resolve); production moves it through the channel once.
#[derive(Debug, Clone)]
pub enum Decision {
    /// Run the Write handler with the model's ORIGINAL args, verbatim.
    Approve,
    /// Run the Write handler with user-EDITED args. The controller re-validates
    /// these against the tool schema and re-asserts they carry no routing/egress
    /// field before executing — edited args may change content only.
    ApproveEdited(Value),
    /// Do not act; push a "user declined" tool-result and continue the loop.
    Deny,
}

/// Managed Tauri state: the set of Write confirmations currently AWAITING a user
/// decision, keyed by `(jobId, callId)`. The controller [`register`]s a sender
/// before it suspends; `agent_confirm` [`resolve`]s it when the user answers; the
/// controller always [`remove`]s the entry once it resumes (via any branch —
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
    /// Record a pending confirmation and its wake-up channel. Called by the
    /// controller immediately before it suspends on the receiver.
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
    /// entry is a no-op. The controller calls this on EVERY resume branch so the
    /// map never retains a stale sender.
    pub fn remove(&self, job_id: &str, call_id: &str) {
        self.pending
            .lock()
            .remove(&(job_id.to_string(), call_id.to_string()));
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
}
