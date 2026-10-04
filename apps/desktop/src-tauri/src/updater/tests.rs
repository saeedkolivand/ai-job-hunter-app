use super::*;

#[test]
fn test_updater_state_default() {
    let state = UpdaterState::default();
    assert!(state.pending_version.is_none());
    assert!(state.pending_update.is_none());
    assert!(state.downloaded_bytes.is_none());
    assert!(!state.checked);
}

#[test]
fn test_updater_state_with_version() {
    let state = UpdaterState {
        pending_version: Some("1.0.0".to_string()),
        downloaded_bytes: Some(vec![1, 2, 3]),
        ..UpdaterState::default()
    };
    assert_eq!(state.pending_version, Some("1.0.0".to_string()));
    assert_eq!(state.downloaded_bytes, Some(vec![1, 2, 3]));
}

#[test]
fn test_updater_state_empty_bytes() {
    let state = UpdaterState {
        downloaded_bytes: Some(vec![]),
        ..UpdaterState::default()
    };
    assert_eq!(state.downloaded_bytes, Some(vec![]));
}

#[test]
fn test_updater_state_version_only() {
    let state = UpdaterState {
        pending_version: Some("2.5.0".to_string()),
        ..UpdaterState::default()
    };
    assert_eq!(state.pending_version, Some("2.5.0".to_string()));
    assert!(state.downloaded_bytes.is_none());
}

#[test]
fn test_updater_state_large_bytes() {
    let large_bytes: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    let state = UpdaterState {
        downloaded_bytes: Some(large_bytes.clone()),
        ..UpdaterState::default()
    };
    assert_eq!(state.downloaded_bytes, Some(large_bytes));
}

#[test]
fn test_updater_state_version_with_special_chars() {
    let state = UpdaterState {
        pending_version: Some("v1.0.0-beta.1".to_string()),
        ..UpdaterState::default()
    };
    assert_eq!(state.pending_version, Some("v1.0.0-beta.1".to_string()));
}

#[test]
fn test_updater_state_bytes_take() {
    let mut state = UpdaterState {
        downloaded_bytes: Some(vec![1, 2, 3]),
        ..UpdaterState::default()
    };
    let taken = state.downloaded_bytes.take();
    assert_eq!(taken, Some(vec![1, 2, 3]));
    assert!(state.downloaded_bytes.is_none());
}

#[test]
fn test_updater_state_version_take() {
    let mut state = UpdaterState {
        pending_version: Some("1.0.0".to_string()),
        ..UpdaterState::default()
    };
    let taken = state.pending_version.take();
    assert_eq!(taken, Some("1.0.0".to_string()));
    assert!(state.pending_version.is_none());
}

#[test]
fn test_updater_state_both_take() {
    let mut state = UpdaterState {
        pending_version: Some("1.0.0".to_string()),
        downloaded_bytes: Some(vec![1, 2, 3]),
        ..UpdaterState::default()
    };
    let version = state.pending_version.take();
    let bytes = state.downloaded_bytes.take();
    assert_eq!(version, Some("1.0.0".to_string()));
    assert_eq!(bytes, Some(vec![1, 2, 3]));
    assert!(state.pending_version.is_none());
    assert!(state.downloaded_bytes.is_none());
}

#[test]
fn test_updater_state_multiple_sets() {
    let mut state = UpdaterState {
        pending_version: Some("1.0.0".to_string()),
        ..UpdaterState::default()
    };
    state.pending_version = Some("2.0.0".to_string());
    assert_eq!(state.pending_version, Some("2.0.0".to_string()));
}

#[test]
fn test_updater_state_bytes_replace() {
    let mut state = UpdaterState {
        downloaded_bytes: Some(vec![1, 2, 3]),
        ..UpdaterState::default()
    };
    state.downloaded_bytes = Some(vec![4, 5, 6]);
    assert_eq!(state.downloaded_bytes, Some(vec![4, 5, 6]));
}

// ── download_in_progress_or_done: the guard `updater_check` (don't discard)
// and `updater_download` (don't start a second transfer) both key on ────────

#[test]
fn test_download_in_progress_or_done_false_when_untouched() {
    assert!(!download_in_progress_or_done(&UpdaterState::default()));
}

#[test]
fn test_download_in_progress_or_done_true_while_downloading() {
    let state = UpdaterState {
        downloading: true,
        ..UpdaterState::default()
    };
    assert!(download_in_progress_or_done(&state));
}

#[test]
fn test_download_in_progress_or_done_true_once_downloaded() {
    let state = UpdaterState {
        downloaded_bytes: Some(vec![1, 2, 3]),
        ..UpdaterState::default()
    };
    assert!(download_in_progress_or_done(&state));
}

#[test]
fn test_download_in_progress_or_done_false_after_bytes_taken() {
    // Mirrors `updater_install`'s `downloaded_bytes.take()` on success — once
    // consumed, a fresh check/download must be allowed again.
    let mut state = UpdaterState {
        downloaded_bytes: Some(vec![1]),
        ..UpdaterState::default()
    };
    state.downloaded_bytes.take();
    assert!(!download_in_progress_or_done(&state));
}

// ── The `checked` producer side (`B1-r3-ACLI-R7-1`) ─────────────────────────

/// Every `status_reply` test (in `replies/tests.rs`) is pure — it reads `UpdaterState.checked`,
/// never sets it — so deleting all four production writes (`updater_check`'s
/// two `Ok(...)` arms, `silent_check`'s two mirrors) left the whole suite
/// green while `updater_status` would answer "never checked" forever on a
/// build that checks every 4 h. This crate has no `tauri::test` mock-app
/// harness (see the doc comments on [`download_in_progress_or_done`] and
/// [`store_managed`]), so the producer side is pinned at the SOURCE rather
/// than by driving the async commands: deleting any of the four writes fails
/// this test even though every `status_reply` test stays green.
#[test]
fn all_four_checked_true_writes_are_still_present() {
    const MOD_RS: &str = include_str!("mod.rs");
    let guard_writes = MOD_RS.matches("guard.checked = true;").count();
    assert_eq!(
        guard_writes, 2,
        "expected both in-scope-guard writes — `updater_check`'s and `silent_check`'s \
         `Ok(Some(update))` arms — got {guard_writes}"
    );
    let relocked_writes = MOD_RS
        .matches("app.state::<Mutex<UpdaterState>>().lock().checked = true")
        .count();
    assert_eq!(
        relocked_writes, 2,
        "expected both re-locked writes — `updater_check`'s and `silent_check`'s \
         `Ok(None)` arms — got {relocked_writes}"
    );
    // T2 hardening: the two counts above are position-independent — they
    // cannot tell WHICH match arm a write sits in, so moving `silent_check`'s
    // `Ok(None)` write into its `Err(_)` arm would leave both counts
    // unchanged. Pin the swallow arm directly: a failed background probe
    // must never be marked `checked`.
    assert!(
        MOD_RS.contains("Err(_) => {}"),
        "silent_check's failed-probe arm must stay a no-op — a `checked` \
         write here would claim a fresh answer after a failed check"
    );
}
