//! Every test below is Linux-only (Flatpak per-app guard dirs + the native
//! browser config paths registered only in `register_unix`'s Linux arm), so
//! the whole module is gated rather than each import: `super::*` (the
//! Linux-gated `write_manifest_if_app_dir_exists`) and `PathBuf`/`manifest_json`
//! would otherwise sit unused on macOS.
#![cfg(target_os = "linux")]

use super::*;
use std::path::PathBuf;

use super::super::super::NATIVE_HOST_NAME;
use super::super::manifest::manifest_json;

#[test]
fn linux_flatpak_paths_written_only_for_installed_apps() {
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
            app_id: "com.vivaldi.Vivaldi",
            manifest_subdir: "config/vivaldi/NativeMessagingHosts",
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
        write_manifest_if_app_dir_exists(case.app_id, &guard, &target, &bytes);
        assert!(
            !target.exists(),
            "app_id={} must not write when guard absent",
            case.app_id
        );

        // 2. Guard present → file IS written with correct JSON.
        std::fs::create_dir_all(&guard).unwrap();
        write_manifest_if_app_dir_exists(case.app_id, &guard, &target, &bytes);
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
        (&chrome_bytes, ".config/vivaldi/NativeMessagingHosts"),
    ];
    for (bytes, rel) in native_paths {
        let path = tmp.path().join(rel).join(NATIVE_HOST_MANIFEST);
        write_manifest(rel, &path, bytes);
        assert!(path.exists(), "native path not written: {rel}");
    }
}
