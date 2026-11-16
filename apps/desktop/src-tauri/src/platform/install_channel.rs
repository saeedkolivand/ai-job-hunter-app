//! Which distribution channel this build was installed from, as a coarse label for
//! crash triage (`install.channel` Sentry tag). It reuses the existing packaging
//! detectors ([`super::packaged_flavour`], [`super::config::launched_appimage`]) and adds
//! only the Flatpak probe, which had none. The label names a channel, never a path.

use std::ffi::OsString;
use std::path::Path;

use super::PackageFlavour;

/// Flatpak's sandbox marker file (present in every Flatpak, next to `FLATPAK_ID`).
const FLATPAK_INFO: &str = "/.flatpak-info";

/// The label for the probes' answers, with every probe injected so tests can fake
/// them without touching the process environment or filesystem. Windows and macOS are
/// decided by target; Linux falls through snap, flatpak, appimage to `deb-or-rpm`.
/// `appimage` is only asked when the earlier Linux answers did not decide.
fn detect_from(
    flavour: Option<PackageFlavour>,
    env: impl Fn(&str) -> Option<OsString>,
    exists: impl Fn(&Path) -> bool,
    appimage: impl FnOnce() -> bool,
    windows: bool,
    macos: bool,
) -> &'static str {
    if windows {
        return match flavour {
            Some(PackageFlavour::MsStore) => "msstore",
            _ => "windows-installer",
        };
    }
    if macos {
        return "macos";
    }
    if flavour == Some(PackageFlavour::Snap) {
        "snap"
    } else if env("FLATPAK_ID").is_some() || exists(Path::new(FLATPAK_INFO)) {
        "flatpak"
    } else if appimage() {
        "appimage"
    } else {
        "deb-or-rpm"
    }
}

/// The channel of the running process.
pub fn detect() -> &'static str {
    detect_from(
        super::packaged_flavour(),
        |key| std::env::var_os(key),
        |p| p.exists(),
        || {
            let exe = std::env::current_exe().ok();
            super::config::launched_appimage(exe.as_deref()).is_some()
        },
        cfg!(windows),
        cfg!(target_os = "macos"),
    )
}

#[cfg(test)]
mod tests;
