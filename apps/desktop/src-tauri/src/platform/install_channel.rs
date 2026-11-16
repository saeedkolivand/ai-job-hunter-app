//! Which distribution channel this build was installed from, as a coarse label for
//! crash triage (`install.channel` Sentry tag). It reuses the existing packaging
//! detectors ([`super::packaged_flavour`], [`super::config::launched_appimage`]) and adds
//! only the Flatpak probe, which had none. The label names a channel, never a path.

use std::path::Path;

use super::PackageFlavour;

/// Flatpak's sandbox marker file (present in every Flatpak, next to `FLATPAK_ID`).
const FLATPAK_INFO: &str = "/.flatpak-info";

/// Pure mapping from the probe answers to the label. Windows and macOS are decided by
/// target; Linux falls through snap, flatpak, appimage to `deb-or-rpm`.
fn decide(
    flavour: Option<PackageFlavour>,
    flatpak: bool,
    appimage: bool,
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
    match flavour {
        Some(PackageFlavour::Snap) => "snap",
        _ if flatpak => "flatpak",
        _ if appimage => "appimage",
        _ => "deb-or-rpm",
    }
}

/// The channel of the running process.
pub fn detect() -> &'static str {
    let flatpak = std::env::var_os("FLATPAK_ID").is_some() || Path::new(FLATPAK_INFO).exists();
    let exe = std::env::current_exe().ok();
    let appimage = super::config::launched_appimage(exe.as_deref()).is_some();
    decide(
        super::packaged_flavour(),
        flatpak,
        appimage,
        cfg!(windows),
        cfg!(target_os = "macos"),
    )
}

#[cfg(test)]
mod tests;
