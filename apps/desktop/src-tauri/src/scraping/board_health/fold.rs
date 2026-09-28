//! Pure derivation for the per-board reliability history: fold one run
//! summary into the stored counters, and the thresholds that turn those
//! counters into a [`super::BoardHealthStatus`] verdict.
//!
//! Split out of `board_health` (issue #1280); no I/O — see [`super::store`]
//! for persistence.

use super::types::{BoardHealth, BoardHealthStatus};
use crate::scraping::engine::BoardScrapeSummary;

/// A board whose last verified run succeeded, but whose last success is older
/// than this, is reported [`BoardHealthStatus::Stale`] — we have not actually
/// confirmed it works in a fortnight (it has only been skipped since). Two weeks
/// is comfortably longer than any built-in autopilot cadence, so a board that is
/// genuinely being exercised never trips it.
pub(super) const STALE_AFTER_MS: u64 = 14 * 24 * 60 * 60 * 1000;

/// Minimum verified runs before a failure RATE means anything — below this a
/// single unlucky run would brand a board unreliable.
const FLAKY_MIN_RUNS: u32 = 8;

/// Share of verified runs that must have failed for a currently-working board to
/// report [`BoardHealthStatus::Flaky`].
const FLAKY_FAIL_PERCENT: u32 = 25;

/// Cap on `verified_runs` before [`decay_tallies`] halves both tallies.
///
/// Without a cap, `verified_runs`/`failed_runs` are LIFETIME counts, which
/// gives [`is_flaky`] unbounded inertia in both directions: a board that had a
/// 10-run outage and has since worked flawlessly stays badged `unreliable` for
/// dozens more clean runs (the old failures are diluted, not forgotten), while
/// a board with a long clean history that starts genuinely flapping needs just
/// as many runs before the rate crosses the threshold — precisely the
/// alternating-failure pattern [`BoardHealthStatus::Flaky`] exists to catch,
/// which `consecutive_failures` structurally cannot see.
///
/// Twice [`FLAKY_MIN_RUNS`] so a decay can never drop `verified_runs` below the
/// minimum sample [`is_flaky`] itself requires.
///
/// This is a window of RUNS, not of time, and the two only coincide at a steady
/// cadence. On the daily autopilot it spans roughly one to two weeks, which
/// lines up with [`STALE_AFTER_MS`]; for someone scraping manually several
/// times an afternoon the same 16 runs can span hours, so the verdict forgets
/// this morning's trouble by evening. Deliberate — the counter has no clock and
/// a run is the only evidence a board ever produces — but do not read
/// "recent reliability" here as a calendar claim.
pub(super) const FLAKY_WINDOW_CAP: u32 = FLAKY_MIN_RUNS * 2;

/// Max stored length of the remembered failure reason.
///
/// The reason is REDACTED (`observability::sanitize_reason`) before it is stored,
/// not merely capped. "The autopilot store already persists the raw string" does
/// not license storing it here: that sink holds one autopilot's latest run and is
/// overwritten every run, whereas this row covers every scrape including manual
/// ones and is retained until the failure streak clears — indefinitely for a
/// permanently-broken board, and deliberately preserved across skips. That is new
/// retention and new scope, so the redaction is enforced here rather than left as
/// an invariant every current and future board has to remember.
pub(super) const MAX_ERROR_LEN: usize = 200;

/// The three mutually-exclusive things a run can say about a board.
///
/// Derived from a [`BoardScrapeSummary`] by [`outcome_of`]; existing as a type
/// (rather than two booleans) is what makes "a skip is not a failure"
/// unrepresentable-otherwise rather than a convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunOutcome {
    /// The board answered. A partial (`truncated`) harvest counts: the board was
    /// reachable and returned rows, which is what "does this source work?" asks.
    Ok,
    /// The board was contacted and failed.
    Error,
    /// The board was never contacted (needs-login / needs-company / needs-keys).
    Skipped,
}

/// Classify one run's summary. `error` wins over `skipped` — the engine never
/// sets both, but a persisted/tampered record could, and an error is the more
/// alarming reading.
fn outcome_of(summary: &BoardScrapeSummary) -> RunOutcome {
    if summary.error.is_some() {
        RunOutcome::Error
    } else if summary.skipped.is_some() {
        RunOutcome::Skipped
    } else {
        RunOutcome::Ok
    }
}

/// Fold one run's summary into a board's stored health. **Pure** — `now` is
/// injected, so the whole derivation is testable without a clock or a DB.
///
/// * `Ok`    → clears the streak, advances `last_success_at` + `last_verified_at`.
/// * `Error` → extends the streak (opening `failing_since` on the 0→1 edge so the
///   "since" is the FIRST failure, not the latest), advances `last_verified_at`.
/// * `Skipped` → advances nothing but the correlation id: the board was not
///   contacted, so it neither succeeded nor failed.
pub fn fold(
    prev: Option<BoardHealth>,
    summary: &BoardScrapeSummary,
    run_id: &str,
    now: u64,
) -> BoardHealth {
    let mut next = prev.unwrap_or_else(BoardHealth::empty);
    match outcome_of(summary) {
        RunOutcome::Ok => {
            next.consecutive_failures = 0;
            next.failing_since = None;
            next.last_error = None;
            next.last_success_at = Some(now);
            next.last_verified_at = Some(now);
            next.verified_runs = next.verified_runs.saturating_add(1);
            (next.verified_runs, next.failed_runs) =
                decay_tallies(next.verified_runs, next.failed_runs);
            next.last_run_id = Some(run_id.to_string());
        }
        RunOutcome::Error => {
            // Saturating so a pathological run count can never wrap the streak
            // back to "healthy".
            next.consecutive_failures = next.consecutive_failures.saturating_add(1);
            // Only the 0→1 edge opens the window; a continuing streak keeps its
            // original start so the UI can say "failing since <first failure>".
            next.failing_since.get_or_insert(now);
            next.last_error = summary.error.as_deref().map(clean_error);
            next.last_verified_at = Some(now);
            next.verified_runs = next.verified_runs.saturating_add(1);
            next.failed_runs = next.failed_runs.saturating_add(1);
            (next.verified_runs, next.failed_runs) =
                decay_tallies(next.verified_runs, next.failed_runs);
            next.last_run_id = Some(run_id.to_string());
        }
        // A skip advances NOTHING — not even `last_run_id`. That field names the
        // run which produced this state, and a skipped board was never contacted
        // by this run, so stamping it would attribute a Tuesday outage to a
        // Thursday run containing no fetch of that board at all.
        RunOutcome::Skipped => {}
    }
    next.status = derive_status(&next, now);
    next
}

/// Redact a reason (`observability::sanitize_reason`, the same scrubbing the
/// autopilot step log uses) and cap it to [`MAX_ERROR_LEN`] **characters** — not
/// bytes, since slicing a byte range would panic mid-codepoint on a non-ASCII
/// message. `sanitize_reason` already caps at its own ceiling; the cap is
/// re-applied here so this store's bound holds regardless of that constant.
fn clean_error(raw: &str) -> String {
    let redacted = crate::observability::sanitize_reason(raw);
    if redacted.chars().count() <= MAX_ERROR_LEN {
        return redacted;
    }
    let mut out: String = redacted.chars().take(MAX_ERROR_LEN).collect();
    out.push('…');
    out
}

/// Status from the folded counters. Kept separate from [`fold`] so it can also
/// re-derive a row read back from disk (whose `Stale` verdict depends on *now*,
/// not on when the row was written).
pub(super) fn derive_status(h: &BoardHealth, now: u64) -> BoardHealthStatus {
    if h.consecutive_failures > 0 {
        return BoardHealthStatus::Failing;
    }
    let Some(_verified) = h.last_verified_at else {
        // Only ever skipped — nothing has been confirmed either way.
        return BoardHealthStatus::Unknown;
    };
    match h.last_success_at {
        // Verified, no failure streak, but the confirmation has aged out: only
        // skips since. `saturating_sub` so a clock that moved backwards reads as
        // "recent", never as a giant staleness.
        Some(at) if now.saturating_sub(at) > STALE_AFTER_MS => BoardHealthStatus::Stale,
        // Working right now, but it has been failing a meaningful SHARE of the
        // runs that reached it — the state a streak counter cannot see.
        Some(_) if is_flaky(h) => BoardHealthStatus::Flaky,
        Some(_) => BoardHealthStatus::Healthy,
        // Verified but never successful with an empty streak is unreachable via
        // `fold`; a hand-edited row could still produce it. Treat "tried, never
        // worked" as stale rather than claiming health.
        None => BoardHealthStatus::Stale,
    }
}

/// Whether the windowed failure RATE (see [`decay_tallies`]) crosses the
/// flapping threshold. Integer math (no float rounding), and a hard minimum
/// sample so one bad run out of two never brands a board.
pub(super) fn is_flaky(h: &BoardHealth) -> bool {
    h.verified_runs >= FLAKY_MIN_RUNS
        && h.failed_runs.saturating_mul(100) >= h.verified_runs.saturating_mul(FLAKY_FAIL_PERCENT)
}

/// Bound `verified`/`failed` to a rolling window of at most [`FLAKY_WINDOW_CAP`]
/// runs, called from [`fold`] every time a run actually contacts the board.
/// `while` (not `if`) so a row written before this fix — whose `verified_runs`
/// may already be arbitrarily large — collapses into the window in ONE fold
/// call instead of trickling down one halving per run.
///
/// `failed` is rescaled BY THE SAME RATIO the halving applies to `verified`
/// (`failed * new_verified / verified`), not halved independently
/// (`failed / 2`). Independent halving rounds `failed` down LESS than
/// `verified` whenever `verified` is odd, which can *inflate* the ratio and
/// manufacture a Flaky verdict out of nothing but rounding — measured:
/// `verified=17, failed=4` (23.5%, correctly not flaky) independently halves to
/// `8, 2` (25.0%, now flaky) on a run that was a plain success. Proportional
/// scaling can only round the ratio DOWN (floor), so a decay step alone can
/// never flip a board from not-flaky to flaky.
/// The rescale is done in `u64`. In `u32`, `failed * next_verified` overflows
/// once the product passes ~4.29e9 and `saturating_mul` clamps it BEFORE the
/// divide, which destroys the very proportionality this function exists to
/// preserve: `(200_000, 150_000)` — a 75% failure rate — decayed to `(12, 2)`,
/// i.e. 17%, reading not-flaky. Unreachable through [`fold`], which caps
/// `verified` at [`FLAKY_WINDOW_CAP`] after every call, but reachable from a
/// hand-edited row — the same threat model this module already defends against
/// for a negative tally, so it is defended here too rather than left to the
/// next caller to rediscover.
pub(super) fn decay_tallies(mut verified: u32, mut failed: u32) -> (u32, u32) {
    while verified > FLAKY_WINDOW_CAP {
        let next_verified = verified / 2;
        // `failed <= verified` is an invariant of `fold`, and floor-scaling
        // preserves it, so the result is always <= next_verified and the
        // narrowing cast cannot truncate.
        failed = (u64::from(failed) * u64::from(next_verified) / u64::from(verified)) as u32;
        verified = next_verified;
    }
    (verified, failed)
}
