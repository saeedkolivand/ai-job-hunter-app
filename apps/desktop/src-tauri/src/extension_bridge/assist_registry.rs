//! The per-connection `answer.assist` stream registry — the `reqId` state
//! machine ([`entry::StreamEntry`]/[`AssistStreamRegistry`]) plus the two small
//! `AppHandle`-abstracting seams ([`JobCanceller`]/[`JobStarter`]) and
//! [`start_and_register`] that let its start/register/cancel ordering be
//! unit-tested without a live `AppHandle` (this crate has no `tauri::test`
//! mock-app harness). This module IS the state machine [`super::stream`]
//! orchestrates around — see that module's doc for the full picture (the
//! write-backpressure/stalled-peer fix, the four ways a stream ends early,
//! and the CWE-639 per-connection isolation this registry exists for).
//!
//! [`AssistStreamRegistry`] is re-exported from `stream` as
//! `stream::AssistStreamRegistry` so every existing reference keeps
//! resolving unchanged.
//!
//! Split into [`entry`] (the `StreamEntry` state) and [`registry`] (the
//! `AssistStreamRegistry` operations over it).

use tauri::AppHandle;

mod entry;
mod registry;

#[cfg(test)]
mod tests;

pub(super) use self::registry::AssistStreamRegistry;

/// Abstracts "cancel one job by id" so [`AssistStreamRegistry::cancel`]/
/// [`AssistStreamRegistry::cancel_all`]'s job-cancelling side effect is
/// unit-testable against a fake recorder, without a live `AppHandle` — this
/// crate has no `tauri::test` mock-app harness. The sole production
/// implementor forwards to [`crate::commands::jobs::job_cancel`].
pub(super) trait JobCanceller {
    fn cancel_job(&self, job_id: &str);
}

impl JobCanceller for AppHandle {
    fn cancel_job(&self, job_id: &str) {
        crate::commands::jobs::job_cancel(self, job_id);
    }
}

/// Abstracts "start a job by id" so [`start_and_register`]'s
/// start-before-register ordering is unit-testable against a fake recorder —
/// mirrors [`JobCanceller`]. The sole production implementor forwards to
/// [`crate::commands::jobs::job_start`] with this module's one fixed job
/// kind (`"extension.answer_assist"`).
pub(super) trait JobStarter {
    fn start_job(&self, job_id: &str);
}

impl JobStarter for AppHandle {
    fn start_job(&self, job_id: &str) {
        crate::commands::jobs::job_start(self, job_id, "extension.answer_assist");
    }
}

/// Start a fresh job for `req_id` and register it with `registry` —
/// deliberately `start` BEFORE `register`, the reverse of this module's
/// original order. The original order had a TOCTOU: `register` published a
/// `Running(job_id)` entry before the job existed, so an `assist.cancel`
/// landing in that gap found `Running`, removed the entry, and called
/// `cancel_job` on an id nothing had started yet (a no-op) — the job then
/// started anyway, Running, with no cancel path left. Starting first closes
/// it: a cancel racing this same gap instead finds the `Pending` marker
/// [`AssistStreamRegistry::begin`] already left behind, so `register` below
/// correctly observes [`entry::StreamEntry::CancelledEarly`] and this
/// function cancels the very job it just started before reporting failure.
/// `None` on that race (the caller should treat it as `"Job cancelled"`);
/// `Some(job_id)` otherwise. Safe to cancel unconditionally on the race
/// path — `job_id` is a fresh UUID, so it can never collide with a later,
/// unrelated `start_job` call.
///
/// `gen` is the caller's OWN generation, threaded down from
/// `handle_answer_assist`: [`AssistStreamRegistry::register`] binds the job
/// only while `req_id` is still held by THAT request's own entry. It matters
/// because a request can reach here TWICE — `compose_with_length_retry`'s
/// retry must bind its new job to the SAME entry the first attempt did,
/// never mint a second generation (which would strand the first entry past
/// the request's single `unregister_gen`) and never resurrect an entry a
/// cancel already removed.
///
/// Generic over a combined [`JobStarter`] + [`JobCanceller`] recorder so
/// this ordering is directly unit-testable without a live `AppHandle`. The
/// sole production caller (`compose_draft_stream`) passes a real
/// `&AppHandle` (which implements both).
pub(super) fn start_and_register<T: JobStarter + JobCanceller>(
    starter: &T,
    registry: &AssistStreamRegistry,
    req_id: &str,
    r#gen: u64,
) -> Option<String> {
    let job_id = crate::db::new_job_id();
    starter.start_job(&job_id);
    if !registry.register(req_id, r#gen, &job_id) {
        starter.cancel_job(&job_id);
        return None;
    }
    Some(job_id)
}
