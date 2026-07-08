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
//! 2. **Concurrency cap** — at most `max_concurrent` in-flight calls of a command.
//!    Acquired as an RAII [`ConcurrencyGuard`] that decrements the live count on
//!    drop, so a panicking / early-returning handler can never leak a slot.
//! 3. **Per-provider daily request ceiling** — a generous runaway-cost backstop on
//!    total accepted AI requests per provider per UTC day.
//!
//! Defaults are intentionally **generous** so normal interactive use never trips
//! them; they exist to stop pathological loops, not to throttle a human.
//!
//! The whole limiter lives in Tauri managed state as `Arc<Limiter>`; the RAII
//! guard holds its own `Arc<Limiter>` clone so the slot is released even if the
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

use crate::error::AppError;

// ── Tuning constants ────────────────────────────────────────────────────────
//
// GENEROUS by design — a human clicking "Generate" or "Scrape" repeatedly stays
// well under these; only a runaway loop trips them.

/// `ai_generate`: at most this many starts per [`RATE_WINDOW`].
pub const AI_GENERATE_RATE_MAX: usize = 20;
/// `ai_generate`: at most this many in-flight at once.
pub const AI_GENERATE_CONCURRENCY_MAX: usize = 3;

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

/// Sliding-window + concurrency state for a single command key.
#[derive(Default)]
struct CommandState {
    /// Accepted start instants within the current window (oldest at the front).
    recent: VecDeque<Instant>,
    /// Currently in-flight calls of this command.
    in_flight: usize,
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

/// RAII concurrency slot. Releasing (drop) decrements the live in-flight count
/// for its command, so an early `?`/panic in the handler can never leak a slot.
pub struct ConcurrencyGuard {
    command: &'static str,
    limiter: Arc<Limiter>,
}

impl Drop for ConcurrencyGuard {
    fn drop(&mut self) {
        let mut map = self.limiter.per_command.lock();
        if let Some(state) = map.get_mut(self.command) {
            state.in_flight = state.in_flight.saturating_sub(1);
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
        {
            let mut map = self.per_command.lock();
            let state = map.entry(command).or_default();

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
            if state.in_flight >= max_concurrent {
                return Err(AppError::RateLimited(format!(
                    "Too many concurrent {command} requests (max {max_concurrent}). Try again shortly."
                )));
            }

            // Admit: record the start and reserve the concurrency slot.
            state.recent.push_back(now);
            state.in_flight += 1;
        }

        Ok(ConcurrencyGuard {
            command,
            limiter: Arc::clone(self),
        })
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
