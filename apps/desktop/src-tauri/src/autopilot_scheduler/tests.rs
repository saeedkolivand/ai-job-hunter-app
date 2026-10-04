//! Tests for the autopilot scheduler, split by topic: [`occurrence`] (jitter and the clock-anchored
//! occurrence math) and [`retry`] (due-ness, the bounded Failed-run retry and interrupted-run
//! recovery). The fixtures both share live here.

use chrono::Offset;

use super::*;
use crate::autopilot::tests::support::autopilot_fixture;

mod occurrence;
mod retry;

fn ap(schedule: &str, status: AutopilotStatus, last_run_at: Option<u64>) -> Autopilot {
    Autopilot {
        status,
        schedule: schedule.into(),
        last_run_at,
        ..autopilot_fixture()
    }
}

/// A fixed *local* wall-clock instant on 2026-06-04 at `h:m:00`, built
/// timezone-portably.
///
/// We can't construct it via `Local.with_ymd_and_hms(...)` — on a CI runner
/// whose local tz makes that wall time non-existent or ambiguous (a DST
/// gap/overlap), `.single()` is `None` and `.unwrap()` panics. Instead we
/// derive the UTC offset for that calendar day from a stable epoch instant
/// and subtract it, so we land on a real instant whose *local* H:M is what
/// we asked for, on every runner regardless of timezone.
fn local_on_2026_06_04(h: u32, m: u32) -> DateTime<Local> {
    // Stable anchor: 2026-06-04 00:00:00 UTC, expressed in local time.
    // 1_780_531_200_000 ms = 2026-06-04T00:00:00Z.
    let local_midnight_utc = DateTime::from_timestamp_millis(1_780_531_200_000)
        .unwrap()
        .with_timezone(&Local);
    // Offset that Local applies on that day; subtracting it makes the
    // resulting instant read as `h:m` on the local wall clock.
    let offset_secs = local_midnight_utc.offset().fix().local_minus_utc() as i64;
    let target_utc_ms =
        1_780_531_200_000 + (h as i64 * 3_600 + m as i64 * 60 - offset_secs) * 1_000;
    DateTime::from_timestamp_millis(target_utc_ms)
        .unwrap()
        .with_timezone(&Local)
}

/// Fixed reference instant on 2026-06-04 at `h:m` local. All occurrence math
/// is deterministic against it (independent of the wall clock) and the
/// construction never panics on any runner's timezone.
fn now_at(h: u32, m: u32) -> DateTime<Local> {
    local_on_2026_06_04(h, m)
}

/// Same calendar instant one day earlier (2026-06-03) at `h:m` local —
/// used by the "yesterday's occurrence" assertions.
fn yesterday_at(h: u32, m: u32) -> DateTime<Local> {
    now_at(h, m) - chrono::Duration::days(1)
}

/// The existing occurrence assertions predate jitter and describe the pure
/// clock arithmetic, so they pass zero explicitly rather than inheriting a
/// default — the shift gets its own tests below.
const NO_JITTER: chrono::Duration = chrono::Duration::zero();
