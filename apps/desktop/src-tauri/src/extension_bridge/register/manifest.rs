//! Host-manifest JSON building + the generic best-effort disk-write helpers — split from
//! `register.rs` (R8 relief). Pure/IO-only primitives; the per-OS PLACEMENT of these manifests
//! lives in the sibling `register_windows`/`register_unix` modules.

use std::path::Path;

use serde_json::json;

use super::super::NATIVE_HOST_NAME;

/// Firefox gecko id (AMO `allowed_extensions` entry). MUST match the extension's
/// manifest `browser_specific_settings.gecko.id`.
const FIREFOX_GECKO_ID: &str = "job-importer@aijobhunter.app";

/// Chrome allow-listed extension origin. Chrome requires the trailing slash.
const CHROME_ALLOWED_ORIGIN: &str = "chrome-extension://oaoekkgkhmgdfnpmfkpphgiikliaicll/";

const DESCRIPTION: &str = "AI Job Hunter browser bridge";

/// Build the JSON bytes for one host manifest. `serde_json` escapes Windows
/// backslashes in the exe path correctly.
pub(super) fn manifest_json(exe: &Path, firefox: bool) -> Vec<u8> {
    let path = exe.to_string_lossy();
    let allow = if firefox {
        json!({ "allowed_extensions": [FIREFOX_GECKO_ID] })
    } else {
        json!({ "allowed_origins": [CHROME_ALLOWED_ORIGIN] })
    };
    // Merge the common fields with the browser-specific allow-list.
    let mut obj = json!({
        "name": NATIVE_HOST_NAME,
        "description": DESCRIPTION,
        "path": path,
        "type": "stdio",
    });
    if let (Some(map), Some(extra)) = (obj.as_object_mut(), allow.as_object()) {
        for (k, v) in extra {
            map.insert(k.clone(), v.clone());
        }
    }
    serde_json::to_vec_pretty(&obj).unwrap_or_default()
}

/// Write `bytes` to `path`, creating parent dirs. Best-effort: logs + returns on
/// any failure.
///
/// `label` (e.g. `"chrome"`, `"vivaldi (flatpak)"`) identifies which manifest
/// this was for a caller reading the log; `path` itself is NEVER logged. Every
/// path here is under the user's home directory, and several browsers share the
/// same manifest file NAME in different directories (so even `file_name()`
/// would be ambiguous) — a caller-supplied label is both the safe choice and
/// the more precise one. These `log::warn!` calls end up in diagnostics bundles
/// users send us, and a path there would leak the OS username.
pub(super) fn write_manifest(label: &str, path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!(
                "[native_host] mkdir for {label} manifest failed (non-fatal): {}",
                crate::observability::sanitize_reason(&e.to_string())
            );
            return;
        }
    }
    if let Err(e) = std::fs::write(path, bytes) {
        log::warn!(
            "[native_host] write {label} manifest failed (non-fatal): {}",
            crate::observability::sanitize_reason(&e.to_string())
        );
    }
}

/// Write `bytes` to `path` only when `guard_dir` already exists on disk.
///
/// Used for Flatpak per-app config paths: `~/.var/app/<id>` is created by
/// Flatpak only when the app is installed. If that directory is absent we skip
/// silently — no directories are created, so we don't leave ghost paths for
/// browsers that aren't installed.
// Used only in the Linux branch (Flatpak guard logic); the cfg mirrors the
// call-sites so the compiler doesn't emit a dead_code warning on other platforms.
#[cfg(any(target_os = "linux", test))]
pub(super) fn write_manifest_if_app_dir_exists(
    label: &str,
    guard_dir: &Path,
    path: &Path,
    bytes: &[u8],
) {
    if !guard_dir.exists() {
        return;
    }
    write_manifest(label, path, bytes);
}

#[cfg(test)]
mod tests;
