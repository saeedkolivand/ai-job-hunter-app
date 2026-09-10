pub mod accent_watcher;
pub mod chrome;
pub mod config;
pub mod linux_appimage;
pub mod msix;
pub mod process;
pub mod snap;
pub mod windows_console;

pub use chrome::{
    detect_chromium_user_data_roots, detect_system_chrome, BrowserLaunch, ChromiumBrowser,
};
pub use process::{cli_path, NoWindow};
#[cfg(windows)]
pub use process::{resolve_cli_binary, ResolvedCli};

/// Which store/sandbox this build is packaged as. A caller that only needs
/// "is it packaged at all" wants [`is_packaged_build`]; one whose reply
/// reaches the user (updater status/refusal copy — see `crate::updater`)
/// needs to know WHICH one, since "Updates come from the Microsoft Store" is
/// simply false for a Snap install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageFlavour {
    /// Microsoft Store (MSIX) — see [`msix`].
    MsStore,
    /// Snap confinement — see [`snap`].
    Snap,
}

impl PackageFlavour {
    /// The wire value for `managedBy`/`by` in the updater's IPC shapes — see
    /// `UpdateCheckResult`/`UpdateStatus` in
    /// `packages/shared/src/ipc/contracts/updater.ts`.
    pub fn as_wire_str(self) -> &'static str {
        match self {
            PackageFlavour::MsStore => "msstore",
            PackageFlavour::Snap => "snap",
        }
    }
}

/// Which packaged flavour (if any) this process is running as. `None` on a
/// plain NSIS/MSI/AppImage/.deb install. There is no host where more than
/// one of these probes could plausibly answer `true` at once, so the
/// first-match order below is never a real tie-break.
pub fn packaged_flavour() -> Option<PackageFlavour> {
    if msix::is_packaged() {
        Some(PackageFlavour::MsStore)
    } else if snap::is_packaged() {
        Some(PackageFlavour::Snap)
    } else {
        None
    }
}

/// `true` when this process is running inside ANY store/sandbox package
/// (MSIX or Snap) — the umbrella "the app is not free to manage its own
/// updates/native-messaging registration" question. Individual callers that
/// need to know WHICH one (e.g. `msix::published_exe_path`, or anything the
/// user reads — see [`packaged_flavour`]) go through the specific module or
/// [`packaged_flavour`] instead.
pub fn is_packaged_build() -> bool {
    packaged_flavour().is_some()
}
