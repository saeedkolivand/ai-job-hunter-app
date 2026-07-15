//! The per-connection `answer.assist` stream registry — the `reqId` state
//! machine ([`StreamEntry`]/[`AssistStreamRegistry`]) plus the two small
//! `AppHandle`-abstracting seams ([`JobCanceller`]/[`JobStarter`]) and
//! [`start_and_register`] that let its start/register/cancel ordering be
//! unit-tested without a live `AppHandle` (this crate has no `tauri::test`
//! mock-app harness). Split out of [`super::stream`] (R8 line-budget split —
//! `stream.rs` orchestrates the writer/read-loop decoupling and the streaming
//! compose call; this module IS the state machine those orchestrate). See
//! [`super::stream`]'s module doc for the full picture this is a piece of
//! (the write-backpressure/stalled-peer fix, the four ways a stream ends
//! early, and the CWE-639 per-connection isolation this registry exists for).
//!
//! [`AssistStreamRegistry`] is re-exported from `stream` as `stream::
//! AssistStreamRegistry` so every existing reference (`mod.rs`,
//! `answer_assist.rs`, and their tests) keeps resolving unchanged.

use std::collections::HashMap;

use parking_lot::Mutex;
use tauri::AppHandle;

/// Abstracts "cancel one job by id" so [`AssistStreamRegistry::cancel`]/
/// [`AssistStreamRegistry::cancel_all`]'s job-cancelling side effect is
/// unit-testable against a fake recorder, without a live `AppHandle` — this
/// crate has no `tauri::test` mock-app harness (mirrors the
/// `SalarySearcher`/`AnswerSearcher` genericization precedent in
/// `answer_assist`/`commands::ai`). The sole production implementor forwards
/// to [`crate::commands::jobs::job_cancel`].
pub(super) trait JobCanceller {
    fn cancel_job(&self, job_id: &str);
}

impl JobCanceller for AppHandle {
    fn cancel_job(&self, job_id: &str) {
        crate::commands::jobs::job_cancel(self, job_id);
    }
}

/// Abstracts "start a job by id" so [`start_and_register`]'s
/// start-before-register ordering is unit-testable against a fake recorder,
/// without a live `AppHandle` — mirrors [`JobCanceller`]'s existing seam. The
/// sole production implementor forwards to [`crate::commands::jobs::job_start`]
/// with this module's one fixed job kind (every job this registry ever tracks
/// is an `"extension.answer_assist"` job).
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
/// landing in that exact gap found `Running`, removed the entry, and called
/// `cancel_job` on an id nothing had started yet (a no-op) — the job then
/// started anyway, Running, with no cancel path left. Starting first closes
/// it: a cancel racing this same gap instead finds the `Pending` marker
/// [`AssistStreamRegistry::begin`] already left behind (set synchronously in
/// `spawn_answer_assist`, before this task was even spawned), so `register`
/// below correctly observes [`StreamEntry::CancelledEarly`] and this function
/// cancels the very job it just started before reporting failure. `None` on
/// that race (the caller should treat it as `"Job cancelled"`); `Some(job_id)`
/// otherwise. Safe to cancel unconditionally on the race path — `job_id` is a
/// fresh UUID ([`crate::db::new_job_id`]), so it can never collide with a
/// later, unrelated `start_job` call. Does NOT need the reqId's generation —
/// `register` reads and preserves whatever generation `begin` already minted.
///
/// Generic over a combined [`JobStarter`] + [`JobCanceller`] recorder (not
/// the concrete `AppHandle`) so this ordering is directly unit-testable
/// without a live `AppHandle` — this crate has no `tauri::test` mock-app
/// harness. The sole production caller (`compose_draft_stream`, in
/// [`super::stream`]) passes a real `&AppHandle` (which implements both).
pub(super) fn start_and_register<T: JobStarter + JobCanceller>(
    starter: &T,
    registry: &AssistStreamRegistry,
    req_id: &str,
) -> Option<String> {
    let job_id = crate::db::new_job_id();
    starter.start_job(&job_id);
    if !registry.register(req_id, &job_id) {
        starter.cancel_job(&job_id);
        return None;
    }
    Some(job_id)
}

/// One `reqId`'s lifecycle in the per-connection registry — from the moment
/// `resolve_answer_assist` starts its pre-compose work (before ANY billable
/// spend) through to either a registered running job or an early
/// cancellation. See [`super::stream`]'s module doc's "An `assist.cancel`
/// races the pre-compose window" case.
///
/// Every variant carries the `reqId`'s **generation** — a per-registry
/// monotonic counter [`AssistStreamRegistry::begin`] mints a fresh value from
/// on every successful call. This is the generation-scoped-removal fix (a
/// security-review + CodeRabbit finding): a client can reuse a `reqId` once
/// its ORIGINAL entry is gone (job cancelled, or the request completed), and
/// without a generation, a delayed cleanup call keyed by `reqId` ALONE could
/// remove the REUSED request's fresh entry instead of the stale one it meant
/// to clean up (e.g. A registers Running, `assist.cancel` removes it, a
/// client reuses the same `reqId` for a brand-new request B which `begin`s +
/// `register`s successfully, and only THEN does A's own tail cleanup run —
/// keyed by `reqId` alone, it would clobber B's entry, leaving B's billable
/// job unreachable/uncancellable). [`AssistStreamRegistry::unregister_gen`]
/// is the fix: it only ever removes an entry whose STORED generation matches
/// the one the caller was handed back by ITS OWN `begin` — B's entry always
/// carries a strictly higher generation than A's, so A's stale cleanup is
/// safely a no-op against it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StreamEntry {
    /// Pre-compose work (gate/resume/limiter/salary/web-notes) is in
    /// flight — no job exists yet.
    Pending(u64),
    /// [`AssistStreamRegistry::register`] recorded its job id.
    Running(u64, String),
    /// An `assist.cancel` arrived while still `Pending` — the pre-compose
    /// caller must short-circuit rather than proceed to the billable
    /// compose call. See [`AssistStreamRegistry::register`]'s return value.
    CancelledEarly(u64),
}

impl StreamEntry {
    /// This entry's generation, regardless of which variant it currently is
    /// — used by [`AssistStreamRegistry::unregister_gen`]'s match-the-caller
    /// check.
    fn gen(&self) -> u64 {
        match self {
            StreamEntry::Pending(g)
            | StreamEntry::Running(g, _)
            | StreamEntry::CancelledEarly(g) => *g,
        }
    }
}

/// The map plus its generation counter, under ONE lock — kept together
/// (rather than a separate `AtomicU64` field) so `begin` mints a fresh
/// generation and inserts `Pending` under the SAME critical section, with no
/// TOCTOU between "read the next generation" and "insert the entry".
#[derive(Default)]
struct RegistryState {
    entries: HashMap<String, StreamEntry>,
    next_gen: u64,
}

/// Per-connection registry of in-flight/pending streaming `answer.assist`
/// requests (`reqId -> `[`StreamEntry`]) — deliberately scoped to ONE
/// connection. See [`super::stream`]'s module doc's "Cancellation is
/// per-connection" section.
#[derive(Default)]
pub(super) struct AssistStreamRegistry(Mutex<RegistryState>);

impl AssistStreamRegistry {
    /// Mark `req_id` as `Pending` BEFORE any pre-compose await (the gate/
    /// resume/limiter/salary/web-notes lookups in `resolve_answer_assist`) —
    /// the realistic window (network round-trips) an `assist.cancel` could
    /// race ahead of [`Self::register`]. Returns `None` (leaving the existing
    /// entry untouched) when `req_id` already names ANY entry on this
    /// connection — `Pending`, `Running`, OR `CancelledEarly` — never
    /// silently overwriting it with a fresh `Pending`. Overwriting a
    /// `Running` entry would orphan its job (still running server-side, but
    /// no longer reachable from this registry, so a later `assist.cancel`
    /// for that reqId could never reach it again). Overwriting a
    /// `CancelledEarly` entry reopens the SAME hole from the other
    /// direction: that marker is NOT settled — it is awaiting consumption by
    /// [`Self::register`] (which sees it, removes it, and reports `false` so
    /// the run that raced the cancel never starts a billable job) or removal
    /// by [`Self::unregister_gen`]/[`Self::cancel_all`]. Allowing a reuse
    /// before that consumption would let a second run's fresh `Pending` slip
    /// in under the same `req_id`; the FIRST (already-cancelled) run's later
    /// `register` call would then see that second run's `Pending` instead of
    /// its own `CancelledEarly` marker and register a billable job anyway —
    /// the cancel guarantee lost. It is always cleared within the original
    /// run's own lifecycle (consumed by `register`, or removed by an
    /// `unregister_gen`/`cancel_all`), so a `req_id` can never get stuck
    /// rejected forever; a well-behaved client uses a fresh reqId per request
    /// anyway.
    ///
    /// Returns `Some(gen)` — a fresh, strictly-monotonic-per-registry
    /// generation, and inserts `Pending(gen)` — only when `req_id` names no
    /// entry at all: a fresh `req_id`, or one that already fully settled and
    /// was removed. The caller MUST hold onto this `gen` and pass it to
    /// [`Self::unregister_gen`] at the end of its own request — see
    /// [`StreamEntry`]'s doc for the clobber this generation exists to close.
    pub(super) fn begin(&self, req_id: &str) -> Option<u64> {
        let mut guard = self.0.lock();
        if guard.entries.contains_key(req_id) {
            return None;
        }
        let gen = guard.next_gen;
        guard.next_gen += 1;
        guard
            .entries
            .insert(req_id.to_string(), StreamEntry::Pending(gen));
        Some(gen)
    }

    /// Register an in-flight stream's `reqId -> jobId`, UNLESS `req_id` was
    /// already marked [`StreamEntry::CancelledEarly`] (an `assist.cancel`
    /// raced the pre-compose window) — in which case this is a no-op and
    /// returns `false`, so the caller aborts BEFORE ever starting the
    /// billable job, rather than registering (and thus only becoming
    /// cancellable from this point forward) a stream the client already
    /// gave up on. Returns `true` otherwise: the normal `Pending` →
    /// `Running` move (preserving the SAME generation `begin` minted — no gen
    /// parameter needed here, it's read off the existing entry), or a caller
    /// that never `begin`s at all (mints a fresh generation on the spot, same
    /// as `begin` would — this only ever happens in tests; every production
    /// caller always `begin`s first).
    pub(super) fn register(&self, req_id: &str, job_id: &str) -> bool {
        let mut guard = self.0.lock();
        if matches!(
            guard.entries.get(req_id),
            Some(StreamEntry::CancelledEarly(_))
        ) {
            guard.entries.remove(req_id);
            return false;
        }
        let gen = match guard.entries.get(req_id) {
            Some(StreamEntry::Pending(gen)) => *gen,
            _ => {
                let gen = guard.next_gen;
                guard.next_gen += 1;
                gen
            }
        };
        guard.entries.insert(
            req_id.to_string(),
            StreamEntry::Running(gen, job_id.to_string()),
        );
        true
    }

    /// Remove `req_id`'s entry ONLY IF its stored generation equals `gen` —
    /// generation-scoped removal, the SOLE way any "end of request" cleanup
    /// may free an entry (see [`StreamEntry`]'s doc for the clobber this
    /// closes). A no-op — never an error — when `req_id` names no entry at
    /// all, OR names one whose generation has already moved on: either
    /// [`Self::cancel`]/[`Self::cancel_all`] already consumed THIS caller's
    /// own entry (a `Running` job cancelled + removed, or a `Pending` →
    /// `CancelledEarly` → later consumed by `register`), or a LATER `begin`
    /// for the same reused `req_id` minted a strictly higher generation — in
    /// either case this call must never remove what it doesn't own.
    pub(super) fn unregister_gen(&self, req_id: &str, gen: u64) {
        let mut guard = self.0.lock();
        if guard.entries.get(req_id).is_some_and(|e| e.gen() == gen) {
            guard.entries.remove(req_id);
        }
    }

    /// Remove + return the RUNNING job registered under `req_id` on THIS
    /// registry — `None` when never registered here, already finished,
    /// still `Pending` (no job yet), or belonging to a DIFFERENT
    /// connection's registry (the CWE-639 case this type exists to close).
    /// Pure (no `AppHandle`, no [`JobCanceller`]), so this security-relevant
    /// property is directly unit-testable. Test-only: [`Self::cancel`] used
    /// to call this (a separate `lock()` acquisition), but that shape was a
    /// TOCTOU (a concurrent `register` could flip `Pending` -> `Running` in
    /// the gap between this method's lock and `cancel`'s own); `cancel` now
    /// inlines the same decision under ONE lock instead, leaving this method
    /// only as a test seam.
    #[cfg(test)]
    pub(super) fn take(&self, req_id: &str) -> Option<String> {
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
    pub(super) fn contains(&self, req_id: &str) -> bool {
        self.0.lock().entries.contains_key(req_id)
    }

    /// Cancel the stream named by `req_id` on THIS registry, if any. A
    /// `Running` entry is job-cancelled via `canceller` (the SAME mechanism
    /// `chat_stream`'s `is_cancelled` polls every chunk, so the provider
    /// call itself stops on its next read) and forgotten. A still-`Pending`
    /// entry (no job yet — the pre-compose window) is marked
    /// [`StreamEntry::CancelledEarly`] instead (preserving its generation),
    /// so [`Self::register`] reports `false` once the pre-compose caller
    /// reaches it. A no-op when `req_id` names nothing on this connection at
    /// all. Always removes/cancels whatever CURRENTLY holds `req_id`
    /// regardless of generation — a cancel targets the current holder, not a
    /// specific generation (only the end-of-request cleanup path,
    /// [`Self::unregister_gen`], is generation-scoped). Generic over
    /// [`JobCanceller`] (not the concrete `AppHandle`) so this is
    /// unit-testable against a fake recorder — the sole production caller
    /// passes a real `&AppHandle` (which implements it).
    pub(super) fn cancel<C: JobCanceller>(&self, canceller: &C, req_id: &str) {
        // The whole "is it Running, or still Pending, or neither" decision
        // happens under ONE lock acquisition — splitting it into `take`
        // (its own lock) followed by a second, separate `self.0.lock()` (the
        // original shape) leaves a gap between the two where a concurrent
        // `register` can flip `Pending` -> `Running` on the multi-threaded
        // runtime, so this call would see neither case and silently miss
        // the cancel (TOCTOU). Same per-variant behavior as before, just
        // decided in one critical section.
        let job_id = {
            let mut guard = self.0.lock();
            match guard.entries.get(req_id) {
                Some(StreamEntry::Running(..)) => match guard.entries.remove(req_id) {
                    Some(StreamEntry::Running(_, job_id)) => Some(job_id),
                    _ => None,
                },
                Some(StreamEntry::Pending(gen)) => {
                    let gen = *gen;
                    guard
                        .entries
                        .insert(req_id.to_string(), StreamEntry::CancelledEarly(gen));
                    None
                }
                _ => None,
            }
        };
        if let Some(job_id) = job_id {
            canceller.cancel_job(&job_id);
        }
    }

    /// Cancel EVERY stream currently registered on THIS connection's
    /// registry — called once the connection's read loop exits (socket
    /// closed/errored) so a client disconnect stops every billable
    /// generation still running for it, not just the one an explicit
    /// `assist.cancel` might have named (the CWE-639 fix stays: this only
    /// ever touches THIS connection's own map). A `Running` entry is
    /// cancelled via `canceller`; a still-`Pending` entry is marked
    /// `CancelledEarly` (mirrors [`Self::cancel`]'s `Pending` arm, preserving
    /// its generation) so in-flight pre-compose work also short-circuits
    /// instead of reaching a now-pointless billable compose call. Generic
    /// over [`JobCanceller`] — see [`Self::cancel`]'s doc.
    pub(super) fn cancel_all<C: JobCanceller>(&self, canceller: &C) {
        let mut guard = self.0.lock();
        let drained: Vec<(String, StreamEntry)> = guard.entries.drain().collect();
        let mut running = Vec::new();
        for (req_id, entry) in drained {
            match entry {
                StreamEntry::Running(_, job_id) => running.push(job_id),
                // Exhaustive on purpose: BOTH still-pending AND
                // already-cancelled-early entries must be reinserted as
                // `CancelledEarly` (preserving their generation). Dropping
                // the latter (the original bug) loses the guard marker on a
                // cancel-then-disconnect during the pre-compose window — the
                // entry vanishes from the map, so the later `register` call
                // for that `req_id` finds nothing, returns `true`, and starts
                // a full billable generation for a request the user already
                // cancelled.
                StreamEntry::Pending(gen) | StreamEntry::CancelledEarly(gen) => {
                    guard
                        .entries
                        .insert(req_id, StreamEntry::CancelledEarly(gen));
                }
            }
        }
        drop(guard);
        for job_id in running {
            canceller.cancel_job(&job_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── AssistStreamRegistry: register / take / unregister_gen ──────────────

    #[test]
    fn register_then_take_returns_and_forgets_it() {
        let r = AssistStreamRegistry::default();
        assert!(r.register("req-1", "job-1"));
        assert_eq!(r.take("req-1"), Some("job-1".to_string()));
        assert_eq!(r.take("req-1"), None, "take also forgets the mapping");
    }

    #[test]
    fn unregister_gen_on_an_unknown_req_id_is_a_no_op() {
        let r = AssistStreamRegistry::default();
        r.unregister_gen("never-registered", 0); // must not panic
        assert_eq!(r.take("never-registered"), None);
    }

    #[test]
    fn register_overwrites_a_prior_mapping_for_the_same_req_id() {
        let r = AssistStreamRegistry::default();
        assert!(r.register("req-1", "job-1"));
        assert!(r.register("req-1", "job-2"));
        assert_eq!(
            r.take("req-1"),
            Some("job-2".to_string()),
            "a re-registration under the same reqId must replace, not duplicate"
        );
    }

    #[test]
    fn unregister_gen_then_register_again_reflects_the_new_mapping() {
        let r = AssistStreamRegistry::default();
        let gen = r.begin("req-1").expect("a fresh reqId");
        assert!(r.register("req-1", "job-1"));
        r.unregister_gen("req-1", gen);
        assert!(r.register("req-1", "job-2"));
        assert_eq!(r.take("req-1"), Some("job-2".to_string()));
    }

    // ── CWE-639 regression: a different connection's registry can never see
    // (let alone cancel) another connection's stream ──────────────────────

    #[test]
    fn take_on_a_different_connections_registry_never_sees_another_connections_stream() {
        // Two independent registries — one per connection, exactly as
        // `handle_connection` creates a fresh one per socket.
        let connection_a = AssistStreamRegistry::default();
        let connection_b = AssistStreamRegistry::default();

        connection_a.register("req-1", "job-1");

        // Connection B never registered "req-1" — it must be a no-op, NEVER
        // able to observe (let alone cancel) connection A's stream.
        assert_eq!(
            connection_b.take("req-1"),
            None,
            "a different connection's registry must never see this reqId"
        );

        // Connection A can still cancel its own stream — the isolation is
        // per-connection, not "nobody can ever cancel it".
        assert_eq!(connection_a.take("req-1"), Some("job-1".to_string()));
    }

    // ── AssistStreamRegistry::cancel / cancel_all (JobCanceller-generic —
    // testable without a live AppHandle; this crate has no tauri::test
    // mock-app harness) ─────────────────────────────────────────────────────

    #[derive(Default)]
    struct RecordingCanceller {
        cancelled: std::cell::RefCell<Vec<String>>,
    }

    impl JobCanceller for RecordingCanceller {
        fn cancel_job(&self, job_id: &str) {
            self.cancelled.borrow_mut().push(job_id.to_string());
        }
    }

    #[test]
    fn cancel_on_an_unknown_req_id_never_touches_the_canceller() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.cancel(&canceller, "never-registered");
        assert!(canceller.cancelled.borrow().is_empty());
    }

    #[test]
    fn cancel_on_a_running_req_id_cancels_its_job_and_forgets_the_mapping() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.register("req-1", "job-1");
        r.cancel(&canceller, "req-1");
        assert_eq!(canceller.cancelled.into_inner(), vec!["job-1".to_string()]);
        assert_eq!(r.take("req-1"), None, "cancel also forgets the mapping");
    }

    #[test]
    fn cancel_all_cancels_every_running_stream_and_leaves_pending_alone_besides_marking_it() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.register("req-1", "job-1");
        r.register("req-2", "job-2");
        r.begin("req-3"); // still pending — no job to cancel
        r.cancel_all(&canceller);

        let mut got = canceller.cancelled.into_inner();
        got.sort();
        assert_eq!(
            got,
            vec!["job-1".to_string(), "job-2".to_string()],
            "only RUNNING entries are ever job-cancelled"
        );
        // The pending entry is now cancelled-early — a still in-flight
        // pre-compose caller must never be allowed to register a job for it.
        assert!(!r.register("req-3", "job-3"));
    }

    #[test]
    fn cancel_all_on_an_empty_registry_is_a_no_op() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.cancel_all(&canceller);
        assert!(canceller.cancelled.into_inner().is_empty());
    }

    #[test]
    fn cancel_all_preserves_an_already_cancelled_early_entry() {
        // HIGH regression: cancel-then-disconnect during the pre-compose
        // window. `begin` + `cancel` leaves `req-1` as `CancelledEarly`
        // BEFORE `cancel_all` ever runs; `cancel_all` must reinsert it
        // (not drop it on the floor), or the later `register` call for the
        // same `req_id` finds nothing, returns `true`, and starts a full
        // billable generation for a request the user already cancelled.
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.begin("req-1");
        r.cancel(&canceller, "req-1"); // -> CancelledEarly, no job existed yet
        r.cancel_all(&canceller);

        assert!(
            canceller.cancelled.borrow().is_empty(),
            "no Running job existed at any point — nothing to job_cancel"
        );
        assert!(
            !r.register("req-1", "job-1"),
            "the CancelledEarly guard must survive cancel_all's drain-and-reinsert"
        );
    }

    // ── Pre-registration cancel race (LOW fix): begin()'s Option<u64> + register()'s bool ─

    #[test]
    fn cancel_during_the_pending_window_prevents_the_later_register_call() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.begin("req-1"); // pre-compose work has started — no job yet
        r.cancel(&canceller, "req-1"); // assist.cancel races the awaits

        assert!(
            canceller.cancelled.borrow().is_empty(),
            "no job exists yet — there is nothing to job_cancel"
        );
        assert!(
            !r.register("req-1", "job-1"),
            "the compose call must never start a billable job for an early-cancelled reqId"
        );
        assert_eq!(
            r.take("req-1"),
            None,
            "the cancelled-early marker must never surface as a real job"
        );
    }

    #[test]
    fn register_without_a_prior_cancel_succeeds_normally_after_begin() {
        let r = AssistStreamRegistry::default();
        r.begin("req-1");
        assert!(r.register("req-1", "job-1"));
        assert_eq!(r.take("req-1"), Some("job-1".to_string()));
    }

    // ── Duplicate reqId rejection (MEDIUM fix): begin() on an already-active
    // entry must never orphan the original job ─────────────────────────────

    #[test]
    fn begin_on_a_fresh_req_id_succeeds() {
        let r = AssistStreamRegistry::default();
        assert!(r.begin("req-1").is_some());
    }

    #[test]
    fn begin_on_an_already_pending_req_id_is_rejected() {
        let r = AssistStreamRegistry::default();
        r.begin("req-1"); // first request's pre-compose window is in flight

        assert!(
            r.begin("req-1").is_none(),
            "a second begin for the same still-Pending reqId must be rejected"
        );
    }

    #[test]
    fn begin_on_an_already_running_req_id_is_rejected_and_the_original_stays_cancellable() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        assert!(r.register("req-1", "job-1")); // the original request is now Running

        // A client reusing the SAME reqId while the original is still
        // running must be rejected — never silently overwrite the Running
        // entry with a fresh Pending, which would orphan job-1 (still
        // running server-side, but no longer reachable to cancel).
        assert!(r.begin("req-1").is_none());

        r.cancel(&canceller, "req-1");
        assert_eq!(
            canceller.cancelled.into_inner(),
            vec!["job-1".to_string()],
            "the original job must still be there and cancellable after the rejected begin"
        );
    }

    #[test]
    fn begin_on_a_cancelled_early_req_id_is_rejected() {
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.begin("req-1");
        r.cancel(&canceller, "req-1"); // -> CancelledEarly, no job existed yet

        assert!(
            r.begin("req-1").is_none(),
            "a CancelledEarly marker is not settled — reuse must be rejected \
             until register (or cancel_all) consumes it"
        );
    }

    #[test]
    fn begin_on_a_cancelled_early_req_id_is_rejected_until_register_consumes_it() {
        // Full spend/cancel-integrity guarantee: a run that reused req-1
        // before the CancelledEarly marker was consumed used to be able to
        // slip a fresh Pending in, which let the FIRST (already-cancelled)
        // run's later `register` call see that Pending instead of its own
        // marker and start a billable job anyway — the exact hole this fix
        // closes.
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        r.begin("req-1"); // run A's pre-compose window opens
        r.cancel(&canceller, "req-1"); // -> CancelledEarly, run A has no job yet

        assert!(
            r.begin("req-1").is_none(),
            "a second run reusing req-1 must be rejected while the marker is un-consumed"
        );
        assert!(
            !r.register("req-1", "job-a"),
            "register consumes the CancelledEarly marker and reports false — \
             run A never starts a billable job"
        );
        assert!(
            r.begin("req-1").is_some(),
            "once consumed, req-1 names no entry at all — reuse is allowed again"
        );
    }

    // ── Generation-scoped removal (security-review + CodeRabbit fix):
    // unregister_gen must never clobber a reused reqId's successor entry ────

    #[test]
    fn unregister_gen_never_clobbers_a_reused_req_ids_successor_entry() {
        // The exact clobber this generation token closes: A registers
        // Running, an `assist.cancel` removes A's entry (cancelling job-a),
        // then a client reuses the SAME reqId for a brand-new request B —
        // B's `begin`/`register` succeed with a STRICTLY HIGHER generation.
        // A's own end-of-request cleanup then arrives LATE (after B has
        // already registered) — keyed by reqId alone, the old unconditional
        // `unregister` would have clobbered B's fresh entry here.
        let r = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();

        let gen_a = r
            .begin("req-x")
            .expect("A's begin succeeds on a fresh reqId");
        assert!(r.register("req-x", "job-a"));
        r.cancel(&canceller, "req-x"); // removes A's Running entry, cancels job-a

        let gen_b = r
            .begin("req-x")
            .expect("B may reuse req-x once A's entry is gone");
        assert!(
            gen_b > gen_a,
            "B's generation must be strictly higher than A's"
        );
        assert!(r.register("req-x", "job-b"));

        // A's tail cleanup arrives LATE — after B has already begun and
        // registered — the exact race the generation token exists to close.
        r.unregister_gen("req-x", gen_a);

        assert!(
            r.contains("req-x"),
            "A's stale, lower-generation cleanup must never remove B's fresh entry"
        );

        // B's job is still fully reachable AND cancellable — never clobbered.
        r.cancel(&canceller, "req-x");
        assert_eq!(
            canceller.cancelled.into_inner(),
            vec!["job-a".to_string(), "job-b".to_string()],
            "B's job must still be cancellable after A's stale cleanup ran"
        );
    }

    #[test]
    fn unregister_gen_removes_the_entry_when_the_generation_still_matches() {
        // The normal (non-reused-reqId) case: the caller's own gen still
        // names the SAME entry it was handed for, so unregister_gen must
        // actually remove it — this is the "one owner cleans up its own
        // request" path every non-race request takes.
        let r = AssistStreamRegistry::default();
        let gen = r.begin("req-1").expect("a fresh reqId");
        assert!(r.register("req-1", "job-1"));

        r.unregister_gen("req-1", gen);

        assert!(
            !r.contains("req-1"),
            "the matching generation must remove the entry"
        );
    }

    // ── start_and_register (HIGH fix: job_start-before-register TOCTOU —
    // starting the job before registering it means a cancel racing the gap
    // finds Pending, not a not-yet-existing Running job) ───────────────────

    #[derive(Default)]
    struct RecordingStarterCanceller {
        started: std::cell::RefCell<Vec<String>>,
        cancelled: std::cell::RefCell<Vec<String>>,
    }

    impl JobStarter for RecordingStarterCanceller {
        fn start_job(&self, job_id: &str) {
            self.started.borrow_mut().push(job_id.to_string());
        }
    }

    impl JobCanceller for RecordingStarterCanceller {
        fn cancel_job(&self, job_id: &str) {
            self.cancelled.borrow_mut().push(job_id.to_string());
        }
    }

    #[test]
    fn start_and_register_starts_the_job_then_registers_it_on_the_happy_path() {
        let registry = AssistStreamRegistry::default();
        let recorder = RecordingStarterCanceller::default();

        let job_id = start_and_register(&recorder, &registry, "req-1")
            .expect("a fresh reqId with no prior cancel must register successfully");

        assert_eq!(
            recorder.started.into_inner(),
            vec![job_id.clone()],
            "job_start must have run, unconditionally, before register was ever consulted"
        );
        assert!(
            recorder.cancelled.borrow().is_empty(),
            "a successful register must never cancel the job it just started"
        );
        assert_eq!(
            registry.take("req-1"),
            Some(job_id),
            "register must have recorded the Running entry"
        );
    }

    #[test]
    fn start_and_register_cancels_the_just_started_job_when_a_cancel_already_raced_ahead() {
        // The exact TOCTOU this reorder closes: an `assist.cancel` that
        // arrived during the pre-compose window (captured here as a
        // pre-seeded `CancelledEarly` marker — see `AssistStreamRegistry::
        // begin`/`cancel`) must never leave a job that's Running but neither
        // cancelled nor cancellable. `job_start` still runs — unconditionally,
        // BEFORE `register` is ever consulted, proving the new order — but
        // `register` then reports the race, and this function must cancel
        // the very job it just started rather than leaving it orphaned.
        let registry = AssistStreamRegistry::default();
        let canceller = RecordingCanceller::default();
        registry.begin("req-1");
        registry.cancel(&canceller, "req-1"); // -> CancelledEarly, no job existed yet

        let recorder = RecordingStarterCanceller::default();
        let result = start_and_register(&recorder, &registry, "req-1");

        assert!(
            result.is_none(),
            "a raced-ahead cancel must make start_and_register report failure"
        );
        assert_eq!(
            recorder.started.borrow().len(),
            1,
            "job_start must still have run — it happens BEFORE register is ever consulted"
        );
        let started_id = recorder.started.borrow()[0].clone();
        assert_eq!(
            recorder.cancelled.into_inner(),
            vec![started_id],
            "the job just started must be job-cancelled immediately — no leaked Running job"
        );
        assert!(
            !registry.contains("req-1"),
            "the CancelledEarly marker must be consumed, not left behind"
        );
    }
}
