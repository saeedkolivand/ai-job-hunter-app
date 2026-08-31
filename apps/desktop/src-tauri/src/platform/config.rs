//! Centralized configuration & path resolution.
//!
//! This module is the **sole owner** of the application data-directory env var
//! and its filesystem fallback. No other module may read `AJH_DATA_DIR` or
//! reconstruct the `~/.ajh` path — a CI guardrail enforces this.
//!
//! Two resolution contexts exist and must agree on the same directory:
//! * **Setup** (`resolve_and_export_data_dir`) has an `AppHandle` and uses
//!   Tauri's authoritative `app_data_dir()`, exporting it so workers can find it.
//! * **Workers** (`data_dir`) — scrapers/appliers run in spawned tasks with no
//!   `AppHandle`, so they read the exported env var (falling back to `~/.ajh`).

use std::path::PathBuf;

/// Env var carrying the resolved data dir to AppHandle-less workers.
const DATA_DIR_ENV: &str = "AJH_DATA_DIR";

/// Fallback directory name under the user's home when the env var is unset.
const FALLBACK_DIR_NAME: &str = ".ajh";

/// Worker-side resolver (no `AppHandle`). The single copy of this logic.
///
/// Reads `AJH_DATA_DIR` (exported by [`resolve_and_export_data_dir`] at setup),
/// falling back to `<home>/.ajh` where home is `USERPROFILE` (Windows) or `HOME`.
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(DATA_DIR_ENV) {
        return PathBuf::from(dir);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(FALLBACK_DIR_NAME)
}

/// The current user's home directory, if resolvable. Centralizes the raw env
/// read for the few callers (e.g. the native-messaging host registration,
/// the agent-CLI pointer file) that need OS-mandated well-known directories
/// outside the app data dir, so the R4 "env access only in platform/**" rule
/// holds.
///
/// Checks `USERPROFILE` (Windows) before `HOME` — mirrors [`data_dir`]'s own
/// fallback order. `HOME` alone is unset on Windows, so a Windows-only caller
/// of the old `$HOME`-only version silently never resolved a home dir at all
/// (its one caller at the time was already `#[cfg(not(windows))]`, so nothing
/// behavioral changes here — this only widens what future/other-OS callers,
/// like the agent-CLI pointer, can rely on).
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Directory holding the agent-CLI pointer file — a SIBLING of, and
/// deliberately NOT reusing, [`FALLBACK_DIR_NAME`]: a foreign process reading
/// this pointer must never mistake this directory for the app's actual data
/// dir (which [`resolve_and_export_data_dir`] resolves via Tauri and may live
/// somewhere else entirely, e.g. `%APPDATA%` on Windows).
const AGENT_POINTER_DIR_NAME: &str = ".ajh-agent";
/// Filename of the pointer JSON itself (`{ exePath, dataDir }`) inside
/// [`AGENT_POINTER_DIR_NAME`].
const AGENT_POINTER_FILE_NAME: &str = "agent.json";

/// Path to the agent-CLI pointer file, if a home dir is resolvable. Shared by
/// the writer (`extension_bridge::register`, on every launch) and the reader
/// (`extension_bridge::agent_cli`, a foreign process with no `AJH_DATA_DIR`
/// and no `AppHandle`) so both agree on the same location without either
/// reconstructing it independently.
pub fn agent_pointer_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(AGENT_POINTER_DIR_NAME).join(AGENT_POINTER_FILE_NAME))
}

/// Setup-side resolver (has an `AppHandle`). Resolves the authoritative app data
/// dir via Tauri and exports it as `AJH_DATA_DIR` so AppHandle-less workers
/// resolve the same path. Returns the resolved directory.
pub fn resolve_and_export_data_dir(app: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    let dir = app.path().app_data_dir().unwrap_or_else(|_| data_dir());
    if std::env::var_os(DATA_DIR_ENV).is_none() {
        // SAFETY: single writer, called once during setup before any worker spawns.
        unsafe {
            std::env::set_var(DATA_DIR_ENV, &dir);
        }
    }
    dir
}

const OLLAMA_HOST_ENV: &str = "OLLAMA_HOST";
const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1:11434";

pub fn ollama_host() -> String {
    std::env::var(OLLAMA_HOST_ENV).unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.to_string())
}

/// Env var carrying extra allowed `Origin`s for the extension-bridge WS
/// handshake (comma-separated, e.g.
/// `chrome-extension://abc...,moz-extension://def...`). DEV-ONLY: an unpacked
/// extension gets a fresh, machine-specific id each load, so the published-id
/// allowlist can't cover local development — this lets a developer pin their
/// local id without code edits. The centralized-config rule forbids reading env
/// outside `platform/`, so the bridge calls this helper instead of `std::env`.
const EXTENSION_DEV_ORIGINS_ENV: &str = "AJH_EXTENSION_DEV_ORIGINS";

/// Extra extension origins to allow during development. Empty in a normal
/// install (the var is unset). Entries are trimmed; blanks are dropped.
pub fn extension_dev_origins() -> Vec<String> {
    std::env::var(EXTENSION_DEV_ORIGINS_ENV)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Read an arbitrary process env var by key, returning `None` when unset or
/// non-UTF-8. The centralized home for ad-hoc env reads (e.g. a CLI agent's
/// `<AGENT>_BIN` binary-path override) so the R4 "env access only in platform/**"
/// rule holds without each caller touching `std::env`.
pub fn env_override(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Test-only RAII scope for the data dir, so a test needing an isolated one
/// does not have to touch `AJH_DATA_DIR` itself.
///
/// This module's own doc calls it the **sole owner** of that variable, and R4
/// enforces it — but R4's test exempts test files, so the rule was enforceable
/// everywhere except the place a test would actually reach for it. This closes
/// that gap: a caller names a directory, never the variable.
///
/// Restores on `Drop` rather than at the end of the test body, so a panicking
/// or early-returning assertion cannot leak the override into whatever runs
/// next. Callers still need `#[serial]` — the variable is process-global, and
/// a guard cannot serialize what it cannot see.
#[cfg(test)]
pub(crate) struct DataDirGuard(Option<std::ffi::OsString>);

#[cfg(test)]
impl DataDirGuard {
    /// Point `data_dir()` at `path` until the guard drops.
    pub(crate) fn set(path: &std::path::Path) -> Self {
        let previous = std::env::var_os(DATA_DIR_ENV);
        // SAFETY: test-only, and callers hold `#[serial]`.
        unsafe { std::env::set_var(DATA_DIR_ENV, path) };
        Self(previous)
    }
}

#[cfg(test)]
impl Drop for DataDirGuard {
    fn drop(&mut self) {
        // SAFETY: as above — restoring exactly what was read in `set`.
        match self.0.take() {
            Some(previous) => unsafe { std::env::set_var(DATA_DIR_ENV, previous) },
            None => unsafe { std::env::remove_var(DATA_DIR_ENV) },
        }
    }
}

/// Test-only RAII scope for BOTH home-dir env vars (`USERPROFILE` + `HOME`) —
/// same reason and shape as [`DataDirGuard`], extended to a pair because
/// [`home_dir`] checks `USERPROFILE` first: a test overriding only `HOME`
/// would silently lose to a real `USERPROFILE` on a Windows host. Any OTHER
/// crate module (e.g. `extension_bridge::register`'s agent-pointer tests)
/// that needs an isolated home dir goes through this rather than touching
/// `std::env` itself, so R4 ("env access only in platform/**") holds even
/// for tests embedded in a non-`platform` file (R4's own text-scan exempts
/// only files whose name ends in `test(s).rs`, not an inline `#[cfg(test)]`
/// module — the same gap `DataDirGuard`'s doc calls out).
#[cfg(test)]
pub(crate) struct HomeDirGuard {
    prev_userprofile: Option<std::ffi::OsString>,
    prev_home: Option<std::ffi::OsString>,
}

#[cfg(test)]
impl HomeDirGuard {
    /// Point `home_dir()` (and therefore `agent_pointer_path()`) at `path`
    /// until the guard drops.
    pub(crate) fn set(path: &std::path::Path) -> Self {
        let prev_userprofile = std::env::var_os("USERPROFILE");
        let prev_home = std::env::var_os("HOME");
        // SAFETY: test-only, and callers hold `#[serial]`.
        unsafe {
            std::env::set_var("USERPROFILE", path);
            std::env::set_var("HOME", path);
        }
        Self {
            prev_userprofile,
            prev_home,
        }
    }
}

#[cfg(test)]
impl Drop for HomeDirGuard {
    fn drop(&mut self) {
        // SAFETY: as above — restoring exactly what was read in `set`.
        unsafe {
            match self.prev_userprofile.take() {
                Some(previous) => std::env::set_var("USERPROFILE", previous),
                None => std::env::remove_var("USERPROFILE"),
            }
            match self.prev_home.take() {
                Some(previous) => std::env::set_var("HOME", previous),
                None => std::env::remove_var("HOME"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Env-var override and default are exercised in a single test: `AJH_DATA_DIR`
    // is process-global, so splitting them into parallel tests races (one's
    // remove_var can land between the other's set_var and read).
    // `#[serial]` because this mutates the process-global var directly (it is
    // testing the resolver itself, so it cannot go through `DataDirGuard`).
    // Without it, it races any other `#[serial]` mutator — which was a real
    // gap once a second test elsewhere began scoping the same variable.
    #[test]
    #[serial_test::serial]
    fn data_dir_honors_env_then_falls_back() {
        // Override via env var.
        unsafe {
            std::env::set_var(DATA_DIR_ENV, "/custom/path");
        }
        assert_eq!(data_dir().to_string_lossy(), "/custom/path");

        // Default falls back to USERPROFILE/HOME and ends with .ajh.
        unsafe {
            std::env::remove_var(DATA_DIR_ENV);
        }
        assert!(data_dir().to_string_lossy().contains(FALLBACK_DIR_NAME));
    }

    // `home_dir`/`USERPROFILE` mutate process-global env — `#[serial]` so this
    // can't race `data_dir_honors_env_then_falls_back` (a different var) or any
    // other `#[serial]` mutator in this module.
    #[test]
    #[serial_test::serial]
    fn home_dir_honors_userprofile_before_home() {
        // `HomeDirGuard`'s `Drop` restores whatever `USERPROFILE`/`HOME` were
        // BEFORE this test ran, even on a panicking assert below (MEDIUM fix
        // — security review): the manual save/restore block this replaces
        // only ran on a normal return, so a failing assert used to leak
        // mutated env into every later test in this process. The path here
        // is never read (this test overwrites both vars immediately below,
        // for each state it exercises) — only the guard's captured
        // "restore to" values matter.
        let _guard = HomeDirGuard::set(std::path::Path::new("/unused-initial"));

        // USERPROFILE wins when both are set (the Windows case: HOME is
        // typically unset there, but this pins the precedence regardless).
        unsafe {
            std::env::set_var("USERPROFILE", "/from/userprofile");
            std::env::set_var("HOME", "/from/home");
        }
        assert_eq!(home_dir().unwrap().to_string_lossy(), "/from/userprofile");

        // HOME alone (USERPROFILE unset) — the pre-fix behavior, still honored.
        unsafe {
            std::env::remove_var("USERPROFILE");
        }
        assert_eq!(home_dir().unwrap().to_string_lossy(), "/from/home");

        // Neither set — this is what silently broke on Windows before the fix
        // (HOME-only never resolved there).
        unsafe {
            std::env::remove_var("HOME");
        }
        assert_eq!(home_dir(), None);
    }

    #[test]
    #[serial_test::serial]
    fn agent_pointer_path_sits_beside_not_inside_the_data_dir_fallback() {
        let home = std::path::Path::new("/home/tester");
        let _guard = HomeDirGuard::set(home);
        let path = agent_pointer_path().unwrap();
        assert_eq!(
            path,
            home.join(AGENT_POINTER_DIR_NAME)
                .join(AGENT_POINTER_FILE_NAME)
        );
        // Never the bare FALLBACK_DIR_NAME (`.ajh`) — that name is the data-dir
        // fallback, and a pointer file living there would be mistakable for it.
        assert!(!path.starts_with(home.join(FALLBACK_DIR_NAME)));
    }
}
