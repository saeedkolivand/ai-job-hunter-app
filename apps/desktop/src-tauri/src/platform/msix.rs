//! Microsoft Store (MSIX) packaging: is this process packaged, and where does a
//! packaged process publish paths that other programs have to keep working?
//!
//! Both Windows flavours are the SAME `ajh-tauri.exe`; the Store one is that exe
//! wrapped in an MSIX (`apps/desktop/src-tauri/windows/msix/AppxManifest.xml`,
//! packed by `apps/desktop/scripts/pack-msix.mjs`). Detection is therefore a
//! runtime question, not a build-time feature: one binary, two containers.
//!
//! Three things must differ once packaged, and all three route through here:
//!
//! 1. **Updating.** The GitHub updater (`crate::updater`) downloads and runs the
//!    NSIS installer, so a Store install that auto-updated would end up with a
//!    SECOND, unmanaged copy of the app beside the packaged one. Store packages
//!    are updated by the Store.
//! 2. **The published exe path.** `current_exe()` inside a package is
//!    `…\WindowsApps\<PackageFullName>\ajh-tauri.exe` — a directory normal users
//!    cannot execute from, whose name embeds the package VERSION, so anything
//!    that records it (the browser native-messaging host, the agent-CLI pointer)
//!    dangles after the next Store update. [`alias_exe_path`] returns the stable
//!    execution-alias shim instead.
//! 3. **Launch at login.** A packaged app registers a `StartupTask` declared in
//!    the manifest, not a `Run` key pointing at that same version-pinned path.
//!
//! Everything else stays identical on purpose: the manifest disables registry
//! and file-system write virtualization, so the HKCU native-messaging
//! registration and the `platform::config::data_dir()` app data directory are
//! the same real locations a non-Store install uses.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::{AppError, AppResult};

#[cfg(windows)]
use windows::{
    core::PWSTR,
    Win32::Foundation::{
        APPMODEL_ERROR_NO_PACKAGE, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, WIN32_ERROR,
    },
    Win32::Storage::Packaging::Appx::{
        GetCurrentPackageFamilyName, GetCurrentPackageFullName, GetCurrentPackagePath,
    },
};

/// `TaskId` of the `desktop:StartupTask` declared in the manifest. The two are
/// one contract: renaming it there without renaming it here silently turns
/// launch-at-login into a no-op on the Store build.
const STARTUP_TASK_ID: &str = "AjhLaunchAtLogin";

/// The alias the manifest's `windows.appExecutionAlias` extension registers.
/// Same name as the exe, so every doc and copy-paste command works on both
/// flavours (`docs/knowledge/agent-cli.md`).
const ALIAS_EXE_NAME: &str = "ajh-tauri.exe";

// ── Package identity ─────────────────────────────────────────────────────────

/// What the OS said when asked for this process's package identity.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Identity {
    /// `APPMODEL_ERROR_NO_PACKAGE` — a plain NSIS/MSI install (or any non-Windows host).
    Absent,
    /// Identity present, with the package's install directory when the OS gave one.
    Present(Option<PathBuf>),
    /// The probe failed in a way we do not understand.
    Unknown,
}

/// The whole decision, kept `#[cfg]`-free so every branch is unit-tested on
/// every host rather than on a Windows-only CI leg (same discipline as
/// `platform::config::launched_appimage`).
///
/// Two asymmetries drive it:
///
/// - **`Unknown` counts as packaged.** The dangerous direction is a packaged
///   process concluding it is unpackaged: that re-enables the GitHub updater
///   and installs a second copy of the app. The opposite mistake merely leaves
///   an unpackaged install without in-app updates, which is visible, reversible
///   and cannot corrupt anything.
/// - **Identity alone is not enough.** Windows propagates package identity to
///   CHILD processes of a full-trust packaged app, so an NSIS-installed
///   `ajh-tauri.exe` spawned by the packaged one would inherit it and wrongly
///   report "Store build". Requiring the running exe to live inside the package
///   install root is what tells the two apart.
fn decide(identity: &Identity, exe: Option<&Path>, alias_dir: Option<&Path>) -> bool {
    match identity {
        Identity::Absent => false,
        Identity::Unknown => true,
        // Identity, but the OS would not name its install root (or the path it
        // named could not be canonicalized) — nothing to compare against, so
        // fall back to the safe direction.
        Identity::Present(None) => true,
        Identity::Present(Some(root)) => match exe {
            // The alias directory counts as "inside the package": a process
            // launched through the execution-alias shim belongs to this
            // package as much as one launched from the install root.
            Some(exe) => same_root(exe, root) || alias_dir.is_some_and(|dir| same_root(exe, dir)),
            None => true,
        },
    }
}

/// Is `exe` inside `root`? Both are expected to be canonical already (see
/// [`is_packaged`]); this half is pure so every shape below is unit-tested on
/// every host.
///
/// **Not `Path::starts_with`.** On Windows that folds case only in the drive
/// prefix and compares every other component byte-for-byte, so
/// `C:\PROGRAM FILES\WINDOWSAPPS\…` inside `C:\Program Files\WindowsApps\…`
/// answers `false` — and a false "not packaged" is the dangerous direction:
/// it re-arms the GitHub updater on a Store install.
///
/// Segment-wise rather than `Path::components()`, and case-folded:
/// `components()` on a non-Windows host does not treat `\` as a separator or
/// recognise the `\\?\` prefix, so the tests would compare one opaque blob
/// instead of segments and this decision would go untested on the CI legs that
/// run it. Segment-wise is also what makes a sibling directory that merely
/// shares a prefix string (`…\WindowsApps2\…`) a non-match.
fn same_root(exe: &Path, root: &Path) -> bool {
    let root = segments(root);
    let exe = segments(exe);
    // An empty root would otherwise match everything. Unreachable —
    // `probe_identity` discards a package path with no segments — but the
    // consequence of being wrong here is "every process is packaged".
    !root.is_empty() && exe.len() > root.len() && exe[..root.len()] == root[..]
}

/// Path segments for comparison: the `\\?\` verbatim prefix removed, both
/// separators honoured, empty segments (trailing/doubled separators) dropped,
/// each segment lowercased.
fn segments(path: &Path) -> Vec<String> {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .split(['\\', '/'])
        .filter(|segment| !segment.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// `true` when this process runs with MSIX package identity (Store build).
///
/// Cached: identity is fixed for the lifetime of a process, so the OS probe
/// happens at most once — which also means the `Unknown` warning below is
/// logged at most once, and the handful of `canonicalize` calls happen once.
///
/// Both paths go through [`std::fs::canonicalize`] before they are compared:
/// it resolves an 8.3 short name (`C:\PROGRA~1\…`) to its long form and puts
/// both sides in the same `\\?\` shape, neither of which a string comparison
/// can do. A canonicalize FAILURE is missing evidence, and missing evidence
/// means packaged — the same safe direction as [`Identity::Unknown`] — which
/// is exactly what dropping it to `None` achieves here: every `None` arm of
/// [`decide`] answers `true`.
pub fn is_packaged() -> bool {
    static PACKAGED: OnceLock<bool> = OnceLock::new();
    *PACKAGED.get_or_init(|| {
        let identity = match probe_identity() {
            Identity::Present(Some(root)) => Identity::Present(canonical(&root)),
            other => other,
        };
        let exe = std::env::current_exe().ok().and_then(|p| canonical(&p));
        let alias = alias_dir().and_then(|p| canonical(&p));
        decide(&identity, exe.as_deref(), alias.as_deref())
    })
}

/// `fs::canonicalize`, failure flattened to `None` (see [`is_packaged`] for
/// why that is the safe direction).
fn canonical(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

/// `GetCurrentPackageFullName` is the plain Win32 identity probe: it needs no
/// COM/WinRT apartment, cannot be influenced by anything in the process, and
/// returns a documented sentinel (`APPMODEL_ERROR_NO_PACKAGE`) for "not
/// packaged" — distinguishable from a genuine failure, which the WinRT
/// `Package::Current()` route is not.
#[cfg(windows)]
fn probe_identity() -> Identity {
    let mut len: u32 = 0;
    // SAFETY: `len` is a valid, initialized out-parameter and `None` is the
    // documented way to ask for the required buffer size; the call writes
    // nothing else and returns a plain error code.
    let rc = unsafe { GetCurrentPackageFullName(&mut len, None) };
    if rc == APPMODEL_ERROR_NO_PACKAGE {
        Identity::Absent
    } else if rc == ERROR_SUCCESS || rc == ERROR_INSUFFICIENT_BUFFER {
        // A path with no segments (empty, or nothing but separators) is
        // discarded rather than passed on: `same_root` would answer `false` for
        // it, which is the unsafe direction, whereas `Present(None)` is the
        // safe one.
        Identity::Present(
            current_package_string(GetCurrentPackagePath)
                .map(PathBuf::from)
                .filter(|path| !segments(path).is_empty()),
        )
    } else {
        // Neither "packaged" nor the documented "no package" — treated as
        // packaged (see `decide`). Logged so the case is diagnosable rather
        // than silent; the code alone, never a path (arch rule R15).
        log::warn!(
            "[msix] unexpected package-identity probe result {} — assuming a packaged build",
            rc.0
        );
        Identity::Unknown
    }
}

/// No MSIX outside Windows; the Store flavour does not exist there.
#[cfg(not(windows))]
fn probe_identity() -> Identity {
    Identity::Absent
}

/// Windows' two-call buffer dance, shared by the `GetCurrentPackage*` queries:
/// ask for the size, allocate, ask again. `None` on any failure — every caller
/// already has an answer for "the OS would not tell us".
#[cfg(windows)]
fn current_package_string(
    query: unsafe fn(*mut u32, Option<PWSTR>) -> WIN32_ERROR,
) -> Option<String> {
    let mut len: u32 = 0;
    // SAFETY: as in `probe_identity` — sizing call, no buffer written. The
    // return code is deliberately ignored: it is always
    // `ERROR_INSUFFICIENT_BUFFER` here, and `len == 0` below is the only
    // outcome worth branching on.
    let _ = unsafe { query(&mut len, None) };
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    // SAFETY: `buf` holds exactly the `len` UTF-16 code units the sizing call
    // asked for and outlives the call; `len` is re-read as an in/out parameter.
    let rc = unsafe { query(&mut len, Some(PWSTR(buf.as_mut_ptr()))) };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

// ── The path a packaged build publishes for itself ───────────────────────────

/// `%LOCALAPPDATA%\Microsoft\WindowsApps\<PackageFamilyName>` — where Windows
/// puts this package's execution-alias shims. Pure, so the layout is tested on
/// every host.
fn alias_directory(local_app_data: &Path, family: &str) -> PathBuf {
    local_app_data
        .join("Microsoft")
        .join("WindowsApps")
        .join(family)
}

/// The alias directory for THIS process's package, or `None` when there is no
/// package identity (or no `%LOCALAPPDATA%`).
///
/// Deliberately does not consult [`is_packaged`]: it is one of that decision's
/// own inputs, so asking would recurse. It only needs identity, which
/// [`decide`] has already established by the time this matters.
fn alias_dir() -> Option<PathBuf> {
    let family = package_family_name()?;
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(alias_directory(Path::new(&local), &family))
}

/// The path this exe should PUBLISH to other programs on a Store install, or
/// `None` on any other build (the caller keeps using `current_exe()`).
///
/// The family name — unlike the full name — carries no version, so the shim
/// path survives every Store update; the real exe path does not.
pub fn alias_exe_path() -> Option<PathBuf> {
    if !is_packaged() {
        return None;
    }
    let path = alias_dir()?.join(ALIAS_EXE_NAME);
    if !path.is_file() {
        // Unverified end-to-end (needs a registered package — see
        // docs/DEPLOYMENT.md). If the assumption is ever wrong, this line in a
        // diagnostics bundle is what says so. No path logged (R15).
        log::warn!(
            "[msix] execution-alias shim not found where expected — the agent-CLI pointer and native-messaging host may not resolve"
        );
    }
    Some(path)
}

#[cfg(windows)]
fn package_family_name() -> Option<String> {
    current_package_string(GetCurrentPackageFamilyName)
}

#[cfg(not(windows))]
fn package_family_name() -> Option<String> {
    None
}

// ── Launch at login (packaged builds only) ───────────────────────────────────

/// Mirror of WinRT's `StartupTaskState`, so the mapping below is a pure
/// function of an `i32` and testable on every host. A `#[cfg(windows)]` test
/// pins these against the real enum, so they cannot drift silently.
mod startup_state {
    pub(super) const DISABLED: i32 = 0;
    pub(super) const DISABLED_BY_USER: i32 = 1;
    pub(super) const ENABLED: i32 = 2;
    pub(super) const DISABLED_BY_POLICY: i32 = 3;
    pub(super) const ENABLED_BY_POLICY: i32 = 4;
}

/// Does this state mean the app actually starts at login?
///
/// `EnabledByPolicy` counts: an administrator turned it on, the task DOES run,
/// and reporting `false` would leave the settings toggle contradicting the
/// machine's own behaviour.
fn startup_state_is_enabled(state: i32) -> bool {
    state == startup_state::ENABLED || state == startup_state::ENABLED_BY_POLICY
}

/// Outcome of asking Windows to enable the task. Unlike the `Run` key, this can
/// be REFUSED — by the user in Settings ▸ Apps ▸ Startup, or by policy — and a
/// refusal must reach the UI as an error, not as a silent "off".
fn enable_outcome(state: i32) -> AppResult<bool> {
    match state {
        startup_state::DISABLED_BY_USER => Err(AppError::Message(
            "Windows has startup for this app turned off. Enable “AI Job Hunter” in Settings ▸ Apps ▸ Startup, then try again.".into(),
        )),
        startup_state::DISABLED_BY_POLICY => Err(AppError::Message(
            "Launch at login is blocked by a system policy on this device.".into(),
        )),
        // Spelled out rather than folded into the catch-all: it is the one
        // remaining "we asked, Windows said no, but not for a reason we can
        // explain" case, and naming it keeps the whole `StartupTaskState`
        // domain visible in one place.
        startup_state::DISABLED => Ok(false),
        other => Ok(startup_state_is_enabled(other)),
    }
}

/// Whether the packaged app's startup task is enabled, or `None` when this is
/// not a packaged build — the caller then falls back to the autostart plugin.
pub fn startup_task_enabled() -> Option<bool> {
    if !is_packaged() {
        return None;
    }
    Some(
        startup_task_state()
            .map(startup_state_is_enabled)
            .unwrap_or(false),
    )
}

/// Enable/disable the packaged app's startup task, or `None` when this is not a
/// packaged build. Returns the state the OS actually applied.
pub fn set_startup_task(enabled: bool) -> Option<AppResult<bool>> {
    if !is_packaged() {
        return None;
    }
    Some(apply_startup_task(enabled))
}

/// `join()` is `windows-future`'s blocking accessor (it was `get()` before
/// 0.3): it waits on the operation with no timeout and no message pump.
///
/// That is why both callers are `#[tauri::command(async)]` — a plain
/// `#[tauri::command]` runs its body inline on the UI thread (see
/// `commands/resume.rs` for the traced call path), so an unbounded wait there
/// freezes the window. `(async)` moves the same synchronous body onto a Tokio
/// worker, where blocking is merely slow.
#[cfg(windows)]
fn startup_task() -> windows::core::Result<windows::ApplicationModel::StartupTask> {
    windows::ApplicationModel::StartupTask::GetAsync(&windows::core::HSTRING::from(
        STARTUP_TASK_ID,
    ))?
    .join()
}

/// Current state as a raw `i32`, or `None` if the task cannot be reached.
#[cfg(windows)]
fn startup_task_state() -> Option<i32> {
    startup_task()
        .and_then(|task| task.State())
        .ok()
        .map(|s| s.0)
}

#[cfg(not(windows))]
fn startup_task_state() -> Option<i32> {
    None
}

#[cfg(windows)]
fn apply_startup_task(enabled: bool) -> AppResult<bool> {
    let task = startup_task().map_err(|e| AppError::Message(e.message()))?;
    if enabled {
        let state = task
            .RequestEnableAsync()
            .and_then(|op| op.join())
            .map_err(|e| AppError::Message(e.message()))?;
        enable_outcome(state.0)
    } else {
        task.Disable().map_err(|e| AppError::Message(e.message()))?;
        Ok(false)
    }
}

#[cfg(not(windows))]
fn apply_startup_task(_enabled: bool) -> AppResult<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(p: impl AsRef<str>) -> PathBuf {
        PathBuf::from(p.as_ref())
    }

    /// The package install root in its canonical (`\\?\`, long-name) shape,
    /// which is what `is_packaged` actually compares.
    const INSTALL: &str = r"\\?\C:\Program Files\WindowsApps\Publisher.App_1.2.3.0_x64__abc";

    /// Every (identity × exe-known) combination `decide` can be handed, so no
    /// arm is left to be argued about in review. The `None` exe column is the
    /// "we could not canonicalize / `current_exe()` failed" case, and every
    /// cell in it must answer the same way its identity does with no exe to
    /// check — packaged, unless identity is positively absent.
    #[test]
    fn the_whole_truth_table_is_pinned() {
        let exe = root(r"\\?\C:\Users\tester\AppData\Local\AI Job Hunter\ajh-tauri.exe");
        let inside = root(INSTALL).join("ajh-tauri.exe");
        let present = Identity::Present(Some(root(INSTALL)));

        assert!(!decide(&Identity::Absent, Some(&exe), None));
        assert!(!decide(&Identity::Absent, None, None));
        assert!(decide(&Identity::Unknown, Some(&exe), None));
        assert!(decide(&Identity::Unknown, None, None));
        assert!(decide(&Identity::Present(None), Some(&exe), None));
        assert!(decide(&Identity::Present(None), None, None));
        assert!(decide(&present, Some(&inside), None));
        assert!(!decide(&present, Some(&exe), None));
        assert!(decide(&present, None, None));
    }

    #[test]
    fn identity_plus_exe_inside_the_package_is_packaged() {
        let install = root(INSTALL);
        let exe = install.join("ajh-tauri.exe");
        assert!(decide(&Identity::Present(Some(install)), Some(&exe), None));
    }

    /// The inherited-identity case: an NSIS-installed exe spawned by the
    /// packaged app inherits package identity but runs from its own directory,
    /// and must keep the normal updater.
    #[test]
    fn identity_inherited_by_a_child_outside_the_package_is_not_packaged() {
        let exe = root(r"\\?\C:\Users\tester\AppData\Local\AI Job Hunter\ajh-tauri.exe");
        assert!(!decide(
            &Identity::Present(Some(root(INSTALL))),
            Some(&exe),
            None
        ));
    }

    /// A process launched through the execution-alias shim belongs to the
    /// package too. `current_exe()` is expected to resolve the shim (an
    /// `APPEXECLINK` reparse point) back to the real WindowsApps path — and if
    /// it cannot, `canonicalize` fails and the exe arrives as `None`, which is
    /// already packaged. This arm makes the third possibility — the alias path
    /// surviving canonicalization as itself — land on the same answer.
    #[test]
    fn an_exe_under_the_alias_directory_is_packaged() {
        let alias = root(r"\\?\C:\Users\t\AppData\Local\Microsoft\WindowsApps\Publisher.App_abc");
        let exe = alias.join("ajh-tauri.exe");
        assert!(decide(
            &Identity::Present(Some(root(INSTALL))),
            Some(&exe),
            Some(&alias)
        ));
        // …but only for THIS package's alias directory.
        let other = root(r"\\?\C:\Users\t\AppData\Local\Microsoft\WindowsApps\Someone.Else_xyz");
        assert!(!decide(
            &Identity::Present(Some(root(INSTALL))),
            Some(&other.join("ajh-tauri.exe")),
            Some(&alias)
        ));
    }

    /// `Path::starts_with` answers `false` for every one of these on Windows
    /// (it folds case only in the drive prefix), and a false "not packaged"
    /// re-arms the GitHub updater on a Store install.
    #[test]
    fn root_matching_survives_case_separators_and_the_verbatim_prefix() {
        let long = r"C:\Program Files\WindowsApps\Publisher.App_1.2.3.0_x64__abc";
        let exe_upper =
            root(r"C:\PROGRAM FILES\WINDOWSAPPS\PUBLISHER.APP_1.2.3.0_X64__ABC\AJH-TAURI.EXE");

        // Mixed case, either side.
        assert!(same_root(&exe_upper, &root(long)));
        assert!(same_root(
            &root(format!(r"{long}\ajh-tauri.exe")),
            &root(long.to_uppercase())
        ));
        // Trailing separator on the root.
        assert!(same_root(
            &root(format!(r"{long}\ajh-tauri.exe")),
            &root(format!(r"{long}\"))
        ));
        // Verbatim prefix on one side only, in either direction.
        assert!(same_root(
            &root(format!(r"\\?\{long}\ajh-tauri.exe")),
            &root(long)
        ));
        assert!(same_root(
            &root(format!(r"{long}\ajh-tauri.exe")),
            &root(format!(r"\\?\{long}"))
        ));
        // Forward slashes (as `current_exe` never produces, but a hand-built
        // path might).
        assert!(same_root(
            &root(long.replace('\\', "/") + "/ajh-tauri.exe"),
            &root(long)
        ));
    }

    /// A sibling directory that merely shares a prefix STRING is not inside the
    /// package — the reason this compares segments rather than characters.
    #[test]
    fn a_sibling_directory_sharing_a_prefix_is_not_inside_the_package() {
        let root_dir = root(r"C:\Program Files\WindowsApps");
        assert!(!same_root(
            &root(r"C:\Program Files\WindowsApps2\Evil.App\ajh-tauri.exe"),
            &root_dir
        ));
        // The exe itself is never the root, and an empty root matches nothing.
        assert!(!same_root(&root_dir.clone(), &root_dir));
        assert!(!same_root(&root(r"C:\anything\ajh-tauri.exe"), &root("")));
    }

    /// 8.3 short names are what `fs::canonicalize` is for: `same_root` is pure
    /// and cannot resolve `PROGRA~1` (asserted here so the division of labour
    /// stays explicit), while the canonical pair it is actually given matches.
    #[test]
    fn short_names_are_resolved_before_comparison_not_by_it() {
        let long = r"C:\Program Files\WindowsApps\Publisher.App_1.2.3.0_x64__abc";
        let short = r"C:\PROGRA~1\WINDOW~1\PUBLIS~1";
        assert!(!same_root(
            &root(format!(r"{short}\ajh-tauri.exe")),
            &root(long)
        ));
        // Both sides as `fs::canonicalize` would return them.
        assert!(same_root(
            &root(format!(r"\\?\{long}\ajh-tauri.exe")),
            &root(format!(r"\\?\{long}"))
        ));
    }

    /// The test binary is a plain unpackaged exe, so the whole probe must say
    /// so — the branch that keeps NSIS/MSI users on the GitHub updater.
    #[test]
    fn unpackaged_process_is_not_packaged() {
        assert!(!is_packaged());
    }

    /// Cached, so repeated calls agree (and cost one probe at most).
    #[test]
    fn repeated_calls_agree() {
        assert_eq!(is_packaged(), is_packaged());
    }

    #[test]
    fn alias_directory_is_family_scoped() {
        assert_eq!(
            alias_directory(Path::new("C:/Users/t/AppData/Local"), "Publisher.App_abcdefg")
                .join(ALIAS_EXE_NAME),
            root("C:/Users/t/AppData/Local/Microsoft/WindowsApps/Publisher.App_abcdefg/ajh-tauri.exe")
        );
    }

    /// Not packaged here, so nothing may be published — the fallback to
    /// `current_exe()` in the callers depends on this being `None`.
    #[test]
    fn alias_exe_path_is_none_when_unpackaged() {
        assert!(alias_exe_path().is_none());
        assert!(startup_task_enabled().is_none());
        assert!(set_startup_task(true).is_none());
    }

    #[test]
    fn only_the_enabled_states_report_on() {
        assert!(startup_state_is_enabled(startup_state::ENABLED));
        assert!(startup_state_is_enabled(startup_state::ENABLED_BY_POLICY));
        assert!(!startup_state_is_enabled(startup_state::DISABLED));
        assert!(!startup_state_is_enabled(startup_state::DISABLED_BY_USER));
        assert!(!startup_state_is_enabled(startup_state::DISABLED_BY_POLICY));
    }

    /// A refusal must surface as an error the settings panel shows, not as a
    /// quiet `Ok(false)` that looks like the user's click did nothing.
    #[test]
    fn a_refused_enable_is_an_error_not_a_silent_off() {
        assert!(matches!(
            enable_outcome(startup_state::DISABLED_BY_USER),
            Err(AppError::Message(ref m)) if m.contains("Startup")
        ));
        assert!(matches!(
            enable_outcome(startup_state::DISABLED_BY_POLICY),
            Err(AppError::Message(ref m)) if m.contains("policy")
        ));
        assert!(matches!(enable_outcome(startup_state::ENABLED), Ok(true)));
        assert!(matches!(enable_outcome(startup_state::DISABLED), Ok(false)));
    }

    /// The mirrored constants above are hand-written; this pins them to the
    /// WinRT enum so a future SDK renumbering cannot drift past us unnoticed.
    #[cfg(windows)]
    #[test]
    fn mirrored_startup_states_match_winrt() {
        use windows::ApplicationModel::StartupTaskState;
        assert_eq!(startup_state::DISABLED, StartupTaskState::Disabled.0);
        assert_eq!(
            startup_state::DISABLED_BY_USER,
            StartupTaskState::DisabledByUser.0
        );
        assert_eq!(startup_state::ENABLED, StartupTaskState::Enabled.0);
        assert_eq!(
            startup_state::DISABLED_BY_POLICY,
            StartupTaskState::DisabledByPolicy.0
        );
        assert_eq!(
            startup_state::ENABLED_BY_POLICY,
            StartupTaskState::EnabledByPolicy.0
        );
    }
}
