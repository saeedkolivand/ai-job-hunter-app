//! Tests for the grace-window snapshot: `accepted`/`remember`/`grace_window_key`/`refresh_from_read` (`proof/grace_window.rs`).

use super::super::super::super::agent_cli::policy::ProofSource;
use super::super::*;
use serde_json::json;

// ── accepted_at / remember_at — the grace window (issue #1162) ──────────

/// The ordinary, no-drift path never even looks at the snapshot map: an exact match on the
/// FRESH `current` value succeeds with nothing remembered for `key` at all.
#[test]
fn accepted_matches_the_fresh_current_value_with_no_snapshot_recorded() {
    assert!(accepted_at(
        Some("grace_cmd_fresh"),
        "4200",
        "4200",
        std::time::Instant::now()
    )
    .is_ok());
}

/// A value that matches neither the current value nor anything ever remembered for this
/// key is the ORDINARY mismatch — never `Expired` (there is nothing to have expired).
#[test]
fn accepted_refuses_a_value_matching_nothing_as_an_ordinary_mismatch() {
    let outcome = accepted_at(
        Some("grace_cmd_never_remembered"),
        "4200",
        "9999",
        std::time::Instant::now(),
    );
    assert!(matches!(outcome, Err(SnapshotOutcome::Mismatch)));
}

/// The headline #1162 case: the value disclosed at `confirmation_required` time (4200) is
/// snapshotted, the CURRENT value has since moved (background AI spend bumped it to 4300),
/// and the caller presents the OLDER value back within the grace window — must be accepted.
#[test]
fn accepted_accepts_a_remembered_snapshot_still_inside_the_ttl_even_though_current_moved() {
    let t0 = std::time::Instant::now();
    remember_at("grace_cmd_within_ttl", "4200".to_string(), t0);
    let outcome = accepted_at(
        Some("grace_cmd_within_ttl"),
        "4300", // the CURRENT value moved since disclosure
        "4200", // the caller presents the value it actually read
        t0 + std::time::Duration::from_secs(30),
    );
    assert!(
        outcome.is_ok(),
        "a snapshot still inside the TTL must be accepted even though the live value moved"
    );
}

/// The same remembered value, presented AFTER the TTL has closed, must refuse — distinctly,
/// as `Expired` rather than the generic `Mismatch`, so the caller learns to re-read rather
/// than assume it simply guessed wrong.
#[test]
fn accepted_refuses_as_expired_once_the_snapshots_ttl_has_closed() {
    let t0 = std::time::Instant::now();
    remember_at("grace_cmd_expired", "4200".to_string(), t0);
    let outcome = accepted_at(
        Some("grace_cmd_expired"),
        "4300",
        "4200",
        t0 + PROOF_SNAPSHOT_TTL + std::time::Duration::from_secs(1),
    );
    assert!(matches!(outcome, Err(SnapshotOutcome::Expired)));
}

/// A value that is simply WRONG — never the current value, never anything remembered for
/// this key — must still refuse as the generic `Mismatch`, snapshot or no snapshot. A
/// grace window must never turn into "any old guess eventually works".
#[test]
fn accepted_still_refuses_a_wrong_value_as_a_mismatch_even_with_a_snapshot_recorded() {
    let t0 = std::time::Instant::now();
    remember_at("grace_cmd_wrong_guess", "4200".to_string(), t0);
    let outcome = accepted_at(
        Some("grace_cmd_wrong_guess"),
        "4300",
        "totally-invented-guess",
        t0 + std::time::Duration::from_secs(5),
    );
    assert!(matches!(outcome, Err(SnapshotOutcome::Mismatch)));
}

/// A snapshot recorded for a DIFFERENT key must never satisfy this one's ceremony — the map
/// is keyed precisely so one row's disclosed value can't authorise another's.
#[test]
fn accepted_never_lets_a_snapshot_from_a_different_key_satisfy_this_one() {
    let t0 = std::time::Instant::now();
    remember_at("grace_cmd_other_command", "4200".to_string(), t0);
    let outcome = accepted_at(
        Some("grace_cmd_this_command"),
        "4300",
        "4200",
        t0 + std::time::Duration::from_secs(5),
    );
    assert!(matches!(outcome, Err(SnapshotOutcome::Mismatch)));
}

/// AC-1/SEC-1 CRITICAL: a `key: None` row must refuse the instant the exact match on
/// `current` fails, never even looking at the snapshot map — a value disclosed for a
/// completely different target (simulated here by a real snapshot under another key) must
/// never authorise it. This is the exact cross-target bypass the finding described.
#[test]
fn accepted_refuses_immediately_when_the_row_has_no_grace_window_even_if_a_snapshot_exists() {
    let t0 = std::time::Instant::now();
    // A real snapshot exists (e.g. `ai_spend_summary`'s own), recorded moments ago.
    remember_at(
        "grace_cmd_unrelated_target",
        "some-proof-value".to_string(),
        t0,
    );
    let outcome = accepted_at(
        None,
        "fresh-value-for-this-target",
        "some-proof-value", // matches the OTHER key's snapshot, not this row's current value
        t0 + std::time::Duration::from_secs(1),
    );
    assert!(
        matches!(outcome, Err(SnapshotOutcome::Mismatch)),
        "a row with no grace window must never accept a value disclosed for a different target"
    );
}

/// SEC-2 HIGH: a snapshot is single-use. The first presentation inside the TTL is accepted
/// (and consumes it); a second presentation of the exact same value must refuse.
#[test]
fn accepted_consumes_the_snapshot_so_a_second_presentation_of_the_same_value_refuses() {
    let t0 = std::time::Instant::now();
    remember_at("grace_cmd_single_use", "4200".to_string(), t0);
    let first = accepted_at(
        Some("grace_cmd_single_use"),
        "4300",
        "4200",
        t0 + std::time::Duration::from_secs(5),
    );
    assert!(
        first.is_ok(),
        "the first presentation inside the TTL must be accepted"
    );
    let second = accepted_at(
        Some("grace_cmd_single_use"),
        "4300",
        "4200",
        t0 + std::time::Duration::from_secs(6),
    );
    assert!(
        matches!(second, Err(SnapshotOutcome::Mismatch)),
        "a consumed snapshot must never authorise a second dispatch"
    );
}

// ── grace_window_key — which rows get a grace window at all (AC-1/SEC-1) ────────────────

#[test]
fn grace_window_key_is_eligible_only_for_the_ai_spend_summary_read_command() {
    let eligible = ProofSource::Scalar {
        read_command: "ai_spend_summary",
        path: &["today", "inputTokens"],
    };
    assert_eq!(grace_window_key(eligible), Some("ai_spend_summary"));
}

/// Every OTHER `Scalar` row (no background-drift problem to solve) must stay ineligible.
#[test]
fn grace_window_key_is_ineligible_for_a_different_scalar_read_command() {
    let ineligible = ProofSource::Scalar {
        read_command: "ai_active_config",
        path: &["activeProvider"],
    };
    assert_eq!(grace_window_key(ineligible), None);
}

/// A per-target row (`ListMatch`) must never be eligible — the finding's exact exploit shape.
#[test]
fn grace_window_key_is_ineligible_for_a_list_match_source() {
    let ineligible = ProofSource::ListMatch {
        read_command: "documents_list",
        id_field: &["id"],
        match_field: "_id",
        value_field: "name",
    };
    assert_eq!(grace_window_key(ineligible), None);
}

// ── refresh_from_read — closes the double-drift gap (AC-7) ─────────────────────────────

/// A direct read of `ai_spend_summary` must refresh the snapshot to whatever value the caller
/// just saw — the double-drift case a single t0-only snapshot cannot cover.
#[test]
fn refresh_from_read_updates_the_snapshot_from_a_direct_ai_spend_summary_read() {
    // A3-r2-AC-4: see `GRACE_WINDOW_KEY_TEST_LOCK`'s own doc.
    let _guard = GRACE_WINDOW_KEY_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let response = json!({ "today": { "inputTokens": 4321 } });
    refresh_from_read("ai_spend_summary", &response);
    let outcome = accepted_at(
        Some("ai_spend_summary"),
        "9999", // some later, further-moved current value
        "4321",
        std::time::Instant::now(),
    );
    assert!(
        outcome.is_ok(),
        "a value the caller just read directly must be accepted as a fresh snapshot"
    );
}

/// A direct read of any OTHER command must never touch the grace-window snapshot.
#[test]
fn refresh_from_read_is_a_noop_for_every_other_command() {
    const KEY: &str = "grace_cmd_refresh_noop_target";
    remember_at(
        KEY,
        "should-not-move".to_string(),
        std::time::Instant::now(),
    );
    refresh_from_read("documents_list", &json!([{ "id": "doc-1" }]));
    let outcome = accepted_at(
        Some(KEY),
        "fresh",
        "should-not-move",
        std::time::Instant::now(),
    );
    assert!(
        outcome.is_ok(),
        "the snapshot recorded directly via remember_at must be untouched by an unrelated \
         refresh_from_read call"
    );
}
