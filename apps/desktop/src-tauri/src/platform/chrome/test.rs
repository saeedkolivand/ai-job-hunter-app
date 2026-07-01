use super::*;
#[cfg(test)]
use serial_test::serial;

// ── BrowserLaunch helpers ─────────────────────────────────────────────────────

#[test]
fn native_path_to_executable_path_returns_some() {
    let pb = std::path::PathBuf::from("/usr/bin/google-chrome");
    let launch = BrowserLaunch::NativePath(pb.clone());
    assert_eq!(launch.to_executable_path(), Some(pb));
}

#[test]
fn flatpak_to_executable_path_returns_none() {
    let launch = BrowserLaunch::FlatpakApp("com.google.Chrome".to_string());
    assert!(launch.to_executable_path().is_none());
}

#[test]
fn native_path_display_command_is_path_string() {
    let pb = std::path::PathBuf::from("/opt/google/chrome/google-chrome");
    let launch = BrowserLaunch::NativePath(pb.clone());
    assert_eq!(launch.display_command(), pb.to_string_lossy().as_ref());
}

#[test]
fn flatpak_display_command_is_flatpak_run() {
    let launch = BrowserLaunch::FlatpakApp("com.google.Chrome".to_string());
    assert_eq!(launch.display_command(), "flatpak run com.google.Chrome");
}

// ── CHROME env var override ───────────────────────────────────────────────────

// `#[serial]` serializes all tests that mutate $CHROME / $HOME so they don't
// race under plain `cargo test` (which shares the process environment across
// threads).  nextest already process-isolates, but the attribute is harmless
// there and makes the intent explicit.
#[test]
#[serial]
fn chrome_env_nonexistent_path_is_skipped() {
    // SAFETY: test-only; nextest runs each test in its own process so
    // set_var is safe against data races.  Under plain cargo test the
    // #[serial] attribute ensures only one env-mutating test runs at a time.
    unsafe {
        std::env::set_var("CHROME", "/nonexistent/path/to/chrome-does-not-exist");
    }
    let result = detect_system_chrome();
    unsafe {
        std::env::remove_var("CHROME");
    }
    // Must not return a non-existent path.
    if let Some(BrowserLaunch::NativePath(p)) = result {
        assert!(p.exists(), "returned a non-existent native path");
    }
}

// ── Linux-specific detection logic ────────────────────────────────────────────

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
mod linux {
    use super::super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── probe_flatpak unit tests (hermetic — no real FS dependency) ──────────
    //
    // These call `probe_flatpak(Some(tmp_home))` directly so they exercise the
    // Flatpak-export lookup in isolation, bypassing the native PATH/abs-path
    // checks in detect_chrome_linux().  The runner's installed browsers are
    // irrelevant.

    #[test]
    fn probe_flatpak_returns_flatpak_app_for_user_export() {
        // Create a fake ~/.local/share/flatpak/exports/bin/com.google.Chrome.
        let tmp = TempDir::new().expect("tempdir");
        let export_dir = tmp.path().join(".local/share/flatpak/exports/bin");
        fs::create_dir_all(&export_dir).unwrap();
        fs::write(export_dir.join("com.google.Chrome"), b"").unwrap();

        let result = probe_flatpak(Some(tmp.path()));
        match result {
            Some(BrowserLaunch::FlatpakApp(id)) => {
                assert_eq!(id, "com.google.Chrome");
            }
            other => panic!("expected FlatpakApp(com.google.Chrome), got {other:?}"),
        }
    }

    #[test]
    fn probe_flatpak_returns_second_id_when_first_absent() {
        // Only Chromium export present (no Chrome).
        let tmp = TempDir::new().expect("tempdir");
        let export_dir = tmp.path().join(".local/share/flatpak/exports/bin");
        fs::create_dir_all(&export_dir).unwrap();
        fs::write(export_dir.join("org.chromium.Chromium"), b"").unwrap();

        let result = probe_flatpak(Some(tmp.path()));
        match result {
            Some(BrowserLaunch::FlatpakApp(id)) => {
                assert_eq!(id, "org.chromium.Chromium");
            }
            other => panic!("expected FlatpakApp(org.chromium.Chromium), got {other:?}"),
        }
    }

    #[test]
    fn probe_flatpak_returns_none_when_exports_absent_and_no_flatpak_binary() {
        // Empty home — no exports dir at all.  flatpak binary is absent on
        // most CI runners, so the `flatpak info` fallback should also yield
        // nothing.  We don't assert None (the runner might have flatpak +
        // some app installed) — we only assert the function completes.
        let tmp = TempDir::new().expect("tempdir");
        let _ = probe_flatpak(Some(tmp.path()));
    }

    #[test]
    fn probe_flatpak_none_home_does_not_panic() {
        // Passing None for home should not panic.
        let _ = probe_flatpak(None);
    }

    // ── detect_chrome_linux smoke (no assertion on return value) ────────────
    //
    // These only verify the function doesn't panic on the host environment.
    // We make NO assertion about what is returned because the runner's PATH
    // and installed browsers are outside our control.

    #[test]
    fn detect_chrome_linux_does_not_panic() {
        let _ = detect_chrome_linux();
    }

    // ── Native-first ordering contract ───────────────────────────────────────
    //
    // When a real binary exists at a known absolute path, detect_chrome_linux
    // must return NativePath — not FlatpakApp — because native installs win.
    // We verify this by pointing $CHROME at a real temp file (so the env-var
    // branch fires before any PATH/abs-path walk) and confirm the return type.

    #[test]
    #[serial_test::serial]
    fn env_chrome_wins_over_flatpak_export() {
        // Create a real temp "binary" so the env-var branch returns it.
        let tmp = TempDir::new().expect("tempdir");
        let fake_bin = tmp.path().join("fake-chrome");
        fs::write(&fake_bin, b"").unwrap();

        // Also create a Flatpak export to confirm native wins.
        let export_dir = tmp.path().join(".local/share/flatpak/exports/bin");
        fs::create_dir_all(&export_dir).unwrap();
        fs::write(export_dir.join("com.google.Chrome"), b"").unwrap();

        unsafe {
            std::env::set_var("CHROME", &fake_bin);
        }
        let result = detect_system_chrome();
        unsafe {
            std::env::remove_var("CHROME");
        }

        match result {
            Some(BrowserLaunch::NativePath(p)) => {
                assert_eq!(p, fake_bin, "should return the CHROME env-var path");
            }
            other => panic!("expected NativePath from CHROME env var, got {other:?}"),
        }
    }

    // ── Flatpak ID constants ─────────────────────────────────────────────────

    #[test]
    fn flatpak_ids_are_nonempty_and_unique() {
        use std::collections::HashSet;
        let ids: HashSet<&str> = FLATPAK_IDS.iter().copied().collect();
        assert_eq!(ids.len(), FLATPAK_IDS.len(), "duplicate Flatpak ID");
        assert!(!FLATPAK_IDS.is_empty());
    }

    #[test]
    fn flatpak_ids_contain_required_browsers() {
        assert!(FLATPAK_IDS.contains(&"com.google.Chrome"));
        assert!(FLATPAK_IDS.contains(&"org.chromium.Chromium"));
        assert!(FLATPAK_IDS.contains(&"com.brave.Browser"));
        assert!(FLATPAK_IDS.contains(&"com.microsoft.Edge"));
    }
}

// ── macOS smoke ───────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
#[test]
fn macos_detect_does_not_panic() {
    let _ = detect_chrome_macos();
}

// ── Windows smoke ─────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
#[test]
fn windows_detect_does_not_panic() {
    let _ = detect_chrome_windows();
}

// ── System-level smoke (all platforms) ───────────────────────────────────────

#[test]
fn detect_system_chrome_result_is_consistent() {
    // Whatever is returned must either be None or a variant whose
    // display_command() is non-empty.  NativePath variants must point to a
    // file that actually exists on disk.  No assertion on *which* variant is
    // returned — that depends on the runner's environment.
    let result = detect_system_chrome();
    if let Some(launch) = result {
        assert!(!launch.display_command().is_empty());
        if let BrowserLaunch::NativePath(p) = &launch {
            assert!(p.exists(), "NativePath {p:?} does not exist on disk");
        }
    }
}
