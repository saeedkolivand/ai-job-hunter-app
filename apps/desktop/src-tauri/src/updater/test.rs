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

// ── Changelog parsing ────────────────────────────────────────────────────────

/// `major.minor.patch` as a tuple for order comparisons in tests only — not a
/// general semver parser (a pre-release suffix like `-beta.1` just truncates at
/// the first non-numeric segment, which is fine for this repo's plain versions).
fn version_tuple(v: &str) -> (u32, u32, u32) {
    let mut parts = v.split(['.', '-']).filter_map(|p| p.parse::<u32>().ok());
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

#[test]
fn test_parse_heading_with_date() {
    assert_eq!(
        parse_heading("## [1.2.3](url) (2026-01-01)\n"),
        Some(("1.2.3".to_string(), Some("2026-01-01".to_string())))
    );
}

#[test]
fn test_parse_heading_without_date() {
    assert_eq!(
        parse_heading("## [1.2.3](url)\n"),
        Some(("1.2.3".to_string(), None))
    );
}

#[test]
fn test_parse_heading_ignores_subsections_and_title() {
    assert_eq!(parse_heading("### ✨ Features\n"), None);
    assert_eq!(parse_heading("# Changelog\n"), None);
    assert_eq!(parse_heading("just prose\n"), None);
}

#[test]
fn test_parse_changelog_two_versions() {
    let raw = "# Changelog\n\n\
        ## [2.0.0](url) (2026-02-02)\n\n### Features\n\n* second\n\n\
        ## [1.0.0](url) (2026-01-01)\n\n### Features\n\n* first\n";
    let entries = parse_changelog(raw);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].version, "2.0.0");
    assert_eq!(entries[0].date.as_deref(), Some("2026-02-02"));
    assert!(entries[0].body.contains("second"));
    assert!(!entries[0].body.contains("first"));
    assert_eq!(entries[1].version, "1.0.0");
    assert!(entries[1].body.contains("first"));
}

#[test]
fn test_parse_changelog_malformed_yields_no_entries() {
    assert!(parse_changelog("not a changelog at all\njust some prose").is_empty());
}

#[test]
fn test_parse_changelog_empty_string() {
    assert!(parse_changelog("").is_empty());
}

#[test]
fn test_changelog_response_malformed_never_panics() {
    let result = changelog_response("# Changelog\n\nno version headings here\n");
    assert_eq!(
        result["error"].as_str(),
        Some("Changelog unavailable (bundled CHANGELOG.md has no releases).")
    );
}

#[test]
fn test_changelog_response_empty_never_panics() {
    let result = changelog_response("");
    assert!(result["error"].is_string());
}

/// Parses the real, bundled `CHANGELOG.md` — guards against a future format
/// change in the generator (or in this parser) silently emptying the changelog.
#[test]
fn test_parse_changelog_real_bundled_file() {
    let entries = parse_changelog(CHANGELOG_MD);
    assert!(
        !entries.is_empty(),
        "bundled CHANGELOG.md should yield at least one release"
    );
    assert!(
        entries[0].date.is_some(),
        "newest release should have a date"
    );
    for pair in entries.windows(2) {
        assert!(
            version_tuple(&pair[0].version) >= version_tuple(&pair[1].version),
            "expected releases newest-first, got {} before {}",
            pair[0].version,
            pair[1].version
        );
    }
}

#[test]
fn test_changelog_response_real_bundled_file() {
    let result = changelog_response(CHANGELOG_MD);
    let releases = result["releases"]
        .as_array()
        .expect("bundled changelog should produce releases");
    assert!(!releases.is_empty());
    assert!(releases.len() <= CHANGELOG_LIMIT);
    let first_version = releases[0]["version"].as_str().unwrap();
    assert!(first_version.chars().next().unwrap().is_ascii_digit());
    assert!(releases[0]["url"]
        .as_str()
        .unwrap()
        .contains(&format!("releases/tag/v{first_version}")));
}
