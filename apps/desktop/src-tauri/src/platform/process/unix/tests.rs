use std::fs;

use super::*;

// ── last_non_empty_line ───────────────────────────────────────────────────────

#[test]
fn last_non_empty_line_basic() {
    assert_eq!(
        last_non_empty_line("/usr/bin:/bin\n"),
        Some("/usr/bin:/bin".to_string())
    );
}

/// Regression: an interactive rc (oh-my-zsh, a fish greeting) prints a
/// banner/motd to stdout before the `printenv PATH` output. That banner must
/// never get glued onto PATH — only the last non-empty line counts.
#[test]
fn last_non_empty_line_skips_a_leading_banner() {
    let out = "Welcome to zsh!\nType 'help' for tips.\n\n/usr/bin:/bin:/opt/homebrew/bin\n";
    assert_eq!(
        last_non_empty_line(out),
        Some("/usr/bin:/bin:/opt/homebrew/bin".to_string())
    );
}

#[test]
fn last_non_empty_line_blank_input_is_none() {
    assert_eq!(last_non_empty_line(""), None);
    assert_eq!(last_non_empty_line("\n\n   \n"), None);
}

// ── nvm_bin_dirs ───────────────────────────────────────────────────────────────

#[test]
fn nvm_bin_dirs_expands_every_installed_version() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("v18.20.0")).unwrap();
    fs::create_dir_all(root.path().join("v20.11.0")).unwrap();
    fs::write(root.path().join("stray-file"), "").unwrap(); // not a version dir

    let dirs = nvm_bin_dirs(root.path());
    assert_eq!(
        dirs.len(),
        2,
        "a stray file under the nvm root must not be treated as a version dir"
    );
    assert!(dirs
        .iter()
        .any(|d| d.contains("v18.20.0") && d.ends_with("bin")));
    assert!(dirs
        .iter()
        .any(|d| d.contains("v20.11.0") && d.ends_with("bin")));
}

#[test]
fn nvm_bin_dirs_missing_root_is_empty() {
    let root = tempfile::tempdir().unwrap();
    assert!(nvm_bin_dirs(&root.path().join("does-not-exist")).is_empty());
}

// ── common_bin_dirs ──────────────────────────────────────────────────────────

/// `#[serial]`: mutates the process-global `HOME` that `common_bin_dirs` reads
/// — same discipline as `platform::config`'s own env tests.
#[test]
#[serial_test::serial]
fn common_bin_dirs_includes_the_node_manager_and_linuxbrew_fallbacks() {
    let home = tempfile::TempDir::new().unwrap();
    let _guard = crate::platform::config::HomeDirGuard::set(home.path());
    let home_str = home.path().to_string_lossy().into_owned();

    let dirs = common_bin_dirs();

    assert!(dirs.contains(&"/home/linuxbrew/.linuxbrew/bin".to_string()));
    assert!(dirs.contains(&"/snap/bin".to_string()));
    assert!(dirs.contains(&format!("{home_str}/.asdf/shims")));
    assert!(dirs.contains(&format!("{home_str}/.local/share/mise/shims")));
    assert!(dirs.contains(&format!("{home_str}/Library/pnpm")));
    assert!(dirs.contains(&format!("{home_str}/.local/share/pnpm")));
    assert!(dirs.contains(&format!("{home_str}/.fnm/aliases/default/bin")));
    assert!(dirs.contains(&format!("{home_str}/.local/share/fnm/aliases/default/bin")));
}

// ── cli_path cache ───────────────────────────────────────────────────────────

#[test]
#[serial_test::serial]
fn cli_path_serves_a_successful_probe_from_cache() {
    let seeded = OsString::from("/seeded/from/cache");
    *cache().lock() = Some(CliPathCache {
        path: Some(seeded.clone()),
        login_shell_ok: true,
        at: Instant::now(),
    });
    assert_eq!(cli_path(), Some(seeded));
}

/// A failed/timed-out login-shell probe must still be served from cache while
/// within its TTL, so a machine with a stuck login shell isn't re-probed (up
/// to a 4 s timeout) on every single CLI spawn. The instant is injected
/// (constructed, not slept to) so the test is instant and deterministic.
#[test]
#[serial_test::serial]
fn cli_path_serves_a_failed_probe_within_its_ttl() {
    let stale = OsString::from("/stale-but-within-ttl");
    *cache().lock() = Some(CliPathCache {
        path: Some(stale.clone()),
        login_shell_ok: false,
        at: Instant::now(),
    });
    assert_eq!(
        cli_path(),
        Some(stale),
        "a failed probe must be served from cache until FAILED_PROBE_TTL elapses"
    );
}

/// Regression: a failed/timed-out login-shell probe must not wedge `cli_path`
/// into permanently returning that stale result — once its TTL elapses, the
/// next call rebuilds.
#[test]
#[serial_test::serial]
fn cli_path_rebuilds_after_a_failed_probes_ttl_expires() {
    let stale = OsString::from("/stale-past-the-ttl");
    *cache().lock() = Some(CliPathCache {
        path: Some(stale.clone()),
        login_shell_ok: false,
        at: Instant::now() - (FAILED_PROBE_TTL + Duration::from_secs(1)),
    });
    assert_ne!(cli_path(), Some(stale));
}

#[test]
#[serial_test::serial]
fn reset_cli_path_cache_clears_a_cached_entry() {
    *cache().lock() = Some(CliPathCache {
        path: None,
        login_shell_ok: true,
        at: Instant::now(),
    });
    reset_cli_path_cache();
    assert!(cache().lock().is_none());
}
