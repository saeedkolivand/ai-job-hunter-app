//! In-memory anti-abuse limiter for paid / expensive commands.
//!
//! Guards `ai_generate`, `ai_lookup_salary`, `ai_research_company`,
//! `ai_research_answer`, `scrape_board`, and `scrape_url` against a looping (or
//! XSS'd) renderer driving unbounded paid-API spend or
//! scrape abuse — today only autopilot is wall-clock bounded, so a direct IPC
//! loop has no ceiling.
//!
//! Three independent guards, all process-local and reset on restart:
//!
//! 1. **Sliding-window request-rate cap** — at most `max_requests` accepted
//!    starts of a given command within the last [`RATE_WINDOW`]. Old timestamps
//!    age out, so it is a true rolling window, not a fixed bucket.
//! 2. **Concurrency cap** — at most `max_concurrent` in-flight calls of a command,
//!    held as an RAII [`ConcurrencyGuard`] so a panicking / early-returning
//!    handler can never leak a slot. Two admission styles share one cap:
//!    [`Limiter::acquire`] REJECTS when the cap is full, [`Limiter::acquire_queued`]
//!    WAITS. Both draw on the same per-command semaphore, so a command using
//!    both styles still has exactly one concurrency budget.
//! 3. **Per-provider daily request ceiling** — a generous runaway-cost backstop on
//!    total accepted AI requests per provider per UTC day.
//!
//! Defaults are intentionally **generous** so normal interactive use never trips
//! them; they exist to stop pathological loops, not to throttle a human.
//!
//! The whole limiter lives in Tauri managed state as `Arc<Limiter>`; the guard
//! owns its semaphore permit outright, so the slot is released even if the
//! command's managed-state handle is gone.
//!
//! ## Known follow-ups (intentionally out of scope here)
//! * Token-exact / cost-exact accounting (this counts *requests*, not tokens).
//! * A settings UI to configure the caps (today they are fixed constants).
//! * Persistence across restart (the daily counter resets on every launch).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::error::AppError;

// ── Tuning constants ────────────────────────────────────────────────────────
//
// GENEROUS by design — a human clicking "Generate" or "Scrape" repeatedly stays
// well under these; only a runaway loop trips them.

/// `ai_generate`: at most this many starts per [`RATE_WINDOW`].
pub const AI_GENERATE_RATE_MAX: usize = 20;
/// `ai_generate`: at most this many in-flight at once.
pub const AI_GENERATE_CONCURRENCY_MAX: usize = 3;
/// `ai_generate`: at most this many callers PARKED waiting for a slot (see
/// [`Limiter::acquire_queued`]). Tailoring one job fires three sequential
/// generations, so a user working through a batch of imports legitimately has
/// several runs outstanding; a runaway loop is what this stops. Beyond the cap
/// the caller is rejected rather than parked, so the queue itself can never
/// become the unbounded resource.
pub const AI_GENERATE_QUEUE_MAX: usize = 20;

/// The `"ai_research"` command bucket: shared by every web-research lookup
/// (`ai_lookup_salary`, `ai_research_company`, and `ai_research_answer`) so
/// they share one rate + concurrency ceiling instead of each needing its own
/// tuning. At most this many starts per [`RATE_WINDOW`].
pub const AI_RESEARCH_RATE_MAX: usize = 20;
/// `"ai_research"` bucket: at most this many in-flight at once.
pub const AI_RESEARCH_CONCURRENCY_MAX: usize = 3;

/// `scrape_board` / `scrape_url`: at most this many starts per [`RATE_WINDOW`].
pub const SCRAPE_RATE_MAX: usize = 30;
/// `scrape_board` / `scrape_url`: at most this many in-flight at once.
pub const SCRAPE_CONCURRENCY_MAX: usize = 2;

/// `agent_run` (the Phase-2 agentic loop command): at most this many starts per
/// [`RATE_WINDOW`]. One run fans out into several provider requests (each turn is
/// separately charged against the per-provider daily ceiling), so admit fewer
/// runs than a single-shot `ai_generate`.
pub const AGENT_RUN_RATE_MAX: usize = 10;
/// `agent_run`: at most this many in-flight at once.
pub const AGENT_RUN_CONCURRENCY_MAX: usize = 2;

/// Rolling rate-limit window (all commands share the window length; only the
/// per-command count differs).
pub const RATE_WINDOW: Duration = Duration::from_secs(60);

/// Generous per-provider per-UTC-day ceiling on accepted AI requests — a coarse
/// runaway-cost backstop, not a billing-accurate budget.
pub const PROVIDER_DAILY_MAX: u32 = 4_000;

// ── Limiter state ─────────────────────────────────────────────────────────────

/// Sliding-window + queue state for a single command key. Concurrency itself is
/// held by [`CommandState::gate`], not a counter — see [`Limiter::acquire`].
struct CommandState {
    /// Accepted start instants within the current window (oldest at the front).
    recent: VecDeque<Instant>,
    /// The command's concurrency budget. Created on first use with that call's
    /// `max_concurrent`; every call site passes a per-command constant, so the
    /// first value is the only value. A later call asking for a DIFFERENT cap on
    /// the same key keeps the original — asserted by a debug assertion rather
    /// than silently honoured, since it would mean two call sites disagree about
    /// one budget.
    gate: Arc<Semaphore>,
    /// The cap [`CommandState::gate`] was built with, kept only so a second call
    /// site asking for a different cap on the same key trips a debug assertion
    /// instead of silently getting the first one.
    max_concurrent: usize,
    /// Callers currently parked in [`Limiter::acquire_queued`] waiting for a
    /// permit. Bounded by that method's `max_queued`, and reported back so the
    /// UI can say "queued — N ahead" instead of looking hung.
    queued: usize,
}

/// Process-local anti-abuse limiter. Managed in Tauri state as `Arc<Limiter>`.
///
/// Cheap: a couple of small `HashMap`s touched only at command entry/exit.
#[derive(Default)]
pub struct Limiter {
    per_command: Mutex<HashMap<&'static str, CommandState>>,
    /// `(utc_day, provider) → accepted request count`. The day key lets a single
    /// map self-evict: a new day's first request sees a stale day and resets.
    per_provider_day: Mutex<HashMap<(u64, String), u32>>,
}

/// RAII concurrency slot: it OWNS the semaphore permit, so the slot is released
/// on drop whether the handler returns, `?`s out, or panics. There is no manual
/// release path to forget — that is the whole point of holding the permit here
/// rather than decrementing a counter.
pub struct ConcurrencyGuard {
    _permit: OwnedSemaphorePermit,
}

/// RAII membership in a command's wait queue, held only while parked in
/// [`Limiter::acquire_queued`].
///
/// The decrement lives in `Drop`, not in the statement after the `.await`,
/// because that await is a cancellation point: a caller whose future is dropped
/// while parked (user cancels, window closes, task aborted) would otherwise
/// leave the counter permanently high and eventually wedge the queue closed.
struct QueueSlot {
    limiter: Arc<Limiter>,
    command: &'static str,
    /// Callers already parked when this one arrived — reported to the UI.
    ahead: usize,
}

impl QueueSlot {
    /// Join `command`'s wait queue, or reject if it is already `max_queued` deep.
    fn enter(
        limiter: &Arc<Limiter>,
        command: &'static str,
        max_queued: usize,
    ) -> Result<Self, AppError> {
        let mut map = limiter.per_command.lock();
        let Some(state) = map.get_mut(command) else {
            // `check_rate` always inserts the entry first, so this is unreachable
            // in practice; treat it as a full queue rather than panicking.
            return Err(AppError::RateLimited(format!("{command} queue unavailable")));
        };
        if state.queued >= max_queued {
            return Err(AppError::RateLimited(format!(
                "Too many {command} requests waiting (max {max_queued}). Try again shortly."
            )));
        }
        let ahead = state.queued;
        state.queued += 1;
        Ok(Self {
            limiter: Arc::clone(limiter),
            command,
            ahead,
        })
    }
}

impl Drop for QueueSlot {
    fn drop(&mut self) {
        let mut map = self.limiter.per_command.lock();
        if let Some(state) = map.get_mut(self.command) {
            state.queued = state.queued.saturating_sub(1);
        }
    }
}

impl Limiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Admit one call of `command`, enforcing BOTH the sliding-window rate cap and
    /// the concurrency cap atomically. On success returns a [`ConcurrencyGuard`]
    /// the caller must hold for the duration of the work; dropping it frees the
    /// slot. On exceed returns a retriable [`AppError::RateLimited`] and reserves
    /// nothing.
    ///
    /// The rate window is admission-counted: a *rejected* call does NOT consume a
    /// window slot, so a hammering loop can't push the recovery time out forever.
    pub fn acquire(
        self: &Arc<Self>,
        command: &'static str,
        max_requests: usize,
        max_concurrent: usize,
    ) -> Result<ConcurrencyGuard, AppError> {
        self.acquire_at(command, max_requests, max_concurrent, Instant::now())
    }

    /// [`Self::acquire`] with an injectable `now`, so the window-rollover test can
    /// advance time without a real-clock wait. Production always passes
    /// `Instant::now()`.
    fn acquire_at(
        self: &Arc<Self>,
        command: &'static str,
        max_requests: usize,
        max_concurrent: usize,
        now: Instant,
    ) -> Result<ConcurrencyGuard, AppError> {
        let gate = self.check_rate(command, max_requests, max_concurrent, now)?;

        // Non-blocking: this admission style REJECTS rather than parks. The
        // window slot is recorded only after the permit is actually taken, so a
        // rejected call costs nothing.
        let permit = gate.try_acquire_owned().map_err(|_| {
            AppError::RateLimited(format!(
                "Too many concurrent {command} requests (max {max_concurrent}). Try again shortly."
            ))
        })?;
        self.record_start(command, now);
        Ok(ConcurrencyGuard { _permit: permit })
    }

    /// [`Self::acquire`], but on a full concurrency cap the caller **waits** for a
    /// slot instead of being rejected.
    ///
    /// For work a human deliberately started and expects to complete — the
    /// tailoring pipeline, where a user working through imported applications
    /// clicks Generate several times in a row. Rejecting the 4th click throws
    /// away an intentional action; parking it does not. Ordering is the
    /// semaphore's (FIFO), so nobody starves.
    ///
    /// The queue is bounded by `max_queued`: past that depth callers are rejected
    /// outright, so a looping renderer cannot turn "wait" into unbounded parked
    /// work. The rate window still applies at admission and still fails fast.
    ///
    /// `on_park` fires **before** the wait begins, exactly once, and only when
    /// the call actually has to wait — with how many callers are ahead of it. It
    /// has to run before the `.await`, not after, or the "queued" signal would
    /// arrive at the same moment the wait ended and tell the user nothing.
    ///
    /// Returns the guard plus whether it parked, so the caller can pair its
    /// "queued" signal with a matching "started" one.
    ///
    /// Cancel-safety: if the returned future is dropped while parked, the
    /// `queued` counter is still decremented — the decrement lives in a
    /// [`QueueSlot`] guard, not in the code path after the `.await`.
    pub async fn acquire_queued(
        self: &Arc<Self>,
        command: &'static str,
        max_requests: usize,
        max_concurrent: usize,
        max_queued: usize,
        on_park: impl FnOnce(usize),
    ) -> Result<(ConcurrencyGuard, bool), AppError> {
        let now = Instant::now();
        let gate = self.check_rate(command, max_requests, max_concurrent, now)?;

        // Fast path: a slot is free right now, so never touch the queue counter
        // and never emit a park signal.
        if let Ok(permit) = Arc::clone(&gate).try_acquire_owned() {
            self.record_start(command, now);
            return Ok((ConcurrencyGuard { _permit: permit }, false));
        }

        // Full: park, unless the queue itself is at its ceiling (rejected here
        // costs no window slot, same as any other rejection).
        let slot = QueueSlot::enter(self, command, max_queued)?;
        self.record_start(command, now);
        on_park(slot.ahead);

        let permit = gate
            .acquire_owned()
            .await
            .map_err(|_| AppError::RateLimited(format!("{command} is shutting down")))?;
        drop(slot);

        Ok((ConcurrencyGuard { _permit: permit }, true))
    }

    /// The shared first half of both admission styles: age the window and
    /// enforce the rate cap, then hand back the command's concurrency gate.
    ///
    /// Records **nothing** — the window slot is written by [`Self::record_start`]
    /// only once the call is genuinely admitted, so a *rejected* call never
    /// consumes a window slot and a hammering loop can't push its own recovery
    /// time out forever.
    fn check_rate(
        &self,
        command: &'static str,
        max_requests: usize,
        max_concurrent: usize,
        now: Instant,
    ) -> Result<Arc<Semaphore>, AppError> {
        let mut map = self.per_command.lock();
        let state = map.entry(command).or_insert_with(|| CommandState {
            recent: VecDeque::new(),
            gate: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
            queued: 0,
        });
        debug_assert_eq!(
            state.max_concurrent, max_concurrent,
            "two call sites disagree about {command}'s concurrency cap"
        );

        // Age out timestamps older than the window (front = oldest).
        let cutoff = now.checked_sub(RATE_WINDOW);
        while let Some(&front) = state.recent.front() {
            match cutoff {
                Some(c) if front <= c => {
                    state.recent.pop_front();
                }
                // `now < RATE_WINDOW` since boot: nothing is old enough to evict.
                _ => break,
            }
        }

        if state.recent.len() >= max_requests {
            return Err(AppError::RateLimited(format!(
                "Rate limit reached for {command}: max {max_requests} requests per {}s. Try again shortly.",
                RATE_WINDOW.as_secs()
            )));
        }

        Ok(Arc::clone(&state.gate))
    }

    /// Record an admitted start against the sliding window. Called only after the
    /// call is committed to running (permit held, or parked with a permit
    /// guaranteed to arrive).
    fn record_start(&self, command: &'static str, now: Instant) {
        if let Some(state) = self.per_command.lock().get_mut(command) {
            state.recent.push_back(now);
        }
    }

    /// How many callers are currently parked waiting for a `command` slot.
    /// Observability only — the queue ceiling is enforced inside
    /// [`QueueSlot::enter`] under the same lock, never by reading this first.
    pub fn queue_depth(&self, command: &'static str) -> usize {
        self.per_command
            .lock()
            .get(command)
            .map_or(0, |state| state.queued)
    }

    /// Charge one accepted request against `provider`'s daily ceiling. Call this
    /// only AFTER [`Self::acquire`] succeeds, so a rejected call costs no budget.
    /// On exceed returns a retriable [`AppError::RateLimited`].
    pub fn charge_provider_daily(&self, provider: &str, max_per_day: u32) -> Result<(), AppError> {
        let day = utc_day();
        let mut map = self.per_provider_day.lock();
        // Self-evict prior days so the map stays tiny (one entry per provider/day).
        map.retain(|(d, _), _| *d == day);
        let count = map.entry((day, provider.to_string())).or_insert(0);
        if *count >= max_per_day {
            return Err(AppError::RateLimited(format!(
                "Daily request limit reached for provider '{provider}' (max {max_per_day}/day). Resets at UTC midnight."
            )));
        }
        *count += 1;
        Ok(())
    }
}

/// Whole days since the Unix epoch in UTC. Used only as a coarse day bucket for
/// the per-provider daily counter; an exact calendar boundary is unnecessary.
fn utc_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400
}

#[cfg(test)]
mod test;
