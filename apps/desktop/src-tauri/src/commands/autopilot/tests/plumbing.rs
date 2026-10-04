//! The command module's own plumbing: the pending-focus buffer, the concurrent-run guard and the
//! rule that every record mutation goes through `mutate_record`.

use parking_lot::Mutex;

use super::super::run::RunGuard;
use super::super::take_pending_focus;

#[test]
fn take_pending_focus_returns_buffered_id_then_clears() {
    let buf = crate::tray::PendingFocus(Mutex::new(Some("autopilot-123".to_string())));
    assert_eq!(take_pending_focus(&buf), Some("autopilot-123".to_string()));
    // Atomic take cleared the slot — a second pull (e.g. a later focus) is empty,
    // so a cold-start deep-link focus is delivered exactly once and can't re-fire.
    assert_eq!(take_pending_focus(&buf), None);
}

#[test]
fn take_pending_focus_returns_none_when_empty() {
    let buf = crate::tray::PendingFocus(Mutex::new(None));
    assert_eq!(take_pending_focus(&buf), None);
}

// ── concurrent-run guard (item 2) ──────────────────────────────────────
// Distinct ids per test isolate the process-global RUNS_IN_FLIGHT set from
// the parallel test runner, so no #[serial] is needed.

#[test]
fn run_guard_blocks_a_second_concurrent_acquire() {
    let id = "guard-test-concurrent";
    let first = RunGuard::try_acquire(id).expect("first acquire succeeds");
    assert!(
        RunGuard::try_acquire(id).is_none(),
        "a second acquire for the same in-flight id is blocked (no double-run)"
    );
    drop(first);
    assert!(
        RunGuard::try_acquire(id).is_some(),
        "after the first guard drops, the id can be acquired again"
    );
}

#[test]
fn run_guard_distinct_ids_do_not_block_each_other() {
    let _a = RunGuard::try_acquire("guard-test-a").expect("id a acquires");
    assert!(
        RunGuard::try_acquire("guard-test-b").is_some(),
        "different autopilot ids run concurrently — the guard is per-id"
    );
}

/// The two tests above pin what `drop_orphaned_resume_cache` DOES; this pins
/// that the commands still call it — the wiring, which is the half that shipped
/// broken (the UPDATE path landed without the cleanup the DELETE path had just
/// gained). Driving `autopilot_update`/`autopilot_remove` for real needs an
/// `AppHandle` and this crate has no `tauri::test` mock-app harness, so the
/// cheapest honest guard is a source pin (the `pipeline::json` +
/// `extension_bridge::answer_rewrite` precedent; `include_str!` also makes rustc
/// track the file, so this can never read a stale copy).
///
/// The invariant: every mutation of an autopilot RECORD goes through
/// `mutate_record`, which owns both halves. A new command that reaches the store
/// directly — the exact shape of the original defect — fails here.
///
/// Rustfmt assumption: the enclosing opener of a call is the nearest preceding
/// line at a strictly smaller indent. True for every form rustfmt emits here; a
/// hand-wrapped receiver chain would need this pin updated with it.
#[test]
fn every_record_mutation_goes_through_mutate_record() {
    const SRC: &str = concat!(
        include_str!("../mod.rs"),
        include_str!("../run.rs"),
        include_str!("../phases.rs"),
        include_str!("../keyword_rank.rs"),
    );

    // Assembled at runtime, never written out as one literal: an inline needle
    // would appear in THIS file, and this file is a `#[path]` module OF the one
    // being scanned — a future inlining would make the test satisfy its own scan.
    let writers: Vec<String> = ["update", "remove"]
        .iter()
        .map(|method| format!(".{}().{method}(", "lock"))
        .collect();
    let owner = format!("mutate_{}(", "record");
    let indent = |l: &str| l.len() - l.trim_start().len();
    let is_code = |l: &str| !l.trim().is_empty() && !l.trim_start().starts_with("//");

    let lines: Vec<&str> = SRC.lines().collect();
    let mut pinned = Vec::new();
    for (i, &line) in lines.iter().enumerate() {
        if !is_code(line) || !writers.iter().any(|w| line.contains(w.as_str())) {
            continue;
        }
        // The in-flight guard is a `HashSet` of run ids, not the record store.
        if line.contains("RUNS_IN_FLIGHT") {
            continue;
        }
        let opener = lines[..i]
            .iter()
            .rev()
            .copied()
            .filter(|l| is_code(l))
            .find(|l| indent(l) < indent(line))
            .unwrap_or_default();
        assert!(
            opener.contains(&owner),
            "src/commands/autopilot/{{mod,run,phases,keyword_rank}}.rs (concatenated):{}\n  {}\nmutates an autopilot record outside \
             `mutate_record`, whose enclosing block is instead:\n  {}\n\n\
             Every record mutation must run through `mutate_record`: the cache id is \
             sha256(resume_text), so a DELETE and a resume REPLACE orphan the identical \
             `autopilot-resume:<sha>` vector + `match_scores` rows. Route the new call \
             through it rather than remembering the cleanup at one more site.",
            i + 1,
            line.trim(),
            opener.trim()
        );
        pinned.push(line.trim());
    }
    assert_eq!(
        pinned.len(),
        2,
        "expected exactly the update + remove call sites to be pinned; got {pinned:?}. \
         A drop to 0 means the scan stopped matching the real writers (a renamed store \
         method, or a reformatted call) and is silently guarding nothing."
    );
}
