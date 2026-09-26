//! [`StreamEntry`]'s generation-tagged lifecycle variants + the per-registry
//! state they're grouped under.

use std::collections::HashMap;

/// One `reqId`'s lifecycle in the per-connection registry — from the moment
/// `resolve_answer_assist` starts its pre-compose work (before ANY billable
/// spend) through to either a registered running job or an early
/// cancellation. See [`super::super::stream`]'s module doc's "An `assist.cancel`
/// races the pre-compose window" case.
///
/// Every variant carries the `reqId`'s **generation** — a per-registry
/// monotonic counter [`super::registry::AssistStreamRegistry::begin`] mints a
/// fresh value from on every successful call. This is the generation-scoped-
/// removal fix: a client can reuse a `reqId` once its ORIGINAL entry is gone
/// (job cancelled, or the request completed), and without a generation, a
/// delayed cleanup call keyed by `reqId` ALONE could remove the REUSED
/// request's fresh entry instead of the stale one it meant to clean up (A
/// registers Running, `assist.cancel` removes it, a client reuses the same
/// `reqId` for a fresh request B which `begin`s + `register`s successfully,
/// and only THEN does A's own tail cleanup run — keyed by `reqId` alone, it
/// would clobber B's entry). [`super::registry::AssistStreamRegistry::unregister_gen`]
/// is the fix: it only ever removes an entry whose STORED generation matches
/// the one the caller's OWN `begin` handed back — B's entry always carries a
/// strictly higher generation than A's, so A's stale cleanup is a safe no-op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum StreamEntry {
    /// Pre-compose work (gate/resume/limiter/salary/web-notes) is in
    /// flight — no job exists yet.
    Pending(u64),
    /// [`super::registry::AssistStreamRegistry::register`] recorded its job id.
    Running(u64, String),
    /// An `assist.cancel` arrived while still `Pending` — the pre-compose
    /// caller must short-circuit rather than proceed to the billable
    /// compose call. See [`super::registry::AssistStreamRegistry::register`]'s return value.
    CancelledEarly(u64),
}

impl StreamEntry {
    /// This entry's generation, regardless of which variant it currently is
    /// — used by [`super::registry::AssistStreamRegistry::unregister_gen`]'s match-the-caller
    /// check.
    pub(super) fn r#gen(&self) -> u64 {
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
pub(super) struct RegistryState {
    pub(super) entries: HashMap<String, StreamEntry>,
    pub(super) next_gen: u64,
}
