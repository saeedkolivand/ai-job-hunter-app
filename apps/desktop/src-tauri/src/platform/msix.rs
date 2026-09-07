//! Does this process run with MSIX **package identity** — i.e. is it the
//! Microsoft Store flavour of the app rather than the NSIS/MSI one?
//!
//! Both flavours are the SAME `ajh-tauri.exe`; the Store one is that exe
//! wrapped in an MSIX (`apps/desktop/src-tauri/windows/msix/AppxManifest.xml`,
//! packed by `apps/desktop/scripts/pack-msix.mjs`). Detection is therefore a
//! runtime question, not a build-time feature: one binary, two containers.
//!
//! The one behaviour that MUST differ is updating. The GitHub updater
//! (`crate::updater`) downloads and runs the **NSIS installer**, so letting a
//! Store install auto-update would install a SECOND, unmanaged copy of the app
//! beside the packaged one — two exes, two Start entries, one shared data dir,
//! and a Store package that can never be updated by the Store again. Store
//! packages are updated by the Store; every updater entry point asks here
//! first.
//!
//! Everything else stays identical on purpose: the manifest disables registry
//! and file-system write virtualization, so the HKCU native-messaging
//! registration (`extension_bridge`), the launch-at-login Run key
//! (`auto-launch`), and the `platform::config::data_dir()` app data directory
//! are the same real locations a non-Store install uses.

use std::sync::OnceLock;

/// `true` when the process has MSIX package identity (Store build).
///
/// Cached: identity is fixed for the lifetime of a process — a packaged
/// process can never become unpackaged or vice versa — so the WinRT call
/// happens at most once.
pub fn is_packaged() -> bool {
    static PACKAGED: OnceLock<bool> = OnceLock::new();
    *PACKAGED.get_or_init(detect)
}

/// `Package::Current()` is the documented identity probe: it resolves for a
/// process running inside a package and fails with `APPMODEL_ERROR_NO_PACKAGE`
/// for one that is not (see Microsoft's "Package a Tauri app for the Microsoft
/// Store" guide). Any other failure is treated the same way — unpackaged — so
/// an unexpected WinRT error can only ever leave the normal GitHub updater
/// enabled, never disable updates for a user who has no Store to update from.
#[cfg(windows)]
fn detect() -> bool {
    windows::ApplicationModel::Package::Current().is_ok()
}

/// No MSIX outside Windows; the Store flavour does not exist there.
#[cfg(not(windows))]
fn detect() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::is_packaged;

    /// The test binary is a plain unpackaged exe, so the probe must say so —
    /// this is the branch that keeps NSIS/MSI users on the GitHub updater. A
    /// detector that returned `true` here (e.g. treating a WinRT error as
    /// "packaged") would silently switch every non-Store install to
    /// "updates come from the Store" and strand it on its installed version.
    #[test]
    fn unpackaged_process_is_not_packaged() {
        assert!(!is_packaged());
    }

    /// Cached, so repeated calls agree (and cost one WinRT call at most).
    #[test]
    fn repeated_calls_agree() {
        assert_eq!(is_packaged(), is_packaged());
    }
}
