//! Grace window for a proof value disclosed via `confirmation_required` (issue #1162) — split out
//! of `proof.rs` under the same R8 LOC-cap reasoning as that file's own split from `agent_call.rs`.

use serde_json::Value;

use super::super::super::agent_cli::policy::ProofSource;
use super::{stringify, walk};

// ── Grace window for a proof value disclosed via `confirmation_required` (issue #1162) ──────
//
// `ai_spend_summary`'s `today.inputTokens` backs ten `Irreversible` rows and advances under
// ORDINARY background AI activity between the moment a caller reads it and the moment it presents
// that same value as `--confirm`. Fix: remember the value CURRENT at `confirmation_required` time
// and accept a presented value matching that snapshot for a short window after.
//
// Deliberately narrow (security review round A3-r1, AC-1/SEC-1 CRITICAL): the window applies ONLY
// to [`GRACE_WINDOW_READ_COMMAND`]. Every other row's proof is bound to a caller-chosen target (a
// document id, a run id…) and only an exact match on the FRESH value is ever accepted — no
// snapshot, no window, so a value disclosed for one target can never authorise a different one.
// `ai_spend_summary`'s `Scalar` proof is the one shape with NO per-target caller input at all
// (`build_input` always resolves it with `{}`), so there is no target to confuse in the first
// place. Widening this to every `Scalar` row (`ai_active_config`/`system_get_version`, neither of
// which has a background-drift problem to solve) would reopen that risk for no benefit.

/// The ONE read command whose `Irreversible` rows get a grace window. `Scalar`'s `path` is
/// identical on every real row naming this read command (verified in `policy.rs`), so one shared
/// snapshot is exactly as precise as one per irreversible command name, and lets
/// [`refresh_from_read`] update it from a single place regardless of which of the ten commands the
/// caller is about to confirm.
// `pub(super)` (A3-r2-AC-6) -- `agent_call::tests`' POLICY-scanning regression test names both.
pub(in crate::extension_bridge::agent_call) const GRACE_WINDOW_READ_COMMAND: &str =
    "ai_spend_summary";
pub(in crate::extension_bridge::agent_call) const GRACE_WINDOW_PATH: &[&str] =
    &["today", "inputTokens"];

/// How long a snapshot stays acceptable even after the CURRENT value has moved. ~120s: generous
/// enough for "read the proof, paste it back", short enough not to become a standing credential.
pub(in crate::extension_bridge::agent_call) const PROOF_SNAPSHOT_TTL: std::time::Duration =
    std::time::Duration::from_secs(120);

/// One remembered proof value. `at` is [`std::time::Instant`] — monotonic, immune to a clock
/// adjustment reviving an expired snapshot.
struct ProofSnapshot {
    value: String,
    at: std::time::Instant,
}

/// Keyed by [`GRACE_WINDOW_READ_COMMAND`] alone (the map holds at most one entry in practice).
static PROOF_SNAPSHOTS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<&'static str, ProofSnapshot>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Whether `source`'s `Irreversible` row gets a grace window at all — `Some(key)` when it does,
/// `None` for every other row, which [`accepted_at`] then never consults the snapshot map for.
pub(in crate::extension_bridge::agent_call) fn grace_window_key(
    source: ProofSource,
) -> Option<&'static str> {
    (source.read_command() == GRACE_WINDOW_READ_COMMAND).then_some(GRACE_WINDOW_READ_COMMAND)
}

/// Record `value` as the current grace-window snapshot — called right after a
/// `confirmation_required` refusal for a [`grace_window_key`]-eligible row, BEFORE the caller
/// could have read this value any other way. Split from [`remember_at`] so a test can drive the
/// pure core against a manufactured `now`.
pub(in crate::extension_bridge::agent_call) fn remember(key: &'static str, value: String) {
    remember_at(key, value, std::time::Instant::now());
}

pub(in crate::extension_bridge::agent_call) fn remember_at(
    key: &'static str,
    value: String,
    now: std::time::Instant,
) {
    let mut map = PROOF_SNAPSHOTS.lock().unwrap_or_else(|e| e.into_inner());
    map.insert(key, ProofSnapshot { value, at: now });
}

/// Refresh the grace-window snapshot from a response the caller just read DIRECTLY through
/// [`super::dispatch_direct`] (never one `resolve` fetched on the caller's own behalf). Closes the
/// double-drift gap security review round A3-r1's AC-7 flagged: the t0 snapshot and the value the
/// caller actually reads before retrying (t1) can differ if the counter moves in both intervals,
/// so this keeps the snapshot current with whichever value the caller most recently saw. A no-op
/// for every other command.
pub(in crate::extension_bridge::agent_call) fn refresh_from_read(command: &str, response: &Value) {
    if command != GRACE_WINDOW_READ_COMMAND {
        return;
    }
    if let Some(value) = walk(response, GRACE_WINDOW_PATH).and_then(stringify) {
        remember(GRACE_WINDOW_READ_COMMAND, value);
    }
}

/// Why a presented `--confirm` value was refused when it isn't the FRESH proof (issue #1162):
/// [`Mismatch`] is an ordinary wrong guess; [`Expired`] matches a real, remembered snapshot whose
/// window has closed, telling the caller to re-read rather than "you guessed wrong".
///
/// [`Mismatch`]: SnapshotOutcome::Mismatch
/// [`Expired`]: SnapshotOutcome::Expired
pub(in crate::extension_bridge::agent_call) enum SnapshotOutcome {
    Mismatch,
    Expired,
}

/// Whether `presented` is acceptable, given the FRESH `current` value and `key` —
/// [`grace_window_key`]'s verdict for the row being confirmed. An exact match on `current` always
/// succeeds first, so this still works for a `None`-key row too. Otherwise only a `Some` key may
/// match the snapshot [`remember`]/[`refresh_from_read`] last recorded for it within
/// [`PROOF_SNAPSHOT_TTL`] — `None` refuses immediately, never touching the map (AC-1/SEC-1
/// CRITICAL: no snapshot can ever authorise a different target's ceremony). A matching snapshot is
/// consumed on accept (SEC-2 HIGH): one disclosure buys exactly one dispatch. Never discloses
/// `current` or the snapshot value — only yes/no.
pub(in crate::extension_bridge::agent_call) fn accepted(
    key: Option<&'static str>,
    current: &str,
    presented: &str,
) -> Result<(), SnapshotOutcome> {
    accepted_at(key, current, presented, std::time::Instant::now())
}

pub(in crate::extension_bridge::agent_call) fn accepted_at(
    key: Option<&'static str>,
    current: &str,
    presented: &str,
    now: std::time::Instant,
) -> Result<(), SnapshotOutcome> {
    if presented == current {
        // A3-r2-AC-3 HIGH -- consume any snapshot for `key` here too, or it survives to
        // authorise a second dispatch once the live counter moves back onto the disclosed value.
        if let Some(key) = key {
            let mut map = PROOF_SNAPSHOTS.lock().unwrap_or_else(|e| e.into_inner());
            map.remove(key);
        }
        return Ok(());
    }
    let Some(key) = key else {
        return Err(SnapshotOutcome::Mismatch);
    };
    let mut map = PROOF_SNAPSHOTS.lock().unwrap_or_else(|e| e.into_inner());
    let outcome = match map.get(key) {
        Some(snap)
            if snap.value == presented
                && now.saturating_duration_since(snap.at) <= PROOF_SNAPSHOT_TTL =>
        {
            Ok(())
        }
        Some(snap) if snap.value == presented => Err(SnapshotOutcome::Expired),
        _ => Err(SnapshotOutcome::Mismatch),
    };
    if outcome.is_ok() {
        map.remove(key);
    }
    outcome
}

/// A3-r2-AC-4: serializes every test touching [`PROOF_SNAPSHOTS`] under the literal
/// [`GRACE_WINDOW_READ_COMMAND`] key -- the one key that isn't test-choosable, so two tests on
/// the real grace-window path race on the shared map without this lock.
#[cfg(test)]
pub(in crate::extension_bridge::agent_call) static GRACE_WINDOW_KEY_TEST_LOCK: std::sync::Mutex<()> =
    std::sync::Mutex::new(());
