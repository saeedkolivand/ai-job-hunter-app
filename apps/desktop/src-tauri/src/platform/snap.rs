//! Snap confinement detection.
//!
//! **Not a bare `SNAP` env-var check.** `snapd` sets `SNAP` (pointing at the
//! snap's read-only mount) for every process it launches — but env vars are
//! INHERITED by children, so a terminal opened inside a classic-confinement
//! snap (VS Code, PyCharm, …) carries `SNAP` into a completely unrelated
//! unpackaged install run from that terminal, permanently mis-reporting it
//! as packaged and silently disabling its updater + native-messaging
//! registration (same class of bug MSIX's [`super::msix`] already guards
//! against for package identity inherited by a child process — see its
//! `identity_inherited_by_a_child_outside_the_package_is_not_packaged`).
//!
//! The fix mirrors MSIX's own: identity (`SNAP` being set) is not enough —
//! the RUNNING EXE must actually live under that path. A child spawned from
//! inside a snap inherits the env var but keeps running from its own,
//! unrelated location, so requiring `current_exe()` to be inside `$SNAP`
//! tells the two apart.

use std::path::Path;
use std::sync::OnceLock;

/// Pure decision behind [`is_packaged`] — split out, same discipline as
/// `msix::decide`/`msix::same_root`, so the real "exe lives inside the snap
/// mount" logic is exercised by a truth table instead of a passthrough.
fn decide(exe: Option<&Path>, snap_dir: Option<&Path>) -> bool {
    match (exe, snap_dir) {
        (Some(exe), Some(dir)) if !dir.as_os_str().is_empty() => exe.starts_with(dir),
        _ => false,
    }
}

/// `true` when this process is running inside a Snap's confinement.
///
/// Cached: identity is fixed for the process lifetime, so `current_exe()`,
/// the `SNAP` lookup, and both `canonicalize` calls happen at most once.
pub fn is_packaged() -> bool {
    static PACKAGED: OnceLock<bool> = OnceLock::new();
    *PACKAGED.get_or_init(|| {
        // Canonicalized like `msix::is_packaged` canonicalizes its own two
        // sides — here specifically because `/snap` is a symlink on some
        // distros (into `/var/lib/snapd/snap`), so a raw `starts_with` could
        // false-negative on an exe path the OS reports through the symlink
        // while `$SNAP` names the resolved target, or vice versa. A failed
        // canonicalize drops to `None`, which `decide` already treats as
        // "not packaged" — the safe direction here (see this module's doc):
        // missing evidence must not silently disable an unrelated plain
        // install's updater/native-messaging registration.
        let snap_dir = std::env::var_os("SNAP")
            .map(std::path::PathBuf::from)
            .and_then(|p| std::fs::canonicalize(&p).ok());
        let exe = std::env::current_exe()
            .ok()
            .and_then(|p| std::fs::canonicalize(&p).ok());
        decide(exe.as_deref(), snap_dir.as_deref())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_inside_the_snap_mount_is_packaged() {
        assert!(decide(
            Some(Path::new("/snap/ai-job-hunter/x1/bin/ajh-tauri")),
            Some(Path::new("/snap/ai-job-hunter/x1"))
        ));
    }

    #[test]
    fn exe_outside_the_snap_mount_is_not_packaged() {
        assert!(!decide(
            Some(Path::new("/usr/bin/ajh-tauri")),
            Some(Path::new("/snap/ai-job-hunter/x1"))
        ));
    }

    /// The exact scenario the review flagged: a terminal launched INSIDE a
    /// classic-confinement snap (VS Code, PyCharm, …) exports `SNAP` to
    /// every child it spawns, including an unrelated unpackaged install of
    /// this app run from that terminal. `SNAP` alone would say "packaged"
    /// for it; only requiring the running exe to live under that path tells
    /// the two apart.
    #[test]
    fn an_env_var_inherited_from_an_unrelated_snap_does_not_make_an_outside_exe_packaged() {
        let inherited_snap_dir = Path::new("/snap/code/145");
        let unrelated_exe = Path::new("/home/user/.local/bin/ajh-tauri");
        assert!(!decide(Some(unrelated_exe), Some(inherited_snap_dir)));
    }

    #[test]
    fn missing_snap_dir_is_not_packaged() {
        assert!(!decide(Some(Path::new("/usr/bin/ajh-tauri")), None));
    }

    #[test]
    fn missing_exe_is_not_packaged() {
        assert!(!decide(None, Some(Path::new("/snap/ai-job-hunter/x1"))));
    }

    #[test]
    fn empty_snap_dir_is_not_packaged() {
        assert!(!decide(
            Some(Path::new("/usr/bin/ajh-tauri")),
            Some(Path::new(""))
        ));
    }

    /// Not running inside a snap in CI/dev, so this must answer `false` —
    /// the branch that keeps a plain install on the GitHub updater and
    /// native-messaging registration.
    #[test]
    fn unconfined_process_is_not_packaged() {
        assert!(std::env::var_os("SNAP").is_none());
        assert!(!is_packaged());
    }
}
