//! [`AssistStreamRegistry`] — the begin/register/cancel state machine over
//! [`super::entry::StreamEntry`]. Split out of the parent module for the R8
//! line-budget cap.

use parking_lot::Mutex;

use super::entry::{RegistryState, StreamEntry};
use super::JobCanceller;

/// Per-connection registry of in-flight/pending streaming `answer.assist`
/// requests (`reqId -> `[`StreamEntry`]) — deliberately scoped to ONE
/// connection. See [`super::super::stream`]'s module doc's "Cancellation is
/// per-connection" section.
#[derive(Default)]
pub(in crate::extension_bridge) struct AssistStreamRegistry(Mutex<RegistryState>);

impl AssistStreamRegistry {
    /// Mark `req_id` as `Pending` BEFORE any pre-compose await (the gate/
    /// resume/limiter/salary/web-notes lookups in `resolve_answer_assist`) —
    /// the realistic window (network round-trips) an `assist.cancel` could
    /// race ahead of [`Self::register`]. Returns `None` (leaving the existing
    /// entry untouched) when `req_id` already names ANY entry — `Pending`,
    /// `Running`, OR `CancelledEarly` — never silently overwriting it.
    /// Overwriting `Running` would orphan its job (no longer reachable from
    /// this registry, so a later `assist.cancel` could never reach it).
    /// Overwriting `CancelledEarly` reopens the same hole from the other
    /// direction: that marker is awaiting consumption by [`Self::register`]
    /// (which removes it and reports `false`, so the raced run never starts a
    /// billable job) or removal by [`Self::unregister_gen`]/[`Self::cancel_all`];
    /// letting a second run's fresh `Pending` slip in under the same `req_id`
    /// before that consumption would make the FIRST run's later `register`
    /// see the second run's `Pending` and register a billable job anyway —
    /// the cancel guarantee lost. It always clears within the original run's
    /// own lifecycle, so a `req_id` can never get stuck rejected forever.
    ///
    /// Returns `Some(gen)` — a fresh, strictly-monotonic-per-registry
    /// generation, and inserts `Pending(gen)` — only when `req_id` names no
    /// entry at all. The caller MUST hold onto this `gen` and pass it to
    /// [`Self::unregister_gen`] at the end of its own request — see
    /// [`StreamEntry`]'s doc for the clobber this generation exists to close.
    pub(in crate::extension_bridge) fn begin(&self, req_id: &str) -> Option<u64> {
        let mut guard = self.0.lock();
        if guard.entries.contains_key(req_id) {
            return None;
        }
        let r#gen = guard.next_gen;
        guard.next_gen += 1;
        guard
            .entries
            .insert(req_id.to_string(), StreamEntry::Pending(r#gen));
        Some(r#gen)
    }

    /// Bind `req_id`'s entry to `job_id`, generation-scoped: it succeeds ONLY
    /// while `req_id` is still held by the CALLER'S OWN entry — `Pending(gen)`
    /// (the first attempt's normal `Pending` → `Running` move) or
    /// `Running(gen, _)` (a second attempt rebinding onto its fresh job; see
    /// [`super::start_and_register`]'s doc for why one request can register
    /// twice). Either way the generation is PRESERVED, never re-minted, so
    /// the request's single [`Self::unregister_gen`] at the end still frees
    /// the entry.
    ///
    /// Returns `false`, touching nothing, in every other case, and the caller
    /// must abort BEFORE running its billable stream:
    ///
    /// * `CancelledEarly(gen)` — an `assist.cancel` raced the pre-compose
    ///   window. Consumed (removed) here, only when it carries the caller's
    ///   own generation.
    /// * no entry at all — `cancel`/`cancel_all` already removed this
    ///   request's `Running` entry. Re-registering would resurrect a billable
    ///   stream for a request the client already gave up on.
    /// * an entry carrying a DIFFERENT generation — a reused `reqId`'s
    ///   successor owns the key now; this caller must never clobber it.
    pub(in crate::extension_bridge) fn register(
        &self,
        req_id: &str,
        r#gen: u64,
        job_id: &str,
    ) -> bool {
        let mut guard = self.0.lock();
        match guard.entries.get(req_id) {
            Some(StreamEntry::Pending(g) | StreamEntry::Running(g, _)) if *g == r#gen => {
                guard.entries.insert(
                    req_id.to_string(),
                    StreamEntry::Running(r#gen, job_id.to_string()),
                );
                true
            }
            Some(StreamEntry::CancelledEarly(g)) if *g == r#gen => {
                guard.entries.remove(req_id);
                false
            }
            _ => false,
        }
    }

    /// Whether `req_id` is STILL held by this caller's own running entry
    /// (`Running(gen, _)`) — the guard `compose_with_length_retry` checks
    /// before paying for a second round-trip. `false` means the client gave
    /// up between the two attempts ([`Self::cancel`] removed the entry, or
    /// [`Self::cancel_all`] drained it on disconnect). [`Self::register`]
    /// re-checks the same ownership atomically under its own lock, so this is
    /// a spend guard (skip the charge), not the correctness boundary.
    pub(in crate::extension_bridge) fn holds_running_gen(&self, req_id: &str, r#gen: u64) -> bool {
        matches!(
            self.0.lock().entries.get(req_id),
            Some(StreamEntry::Running(g, _)) if *g == r#gen
        )
    }

    /// Whether `req_id`'s entry was cancelled while still `Pending` (an `assist.cancel` — or the
    /// whole connection closing via [`Self::cancel_all`] — that raced ahead of the FIRST
    /// [`Self::register`] call). A non-consuming PEEK: `register` still reaches the marker
    /// afterwards and consumes it exactly as before. Exists so a caller doing BILLABLE
    /// pre-register grounding (see `answer_assist_topic::research_company_brief`) can skip the
    /// spend instead of only discovering the cancel once `register` finally runs deep inside
    /// `compose_draft_stream` — the same spend-guard role [`Self::holds_running_gen`] plays for a
    /// retry charge.
    pub(in crate::extension_bridge) fn is_cancelled_early(&self, req_id: &str, r#gen: u64) -> bool {
        matches!(
            self.0.lock().entries.get(req_id),
            Some(StreamEntry::CancelledEarly(g)) if *g == r#gen
        )
    }

    /// Remove `req_id`'s entry ONLY IF its stored generation equals `gen` —
    /// generation-scoped removal, the SOLE way any "end of request" cleanup
    /// may free an entry (see [`StreamEntry`]'s doc for the clobber this
    /// closes). A no-op — never an error — when `req_id` names no entry at
    /// all, OR one whose generation has already moved on: either
    /// [`Self::cancel`]/[`Self::cancel_all`] already consumed THIS caller's
    /// own entry, or a LATER `begin` for the same reused `req_id` minted a
    /// strictly higher generation — either way this call must never remove
    /// what it doesn't own.
    pub(in crate::extension_bridge) fn unregister_gen(&self, req_id: &str, r#gen: u64) {
        let mut guard = self.0.lock();
        if guard
            .entries
            .get(req_id)
            .is_some_and(|e| e.r#gen() == r#gen)
        {
            guard.entries.remove(req_id);
        }
    }

    /// Remove + return the RUNNING job registered under `req_id` on THIS
    /// registry — `None` when never registered here, already finished,
    /// still `Pending` (no job yet), or belonging to a DIFFERENT
    /// connection's registry (the CWE-639 case this type exists to close).
    /// Test-only: [`Self::cancel`] used to call this (a separate `lock()`),
    /// which was a TOCTOU (a concurrent `register` could flip `Pending` ->
    /// `Running` in the gap); `cancel` now inlines the same decision under
    /// ONE lock, leaving this method only as a test seam.
    #[cfg(test)]
    pub(in crate::extension_bridge) fn take(&self, req_id: &str) -> Option<String> {
        let mut guard = self.0.lock();
        // Checked BEFORE removing — a naive unconditional `remove` would
        // destroy a `Pending`/`CancelledEarly` entry it isn't actually
        // returning, silently losing that state for anyone who checks
        // `req_id` afterward (this was a real bug caught by this file's own
        // pre-registration-race test).
        match guard.entries.get(req_id) {
            Some(StreamEntry::Running(..)) => match guard.entries.remove(req_id) {
                Some(StreamEntry::Running(_, job_id)) => Some(job_id),
                _ => None,
            },
            _ => None,
        }
    }

    /// Test-only seam: whether ANY entry (`Pending`, `Running`, or
    /// `CancelledEarly`) exists for `req_id` — unlike [`Self::take`] (which
    /// only ever observes a `Running` job), this is what a leak-detection
    /// test needs to assert a `Pending` entry was actually removed, not just
    /// left un-taken.
    #[cfg(test)]
    pub(in crate::extension_bridge) fn contains(&self, req_id: &str) -> bool {
        self.0.lock().entries.contains_key(req_id)
    }

    /// Cancel the stream named by `req_id` on THIS registry, if any. A
    /// `Running` entry is job-cancelled via `canceller` (the SAME mechanism
    /// `chat_stream`'s `is_cancelled` polls every chunk) and forgotten. A
    /// still-`Pending` entry (no job yet) is marked
    /// [`StreamEntry::CancelledEarly`] instead (preserving its generation),
    /// so [`Self::register`] reports `false` once the pre-compose caller
    /// reaches it. A no-op when `req_id` names nothing here. Always
    /// removes/cancels whatever CURRENTLY holds `req_id` regardless of
    /// generation — only [`Self::unregister_gen`] is generation-scoped.
    /// Generic over [`JobCanceller`] so this is unit-testable against a fake
    /// recorder.
    pub(in crate::extension_bridge) fn cancel<C: JobCanceller>(&self, canceller: &C, req_id: &str) {
        // Decided under ONE lock acquisition: splitting this into `take` (its
        // own lock) then a second `self.0.lock()` leaves a gap where a
        // concurrent `register` can flip `Pending` -> `Running`, silently
        // missing the cancel (TOCTOU).
        let job_id = {
            let mut guard = self.0.lock();
            match guard.entries.get(req_id) {
                Some(StreamEntry::Running(..)) => match guard.entries.remove(req_id) {
                    Some(StreamEntry::Running(_, job_id)) => Some(job_id),
                    _ => None,
                },
                Some(StreamEntry::Pending(r#gen)) => {
                    let r#gen = *r#gen;
                    guard
                        .entries
                        .insert(req_id.to_string(), StreamEntry::CancelledEarly(r#gen));
                    None
                }
                _ => None,
            }
        };
        if let Some(job_id) = job_id {
            canceller.cancel_job(&job_id);
        }
    }

    /// Cancel EVERY stream registered on THIS connection's registry — called
    /// once the connection's read loop exits (socket closed/errored) so a
    /// client disconnect stops every billable generation still running for
    /// it, not just one an explicit `assist.cancel` named (this only ever
    /// touches THIS connection's own map — the CWE-639 fix). A `Running`
    /// entry is cancelled via `canceller`; a still-`Pending` entry is marked
    /// `CancelledEarly` (mirrors [`Self::cancel`]'s `Pending` arm) so
    /// in-flight pre-compose work also short-circuits.
    pub(in crate::extension_bridge) fn cancel_all<C: JobCanceller>(&self, canceller: &C) {
        let mut guard = self.0.lock();
        let drained: Vec<(String, StreamEntry)> = guard.entries.drain().collect();
        let mut running = Vec::new();
        for (req_id, entry) in drained {
            match entry {
                StreamEntry::Running(_, job_id) => running.push(job_id),
                // Exhaustive on purpose: an already-`CancelledEarly` entry
                // must be reinserted too, not dropped — dropping it (the
                // original bug) loses the guard marker on a
                // cancel-then-disconnect during the pre-compose window, so
                // the later `register` call finds nothing, returns `true`,
                // and starts a full billable generation for a request the
                // user already cancelled.
                StreamEntry::Pending(r#gen) | StreamEntry::CancelledEarly(r#gen) => {
                    guard
                        .entries
                        .insert(req_id, StreamEntry::CancelledEarly(r#gen));
                }
            }
        }
        drop(guard);
        for job_id in running {
            canceller.cancel_job(&job_id);
        }
    }
}
