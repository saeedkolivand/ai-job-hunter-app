use super::*;

#[test]
fn test_updater_state_default() {
    let state = UpdaterState::default();
    assert!(state.pending_version.is_none());
    assert!(state.pending_update.is_none());
    assert!(state.downloaded_bytes.is_none());
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
