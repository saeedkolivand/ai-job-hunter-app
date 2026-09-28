//! Non-Windows placement for the native-messaging host manifests — split from `register.rs`
//! (R8 relief): macOS + Linux both read fixed well-known directories directly (no registry),
//! plus the Linux Flatpak per-app config dirs, which are written only when Flatpak has already
//! created the app's own confined config root (see `manifest::write_manifest_if_app_dir_exists`).

#![cfg(not(windows))]

use std::path::Path;

use super::super::NATIVE_HOST_MANIFEST;
use super::manifest::write_manifest;
// Linux-only (Flatpak per-app guard) — matches the item's own gate at its
// only call site below, so this import doesn't exist (and doesn't need to)
// on macOS.
#[cfg(target_os = "linux")]
use super::manifest::write_manifest_if_app_dir_exists;

/// Write both host manifests to every well-known browser config dir this OS supports.
pub(super) fn register_unix(data_dir: &Path, firefox_json: &[u8], chrome_json: &[u8]) {
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
        write_manifest("firefox", &firefox_path, firefox_json);
        write_manifest("chrome", &chrome_path, chrome_json);
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
        let vivaldi_path = home
            .join(".config/vivaldi/NativeMessagingHosts")
            .join(NATIVE_HOST_MANIFEST);
        write_manifest("firefox", &firefox_path, firefox_json);
        write_manifest("chrome", &chrome_path, chrome_json);
        write_manifest("chromium", &chromium_path, chrome_json);
        write_manifest("brave", &brave_path, chrome_json);
        write_manifest("edge", &edge_path, chrome_json);
        write_manifest("vivaldi", &vivaldi_path, chrome_json);

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
            // Vivaldi is Chromium-family → Chrome-style manifest (allowed_origins).
            FlatpakEntry {
                app_id: "com.vivaldi.Vivaldi",
                manifest_rel: "config/vivaldi/NativeMessagingHosts",
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
            write_manifest_if_app_dir_exists(entry.app_id, &guard, &path, bytes);
        }
    }
}

#[cfg(test)]
mod tests;
