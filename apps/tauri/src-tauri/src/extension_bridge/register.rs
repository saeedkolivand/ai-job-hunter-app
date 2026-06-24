//! Native-messaging host registration — writes the per-browser host manifests
//! and (on Windows) the HKCU registry pointers so Firefox/Chrome can find and
//! spawn our relay ([`super::native_host`]).
//!
//! Called best-effort from the Tauri `setup` on every launch ([`register_native_host`]):
//! idempotent and overwriting, so `path` (= the current exe) tracks app moves and
//! updates. NEVER panics or propagates to boot — every step logs a warning on
//! failure and continues.
//!
//! ## What gets written
//! A host manifest is a small JSON file naming the host, its `stdio` type, the
//! absolute exe `path`, and the browser-specific allow-list:
//! - **Firefox** uses `"allowed_extensions": ["<gecko-id>"]`.
//! - **Chrome**  uses `"allowed_origins": ["chrome-extension://<id>/"]` (Chrome
//!   requires the trailing slash).
//!
//! Placement is OS- + browser-specific (see [`register_native_host`]). On Windows
//! the browser finds the manifest via an HKCU registry value; on macOS/Linux it
//! reads a fixed well-known directory directly (no registry).

use std::path::Path;

use serde_json::json;

use super::NATIVE_HOST_NAME;
// Only the non-Windows branches read the manifest filename; on Windows the path
// is derived from `NATIVE_HOST_NAME` instead (the registry value points at it).
#[cfg(not(windows))]
use super::NATIVE_HOST_MANIFEST;

/// Firefox gecko id (AMO `allowed_extensions` entry). MUST match the extension's
/// manifest `browser_specific_settings.gecko.id`.
const FIREFOX_GECKO_ID: &str = "job-importer@aijobhunter.app";

/// Chrome allow-listed extension origin. Chrome requires the trailing slash.
const CHROME_ALLOWED_ORIGIN: &str = "chrome-extension://oaoekkgkhmgdfnpmfkpphgiikliaicll/";

const DESCRIPTION: &str = "AI Job Hunter browser bridge";

/// Build the JSON bytes for one host manifest. `serde_json` escapes Windows
/// backslashes in the exe path correctly.
fn manifest_json(exe: &Path, firefox: bool) -> Vec<u8> {
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
fn write_manifest(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!(
                "[native_host] mkdir {} failed (non-fatal): {e}",
                parent.display()
            );
            return;
        }
    }
    if let Err(e) = std::fs::write(path, bytes) {
        log::warn!(
            "[native_host] write {} failed (non-fatal): {e}",
            path.display()
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
fn write_manifest_if_app_dir_exists(guard_dir: &Path, path: &Path, bytes: &[u8]) {
    if !guard_dir.exists() {
        return;
    }
    write_manifest(path, bytes);
}

/// Register the native-messaging host for Firefox + Chrome. Best-effort and
/// idempotent — safe to call on every launch.
pub fn register_native_host(data_dir: &Path) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            log::warn!("[native_host] current_exe() failed (non-fatal): {e}");
            return;
        }
    };
    let firefox_json = manifest_json(&exe, true);
    let chrome_json = manifest_json(&exe, false);

    #[cfg(windows)]
    {
        // Windows: the manifest location is arbitrary; the browser is pointed at
        // it by an HKCU registry value. Keep both under the app data dir.
        let dir = data_dir.join("native-messaging");
        let firefox_path = dir.join(format!("{NATIVE_HOST_NAME}.firefox.json"));
        let chrome_path = dir.join(format!("{NATIVE_HOST_NAME}.chrome.json"));
        write_manifest(&firefox_path, &firefox_json);
        write_manifest(&chrome_path, &chrome_json);

        let firefox_key = format!("Software\\Mozilla\\NativeMessagingHosts\\{NATIVE_HOST_NAME}");
        let chrome_key =
            format!("Software\\Google\\Chrome\\NativeMessagingHosts\\{NATIVE_HOST_NAME}");
        if let Err(e) = set_hkcu_default(&firefox_key, &firefox_path.to_string_lossy()) {
            log::warn!("[native_host] HKCU firefox key failed (non-fatal): {e}");
        }
        if let Err(e) = set_hkcu_default(&chrome_key, &chrome_path.to_string_lossy()) {
            log::warn!("[native_host] HKCU chrome key failed (non-fatal): {e}");
        }
    }

    #[cfg(not(windows))]
    {
        let _ = data_dir; // unused off Windows — browsers read fixed well-known dirs
        let Some(home) = crate::platform::config::home_dir() else {
            log::warn!("[native_host] HOME unset — skipping host-manifest registration");
            return;
        };

        #[cfg(target_os = "macos")]
        {
            let firefox_path = home
                .join("Library/Application Support/Mozilla/NativeMessagingHosts")
                .join(NATIVE_HOST_MANIFEST);
            let chrome_path = home
                .join("Library/Application Support/Google/Chrome/NativeMessagingHosts")
                .join(NATIVE_HOST_MANIFEST);
            write_manifest(&firefox_path, &firefox_json);
            write_manifest(&chrome_path, &chrome_json);
        }

        #[cfg(target_os = "linux")]
        {
            // ── Native (non-sandboxed) browser paths ─────────────────────────
            let firefox_path = home
                .join(".mozilla/native-messaging-hosts")
                .join(NATIVE_HOST_MANIFEST);
            let chrome_path = home
                .join(".config/google-chrome/NativeMessagingHosts")
                .join(NATIVE_HOST_MANIFEST);
            let chromium_path = home
                .join(".config/chromium/NativeMessagingHosts")
                .join(NATIVE_HOST_MANIFEST);
            let brave_path = home
                .join(".config/BraveSoftware/Brave-Browser/NativeMessagingHosts")
                .join(NATIVE_HOST_MANIFEST);
            let edge_path = home
                .join(".config/microsoft-edge/NativeMessagingHosts")
                .join(NATIVE_HOST_MANIFEST);
            write_manifest(&firefox_path, &firefox_json);
            write_manifest(&chrome_path, &chrome_json);
            write_manifest(&chromium_path, &chrome_json);
            write_manifest(&brave_path, &chrome_json);
            write_manifest(&edge_path, &chrome_json);

            // ── Flatpak per-app config dirs ───────────────────────────────────
            // Sandboxed Flatpak browsers cannot read ~/.config; they read their
            // own per-app dir at ~/.var/app/<id>/config/…/NativeMessagingHosts/.
            // We guard on the per-app root (~/.var/app/<id>) — that directory
            // exists only when the Flatpak is installed, so we never create
            // ghost paths for absent browsers.
            let flatpak_base = home.join(".var/app");
            struct FlatpakEntry {
                app_id: &'static str,
                manifest_rel: &'static str,
                firefox: bool,
            }
            let flatpak_entries: &[FlatpakEntry] = &[
                FlatpakEntry {
                    app_id: "com.google.Chrome",
                    manifest_rel: "config/google-chrome/NativeMessagingHosts",
                    firefox: false,
                },
                FlatpakEntry {
                    app_id: "org.chromium.Chromium",
                    manifest_rel: "config/chromium/NativeMessagingHosts",
                    firefox: false,
                },
                FlatpakEntry {
                    app_id: "com.brave.Browser",
                    manifest_rel: "config/BraveSoftware/Brave-Browser/NativeMessagingHosts",
                    firefox: false,
                },
                FlatpakEntry {
                    app_id: "com.microsoft.Edge",
                    manifest_rel: "config/microsoft-edge/NativeMessagingHosts",
                    firefox: false,
                },
                // NECESSARY BUT NOT SUFFICIENT for sandboxed Firefox Flatpak:
                // Writing the manifest here is correct and harmless, but a
                // sandboxed Firefox Flatpak cannot spawn the native-messaging
                // host binary because that binary lives outside the Flatpak
                // sandbox.  The user must either run:
                //
                //   flatpak override --user --filesystem=host org.mozilla.firefox
                //
                // or wait for portal-based native-messaging support.  Without
                // this override, Firefox Flatpak will find the manifest but fail
                // to execute the host.
                //
                // Chromium-family Flatpaks (Chrome/Chromium/Brave/Edge) generally
                // ship with broader filesystem access so manifest-only placement
                // works for them without an override.
                //
                // Note: the loopback WebSocket bridge path is unaffected by this
                // sandbox limitation — it connects via TCP, not stdio.
                FlatpakEntry {
                    app_id: "org.mozilla.firefox",
                    manifest_rel: ".mozilla/native-messaging-hosts",
                    firefox: true,
                },
            ];
            for entry in flatpak_entries {
                let guard = flatpak_base.join(entry.app_id);
                let path = guard.join(entry.manifest_rel).join(NATIVE_HOST_MANIFEST);
                let bytes = if entry.firefox {
                    &firefox_json
                } else {
                    &chrome_json
                };
                write_manifest_if_app_dir_exists(&guard, &path, bytes);
            }
        }
    }
}

/// Set the default (`""`) value of an HKCU subkey to `value` (REG_SZ). Creates
/// the key if absent. Encapsulates the raw Win32 FFI; the registry crate would
/// be a new dependency, so this hand-rolls `RegCreateKeyExW` + `RegSetValueExW`
/// against the already-present `windows` crate.
#[cfg(windows)]
fn set_hkcu_default(subkey: &str, value: &str) -> std::io::Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    // UTF-16, NUL-terminated, for the PCWSTR args.
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let value_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();

    // SAFETY: standard advapi32 registry FFI. `subkey_w` is a valid NUL-terminated
    // UTF-16 buffer kept alive across the call; `hkey` is written by
    // RegCreateKeyExW and closed unconditionally below. `value_w` (incl. its NUL)
    // is written as REG_SZ with an explicit byte length.
    unsafe {
        let mut hkey = HKEY::default();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey_w.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if status != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(status.0 as i32));
        }

        // REG_SZ byte length includes the trailing NUL (2 bytes per u16).
        let bytes = std::slice::from_raw_parts(
            value_w.as_ptr() as *const u8,
            std::mem::size_of_val(value_w.as_slice()),
        );
        let set = RegSetValueExW(hkey, PCWSTR::null(), None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(hkey);
        if set != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(set.0 as i32));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn firefox_manifest_has_allowed_extensions_and_stdio() {
        let exe = PathBuf::from(if cfg!(windows) {
            r"C:\Program Files\AI Job Hunter\app.exe"
        } else {
            "/opt/aijobhunter/app"
        });
        let bytes = manifest_json(&exe, true);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["name"], NATIVE_HOST_NAME);
        assert_eq!(v["type"], "stdio");
        assert_eq!(v["allowed_extensions"][0], FIREFOX_GECKO_ID);
        assert!(v.get("allowed_origins").is_none());
        // The exe path round-trips (serde_json escaped any backslashes).
        assert_eq!(v["path"], exe.to_string_lossy().as_ref());
    }

    #[test]
    fn chrome_manifest_has_allowed_origins_with_trailing_slash() {
        let exe = PathBuf::from("/opt/aijobhunter/app");
        let bytes = manifest_json(&exe, false);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["type"], "stdio");
        assert_eq!(v["allowed_origins"][0], CHROME_ALLOWED_ORIGIN);
        assert!(v["allowed_origins"][0].as_str().unwrap().ends_with('/'));
        assert!(v.get("allowed_extensions").is_none());
    }

    // ── write_manifest_if_app_dir_exists ─────────────────────────────────────

    #[test]
    fn write_manifest_if_app_dir_exists_skips_when_guard_absent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let guard = tmp.path().join("nonexistent-app");
        let target = guard.join("config/NativeMessagingHosts/host.json");
        // Must NOT create any file or directory when the guard dir is absent.
        write_manifest_if_app_dir_exists(&guard, &target, b"{}");
        assert!(
            !target.exists(),
            "should not create file when guard is absent"
        );
        assert!(!guard.exists(), "should not create guard dir");
    }

    #[test]
    fn write_manifest_if_app_dir_exists_writes_when_guard_present() {
        let tmp = tempfile::TempDir::new().unwrap();
        let guard = tmp.path().join("com.google.Chrome");
        std::fs::create_dir_all(&guard).unwrap();
        let target = guard
            .join("config/google-chrome/NativeMessagingHosts")
            .join("host.json");
        write_manifest_if_app_dir_exists(&guard, &target, b"{\"ok\":true}");
        assert!(
            target.exists(),
            "manifest should be written when guard exists"
        );
        let content = std::fs::read(&target).unwrap();
        assert_eq!(content, b"{\"ok\":true}");
    }

    // ── Linux flatpak path coverage ──────────────────────────────────────────

    /// Enumerate the expected Flatpak app-id → NativeMessagingHosts subpath
    /// mappings and confirm each one ends up written when the per-app guard dir
    /// is present, and NOT written when it is absent.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_flatpak_paths_written_only_for_installed_apps() {
        use std::path::Path;

        // The table must match what register.rs writes. If the table in
        // register.rs changes, this test catches the drift.
        struct FlatpakCase {
            app_id: &'static str,
            manifest_subdir: &'static str,
            firefox: bool,
        }
        let cases = [
            FlatpakCase {
                app_id: "com.google.Chrome",
                manifest_subdir: "config/google-chrome/NativeMessagingHosts",
                firefox: false,
            },
            FlatpakCase {
                app_id: "org.chromium.Chromium",
                manifest_subdir: "config/chromium/NativeMessagingHosts",
                firefox: false,
            },
            FlatpakCase {
                app_id: "com.brave.Browser",
                manifest_subdir: "config/BraveSoftware/Brave-Browser/NativeMessagingHosts",
                firefox: false,
            },
            FlatpakCase {
                app_id: "com.microsoft.Edge",
                manifest_subdir: "config/microsoft-edge/NativeMessagingHosts",
                firefox: false,
            },
            FlatpakCase {
                app_id: "org.mozilla.firefox",
                manifest_subdir: ".mozilla/native-messaging-hosts",
                firefox: true,
            },
        ];

        let exe = PathBuf::from("/opt/aijobhunter/app");
        let manifest_name = NATIVE_HOST_MANIFEST;

        for case in &cases {
            let tmp = tempfile::TempDir::new().unwrap();
            let flatpak_base = tmp.path().join(".var/app");
            let guard = flatpak_base.join(case.app_id);
            let target = guard.join(case.manifest_subdir).join(manifest_name);

            // 1. Guard absent → file must NOT be written.
            let bytes = if case.firefox {
                manifest_json(&exe, true)
            } else {
                manifest_json(&exe, false)
            };
            write_manifest_if_app_dir_exists(&guard, &target, &bytes);
            assert!(
                !target.exists(),
                "app_id={} must not write when guard absent",
                case.app_id
            );

            // 2. Guard present → file IS written with correct JSON.
            std::fs::create_dir_all(&guard).unwrap();
            write_manifest_if_app_dir_exists(&guard, &target, &bytes);
            assert!(
                target.exists(),
                "app_id={} must write when guard present",
                case.app_id
            );
            let v: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
            assert_eq!(v["name"], NATIVE_HOST_NAME);
            if case.firefox {
                assert!(
                    v.get("allowed_extensions").is_some(),
                    "firefox needs allowed_extensions"
                );
                assert!(v.get("allowed_origins").is_none());
            } else {
                assert!(
                    v.get("allowed_origins").is_some(),
                    "chrome needs allowed_origins"
                );
                assert!(v.get("allowed_extensions").is_none());
            }
        }
    }

    // ── Linux native paths coverage ──────────────────────────────────────────

    /// Confirm that Chromium, Brave, and Edge native config paths are also
    /// covered (not just google-chrome), by checking write_manifest writes
    /// to each destination without error.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_native_browser_paths_are_writable() {
        let exe = PathBuf::from("/opt/aijobhunter/app");
        let chrome_bytes = manifest_json(&exe, false);
        let firefox_bytes = manifest_json(&exe, true);

        let tmp = tempfile::TempDir::new().unwrap();
        let native_paths: &[(&[u8], &str)] = &[
            (&firefox_bytes, ".mozilla/native-messaging-hosts"),
            (&chrome_bytes, ".config/google-chrome/NativeMessagingHosts"),
            (&chrome_bytes, ".config/chromium/NativeMessagingHosts"),
            (
                &chrome_bytes,
                ".config/BraveSoftware/Brave-Browser/NativeMessagingHosts",
            ),
            (&chrome_bytes, ".config/microsoft-edge/NativeMessagingHosts"),
        ];
        for (bytes, rel) in native_paths {
            let path = tmp.path().join(rel).join(NATIVE_HOST_MANIFEST);
            write_manifest(&path, bytes);
            assert!(path.exists(), "native path not written: {rel}");
        }
    }
}
