//! Tests for the confirm-and-run ceremony and its grace-window interaction with `proof` (`dispatch.rs`).

use super::super::super::agent_cli::policy::{Effect, ProofSource, POLICY};
use super::super::dispatch::confirm_and_run;
use super::super::*;
use super::support::{CMD_SOURCE, PROOF_VALUE, WRONG_GUESS};

#[test]
fn confirm_and_run_refuses_proof_unavailable_without_running_the_command() {
    let mut ran = false;
    let outcome = confirm_and_run(CMD_SOURCE, None, PROOF_VALUE, || ran = true);
    assert!(
        matches!(outcome, Err(Refusal::ProofUnavailable)),
        "an unresolvable proof must refuse, distinctly from a wrong value"
    );
    assert!(
        !ran,
        "an irreversible command must never run when the proof could not be resolved at all"
    );
}

#[test]
fn confirm_and_run_refuses_a_mismatch_without_running_the_command_and_leaks_neither_value() {
    let mut ran = false;
    let outcome = confirm_and_run(
        CMD_SOURCE,
        Some(PROOF_VALUE.to_string()),
        WRONG_GUESS,
        || ran = true,
    );
    let Err(refusal) = outcome else {
        panic!("a wrong confirm must refuse");
    };
    assert!(matches!(
        refusal,
        Refusal::ConfirmationMismatch { moved: false }
    ));
    assert!(
        !ran,
        "MUTATION GUARD: running before the comparison would dispatch an irreversible \
         command on a wrong confirm — this assertion is the one that fails if the core is \
         reordered to call `run` first"
    );
    let detail = refusal.detail();
    assert!(
        !detail.contains(PROOF_VALUE) && !detail.contains(WRONG_GUESS),
        "a mismatch must disclose neither the expected value nor the guess: {detail}"
    );
}

#[test]
fn confirm_and_run_runs_the_command_exactly_once_on_an_exact_match() {
    let mut runs = 0;
    let outcome = confirm_and_run(
        CMD_SOURCE,
        Some(PROOF_VALUE.to_string()),
        PROOF_VALUE,
        || {
            runs += 1;
            json!({ "dispatched": true })
        },
    );
    assert_eq!(
        outcome.ok(),
        Some(json!({ "dispatched": true })),
        "a matching confirm must return the run step's own reply, unchanged"
    );
    assert_eq!(runs, 1, "the command must run exactly once, never twice");
}

/// End-to-end through the PUBLIC entry point (not only `proof`'s own internal `_at` tests):
/// a `confirm` matching a snapshot `proof::remember` recorded for the real, grace-window-eligible
/// `ai_spend_summary` source is accepted by `confirm_and_run`, even though the freshly-`resolved`
/// value handed in has since moved (issue #1162's own background-drift case).
#[test]
fn confirm_and_run_accepts_a_remembered_snapshot_even_after_the_live_value_moved() {
    // A3-r2-AC-4: the literal `ai_spend_summary` key is the real, fixed grace-window key (not a
    // test-choosable literal), so this test shares a lock with `proof`'s own
    // `refresh_from_read_updates_the_snapshot_from_a_direct_ai_spend_summary_read` -- see
    // `proof::GRACE_WINDOW_KEY_TEST_LOCK`'s doc.
    let _guard = proof::GRACE_WINDOW_KEY_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    const GRACE_SOURCE: ProofSource = ProofSource::Scalar {
        read_command: "ai_spend_summary",
        path: &["today", "inputTokens"],
    };
    proof::remember("ai_spend_summary", PROOF_VALUE.to_string());
    let mut ran = false;
    // `resolved` stands in for the CURRENT value having moved since disclosure; `confirm` is
    // the value the caller actually read and is presenting back.
    let outcome = confirm_and_run(
        GRACE_SOURCE,
        Some(WRONG_GUESS.to_string()),
        PROOF_VALUE,
        || ran = true,
    );
    assert!(
        outcome.is_ok(),
        "a confirm matching a fresh-enough snapshot must be accepted despite the moved value"
    );
    assert!(ran, "the command must run once the snapshot is accepted");
}

/// A3-r1-AC-1/SEC-1 CRITICAL, through the PUBLIC entry point: a proof snapshot disclosed for one
/// per-target command (e.g. `documents_remove` targeting doc A) must never satisfy a DIFFERENT
/// command's ceremony, even though the old, command-name-only key made exactly this shape
/// possible for a spend-based proof. Uses a `ListMatch` source (a real per-target shape) rather
/// than a `Scalar` one — no grace window exists for it at all, so a snapshot recorded under
/// whatever key it might have used must never be consulted.
#[test]
fn confirm_and_run_never_lets_a_per_target_source_use_the_grace_window() {
    const PER_TARGET_SOURCE: ProofSource = ProofSource::ListMatch {
        read_command: "documents_list",
        id_field: &["id"],
        match_field: "_id",
        value_field: "name",
    };
    // A3-r2-AC-5 fix: seeded under `"documents_list"` -- the exact key a REGRESSED
    // `grace_window_key` (one keyed off `source.read_command()` directly, the pre-fix shape)
    // would consult for `PER_TARGET_SOURCE`. The probe literal this replaced was never looked
    // up by any implementation, fixed or regressed, so the test passed unconditionally; seeding
    // under the command's OWN read_command name is what makes it fail if the eligibility gate is
    // ever widened. No other test in this crate uses `"documents_list"` as a snapshot key, so
    // this still can't collide with a concurrently-running test.
    proof::remember("documents_list", PROOF_VALUE.to_string());
    let mut ran = false;
    let outcome = confirm_and_run(
        PER_TARGET_SOURCE,
        Some(WRONG_GUESS.to_string()),
        PROOF_VALUE,
        || ran = true,
    );
    assert!(
        matches!(outcome, Err(Refusal::ConfirmationMismatch { moved: false })),
        "a per-target source must never accept a value via any grace window"
    );
    assert!(
        !ran,
        "an irreversible command must never run on a value the ceremony refused"
    );
}

/// A3-r2-AC-3 HIGH, through the PUBLIC `proof::accepted` entry point (the internal `accepted_at`
/// test in `proof.rs` covers the pure core; this pins the same guarantee at the boundary
/// `confirm_and_run` actually calls). The exact-match fast path is ALSO single-use: a snapshot
/// recorded at V is still current when `--confirm V` arrives (exact match, accepted), the live
/// counter then moves to V', and the SAME `--confirm V` must not be accepted again off the
/// surviving snapshot.
#[test]
fn proof_accepted_consumes_the_snapshot_on_an_exact_match_too() {
    proof::remember("grace_cmd_exact_match_single_use", "4200".to_string());
    let first = proof::accepted(Some("grace_cmd_exact_match_single_use"), "4200", "4200");
    assert!(
        first.is_ok(),
        "an exact match against live must be accepted"
    );
    let second = proof::accepted(Some("grace_cmd_exact_match_single_use"), "4300", "4200");
    assert!(
        matches!(second, Err(proof::SnapshotOutcome::Mismatch)),
        "one disclosure must buy exactly one dispatch, even via the exact-match fast path"
    );
}

/// A3-r2-AC-6 MEDIUM: [`proof::GRACE_WINDOW_PATH`] used to rest on a hand-verified prose claim
/// nothing enforced -- an 11th `ai_spend_summary`-backed `Irreversible` row with a DIFFERENT path
/// would still be `grace_window_key`-eligible (keyed on `read_command` alone), so its ceremony
/// could be satisfied by a value read from a field it doesn't prove on. Scans every real
/// [`POLICY`] row instead of trusting the prose.
#[test]
fn every_ai_spend_summary_irreversible_row_proves_on_the_shared_grace_window_path() {
    let mut checked = 0;
    for entry in POLICY {
        let Effect::Irreversible(source) = entry.effect else {
            continue;
        };
        if source.read_command() != proof::GRACE_WINDOW_READ_COMMAND {
            continue;
        }
        // Issue #1183 O3: match on the WHOLE `ProofSource`, not only `Scalar` -- the prior
        // `let ... else { continue }` pattern skipped a non-`Scalar` row naming
        // `ai_spend_summary` (a `Lookup`/`ListMatch`/`Count`/`MatchCount`) silently, the same
        // shape this test's own doc says `GRACE_WINDOW_PATH` used to rest on unenforced prose
        // for. `panic!` on any other variant so a future non-`Scalar` grace-window-eligible row
        // is a loud failure here, not a quiet gap in this test's own coverage.
        let ProofSource::Scalar { path, .. } = source else {
            panic!(
                "{} names {} but is not a Scalar proof source ({source:?}) -- the shared \
                 grace-window snapshot only ever answers a Scalar shape",
                entry.path,
                proof::GRACE_WINDOW_READ_COMMAND
            );
        };
        assert_eq!(
            path,
            proof::GRACE_WINDOW_PATH,
            "{} names {} but proves on a path DIFFERENT from the shared grace-window snapshot \
             -- eligible, but answering for the wrong field",
            entry.path,
            proof::GRACE_WINDOW_READ_COMMAND
        );
        checked += 1;
    }
    // Hand-written literal, not derived from the loop -- catches a row silently REMOVED.
    // 10 -> 9 (issue #1169): `help_search`'s dense arm proved on `ai_spend_summary` via its own
    // `charge_provider_daily` read before that row moved `Irreversible` -> `NotExposed`, taking
    // its grace-window-eligible proof with it.
    assert_eq!(
        checked, 9,
        "expected 9 POLICY rows naming ai_spend_summary this way"
    );
}

// ── classify_response / invoke_error_detail (pure) ───────────────────────
// HIGH fix (security review round 2): `InvokeResponse::Err` used to be
// folded straight into `invoke_command`'s `Ok(Value)`, so a Tauri-level
// rejection (bad/missing args, an ACL denial, an unregistered command) OR a
// command's own typed `Err` reported `dispatched: true` for a call whose
// body never ran (or failed). These pin the pure split that fixes it —
// `classify_response` has no `AppHandle`, so it's directly testable, unlike
// `invoke_command` itself (this crate has no `tauri::test` mock-app harness).
