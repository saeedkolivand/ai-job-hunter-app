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

// ── status_reply: `updater_status`'s read-only reply (round 5,
// `B1-r1-ACLI-R5-1`) — no network, no `UpdaterState` write, no event ────────

#[test]
fn test_status_reply_unknown_when_nothing_pending_and_never_checked() {
    assert_eq!(
        status_reply(&UpdaterState::default(), false),
        json!({ "available": false, "checked": false })
    );
}

/// `B1-r2-ACLI-R6-2` — the sole regression this whole field exists to fix:
/// a caller must be able to tell "checked, genuinely current" apart from
/// "no check has ever run" / "the last one failed". Both used to be the
/// exact same `{"available": false}`.
#[test]
fn test_status_reply_checked_and_current_differs_from_never_checked() {
    let checked = UpdaterState {
        checked: true,
        ..UpdaterState::default()
    };
    let never_checked = UpdaterState::default();
    assert_eq!(
        status_reply(&checked, false),
        json!({ "available": false, "checked": true })
    );
    assert_ne!(
        status_reply(&checked, false),
        status_reply(&never_checked, false)
    );
}

/// A Store (MSIX) build never runs a network check at all — `checked` stays
/// `false` forever on that flavour, so without consulting `packaged` first
/// this reply would be indistinguishable from "never checked" on a build
/// that will NEVER check, rather than the store's own `managedBy` marker.
#[test]
fn test_status_reply_store_managed_wins_over_checked_state() {
    let state = UpdaterState {
        checked: true,
        ..UpdaterState::default()
    };
    assert_eq!(
        status_reply(&state, true),
        json!({ "available": false, "managedBy": "store" })
    );
}

#[test]
fn test_status_reply_available_with_the_pending_version_once_checked() {
    let state = UpdaterState {
        pending_version: Some("2.5.0".to_string()),
        ..UpdaterState::default()
    };
    assert_eq!(
        status_reply(&state, false),
        json!({ "available": true, "version": "2.5.0" })
    );
}

#[test]
fn test_status_reply_reads_pending_version_not_downloaded_bytes() {
    // A finished download still reports the PENDING version — `updater_install`'s proof source
    // reads it here, not off `downloaded_bytes`, which carries no version string of its own.
    let state = UpdaterState {
        pending_version: Some("3.0.0".to_string()),
        downloaded_bytes: Some(vec![1, 2, 3]),
        ..UpdaterState::default()
    };
    assert_eq!(
        status_reply(&state, false),
        json!({ "available": true, "version": "3.0.0" })
    );
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

// ── The `checked` producer side (`B1-r3-ACLI-R7-1`) ─────────────────────────

/// Every `status_reply` test above is pure — it reads `UpdaterState.checked`,
/// never sets it — so deleting all four production writes (`updater_check`'s
/// two `Ok(...)` arms, `silent_check`'s two mirrors) left the whole suite
/// green while `updater_status` would answer "never checked" forever on a
/// build that checks every 4 h. This crate has no `tauri::test` mock-app
/// harness (see the doc comments on [`download_in_progress_or_done`] and
/// [`store_managed`]), so the producer side is pinned at the SOURCE rather
/// than by driving the async commands: deleting any of the four writes fails
/// this test even though every `status_reply` test above stays green.
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

// ── Microsoft Store flavour ───────────────────────────────────────────────────

#[test]
fn test_store_managed_only_when_packaged() {
    assert!(
        store_managed(false).is_none(),
        "an NSIS/MSI install must keep checking GitHub"
    );
    assert!(
        store_managed(true).is_some(),
        "a Store install must never check GitHub"
    );
}

/// Anchored on the FIELDS — the thing `UpdateCheckResult` in
/// `packages/shared/src/ipc/contracts/updater.ts` actually declares — so a
/// renamed or dropped field fails while a serializer that reorders keys does
/// not. (Comparing serialized strings would invent a key-order invariant the
/// IPC contract does not have.)
#[test]
fn test_store_managed_has_the_contract_shape() {
    assert_eq!(
        store_managed(true).unwrap(),
        json!({ "available": false, "managedBy": "store" })
    );
}

/// The pushed shape the renderer's `managed` status variant matches on.
#[test]
fn test_managed_status_has_the_contract_shape() {
    assert_eq!(
        managed_status(),
        json!({ "state": "managed", "by": "store" })
    );
}

/// A Store build's download/install refusal is an `error` reply — the shape the
/// renderer already renders — not a silent no-op that would look like success.
#[test]
fn test_store_managed_refusal_is_an_error_reply() {
    let refusal = store_managed_refusal();
    assert!(refusal
        .get("error")
        .and_then(|e| e.as_str())
        .is_some_and(|m| m.contains("Store")));
}
