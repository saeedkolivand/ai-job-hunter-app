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

#[cfg(test)]
mod tests {
    use super::*;

    // Env-var override and default are exercised in a single test: `AJH_DATA_DIR`
    // is process-global, so splitting them into parallel tests races (one's
    // remove_var can land between the other's set_var and read).
    #[test]
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
}
