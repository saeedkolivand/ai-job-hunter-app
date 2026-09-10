//! Flatpak sandbox detection.
//!
//! **Not `FLATPAK_ID`.** That env var is how a sandboxed process learns its
//! own app ID, but like any env var it is INHERITED by every child process —
//! so a terminal opened from inside a Flatpak'd IDE, or any shell launched
//! by one, would carry `FLATPAK_ID` into a completely unrelated unpackaged
//! `.deb`/AppImage install run from it, permanently mis-reporting that
//! install as packaged (same class of bug MSIX's [`super::msix`] already
//! guards against for package identity inherited by a child process — see
//! its `identity_inherited_by_a_child_outside_the_package_is_not_packaged`).
//!
//! `/.flatpak-info` is the reliable signal instead: it is a file bind-mounted
//! into the sandbox's own mount namespace by `bubblewrap`, so it exists only
//! for processes actually running inside the sandbox right now — a child's
//! own mount namespace does not carry it along just because an ancestor's
//! environment does.

use std::path::Path;
use std::sync::OnceLock;

/// Pure decision behind [`is_packaged`] — split out so the real check (does
/// this marker file exist) is exercised against real temp files rather than
/// asserting `true == true` against a value the test invented itself.
fn decide(marker: &Path) -> bool {
    marker.exists()
}

/// The file `bubblewrap` bind-mounts into every Flatpak sandbox. Not a
/// constant callers can override — real Flatpak processes never look
/// anywhere else for this signal either.
const MARKER_PATH: &str = "/.flatpak-info";

/// `true` when this process is running inside a Flatpak sandbox.
///
/// Cached: the answer can't change mid-process, so the one filesystem probe
/// happens at most once.
pub fn is_packaged() -> bool {
    static PACKAGED: OnceLock<bool> = OnceLock::new();
    *PACKAGED.get_or_init(|| decide(Path::new(MARKER_PATH)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_true_when_the_marker_file_exists() {
        let marker = tempfile::NamedTempFile::new().unwrap();
        assert!(decide(marker.path()));
    }

    #[test]
    fn decide_false_when_the_marker_file_is_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(!decide(&dir.path().join("does-not-exist")));
    }

    /// Not running inside Flatpak in CI/dev, so the real marker is absent
    /// and this must answer `false` — the branch that keeps a plain install
    /// on the GitHub updater and native-messaging registration.
    #[test]
    fn unsandboxed_process_is_not_packaged() {
        assert!(!Path::new(MARKER_PATH).exists());
        assert!(!is_packaged());
    }
}
