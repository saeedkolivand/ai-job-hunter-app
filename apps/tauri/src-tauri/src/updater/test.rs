use super::*;

#[test]
fn test_updater_state_default() {
    let state = UpdaterState::default();
    assert!(state.pending_version.is_none());
    assert!(state.downloaded_bytes.is_none());
}

#[test]
fn test_updater_state_with_data() {
    let state = UpdaterState {
        pending_version: Some("1.0.0".to_string()),
        downloaded_bytes: Some(vec![1, 2, 3]),
    };
    assert_eq!(state.pending_version, Some("1.0.0".to_string()));
    assert_eq!(state.downloaded_bytes, Some(vec![1, 2, 3]));
}

#[test]
fn test_updater_state_empty_bytes() {
    let state = UpdaterState {
        pending_version: None,
        downloaded_bytes: Some(vec![]),
    };
    assert_eq!(state.downloaded_bytes, Some(vec![]));
}

#[test]
fn test_updater_state_version_only() {
    let state = UpdaterState {
        pending_version: Some("2.5.0".to_string()),
        downloaded_bytes: None,
    };
    assert_eq!(state.pending_version, Some("2.5.0".to_string()));
    assert!(state.downloaded_bytes.is_none());
}

#[test]
fn test_updater_state_large_bytes() {
    let large_bytes: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    let state = UpdaterState {
        pending_version: None,
        downloaded_bytes: Some(large_bytes.clone()),
    };
    assert_eq!(state.downloaded_bytes, Some(large_bytes));
}

#[test]
fn test_updater_state_version_with_special_chars() {
    let state = UpdaterState {
        pending_version: Some("v1.0.0-beta.1".to_string()),
        downloaded_bytes: None,
    };
    assert_eq!(state.pending_version, Some("v1.0.0-beta.1".to_string()));
}

#[test]
fn test_updater_state_bytes_take() {
    let mut state = UpdaterState {
        pending_version: None,
        downloaded_bytes: Some(vec![1, 2, 3]),
    };
    let taken = state.downloaded_bytes.take();
    assert_eq!(taken, Some(vec![1, 2, 3]));
    assert!(state.downloaded_bytes.is_none());
}

#[test]
fn test_updater_state_version_take() {
    let mut state = UpdaterState {
        pending_version: Some("1.0.0".to_string()),
        downloaded_bytes: None,
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
        downloaded_bytes: None,
    };
    state.pending_version = Some("2.0.0".to_string());
    assert_eq!(state.pending_version, Some("2.0.0".to_string()));
}

#[test]
fn test_updater_state_bytes_replace() {
    let mut state = UpdaterState {
        pending_version: None,
        downloaded_bytes: Some(vec![1, 2, 3]),
    };
    state.downloaded_bytes = Some(vec![4, 5, 6]);
    assert_eq!(state.downloaded_bytes, Some(vec![4, 5, 6]));
}
