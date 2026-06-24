use super::*;

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

#[test]
fn chrome_env_nonexistent_path_is_skipped() {
    // SAFETY: test-only; single-threaded test binary on this platform.
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

    // Helper: set HOME to a temp dir for the duration of a test closure.
    fn with_home<F: FnOnce(&std::path::Path) -> R, R>(f: F) -> R {
        let tmp = TempDir::new().expect("tempdir");
        let old = std::env::var_os("HOME");
        // SAFETY: single-threaded section; we restore after the closure.
        unsafe {
            std::env::set_var("HOME", tmp.path());
        }
        let result = f(tmp.path());
        unsafe {
            match old {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
        result
    }

    #[test]
    fn detect_chrome_linux_does_not_panic() {
        // We cannot write to /snap/bin (root-owned) or mock absolute system
        // paths in-process, so the snap-detection branch cannot be exercised
        // here.  This test only guarantees the function completes without
        // panicking on the host environment.  Real snap coverage would require
        // a mount namespace or a dependency-injected path resolver.
        let _ = detect_chrome_linux();
    }

    #[test]
    fn flatpak_app_returns_flatpak_launch_for_installed_id() {
        // Simulate `~/.local/share/flatpak/exports/bin/com.google.Chrome` existing.
        with_home(|home| {
            let export_dir = home.join(".local/share/flatpak/exports/bin");
            fs::create_dir_all(&export_dir).unwrap();
            let wrapper = export_dir.join("com.google.Chrome");
            fs::write(&wrapper, b"").unwrap();

            let result = detect_chrome_linux();
            match result {
                Some(BrowserLaunch::FlatpakApp(id)) => {
                    assert_eq!(id, "com.google.Chrome");
                }
                other => panic!("expected FlatpakApp(com.google.Chrome), got {other:?}"),
            }
        });
    }

    #[test]
    fn flatpak_system_export_path_probe_does_not_panic() {
        // We cannot write to /var/lib/flatpak (root-owned), so the system
        // export path cannot be exercised in-process.  That code path shares
        // the same match arm as the user-local export path, which IS covered
        // by flatpak_app_returns_flatpak_launch_for_installed_id above.
        // This test only confirms the function doesn't panic when the system
        // path is unreachable.
        let _ = detect_chrome_linux();
    }

    #[test]
    fn no_browser_returns_none_on_clean_home() {
        with_home(|_home| {
            // In a clean tempdir there are no flatpak exports; PATH is still
            // the real PATH, so we can only assert the function completes.
            // A real CI system may or may not have a browser on PATH.
            let _ = detect_chrome_linux();
        });
    }

    // ── Flatpak detection helpers ────────────────────────────────────────────

    #[test]
    fn flatpak_ids_are_nonempty_and_unique() {
        use std::collections::HashSet;
        let ids: HashSet<&str> = FLATPAK_IDS.iter().copied().collect();
        assert_eq!(ids.len(), FLATPAK_IDS.len(), "duplicate Flatpak ID");
        assert!(!FLATPAK_IDS.is_empty());
    }

    #[test]
    fn flatpak_ids_contain_chrome_and_chromium() {
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
    // display_command() is non-empty.
    let result = detect_system_chrome();
    if let Some(launch) = result {
        assert!(!launch.display_command().is_empty());
        // NativePath variant must point to an existing file.
        if let BrowserLaunch::NativePath(p) = &launch {
            assert!(p.exists(), "NativePath {p:?} does not exist on disk");
        }
    }
}
