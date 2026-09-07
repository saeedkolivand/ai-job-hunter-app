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
fn decide(identity: &Identity, exe: Option<&Path>) -> bool {
    match identity {
        Identity::Absent => false,
        Identity::Unknown => true,
        // Identity, but the OS would not name its install root — nothing to
        // compare against, so fall back to the safe direction.
        Identity::Present(None) => true,
        Identity::Present(Some(root)) => match exe {
            Some(exe) => exe.starts_with(root),
            None => true,
        },
    }
}

/// `true` when this process runs with MSIX package identity (Store build).
///
/// Cached: identity is fixed for the lifetime of a process, so the OS probe
/// happens at most once — which also means the `Unknown` warning below is
/// logged at most once.
pub fn is_packaged() -> bool {
    static PACKAGED: OnceLock<bool> = OnceLock::new();
    *PACKAGED.get_or_init(|| decide(&probe_identity(), std::env::current_exe().ok().as_deref()))
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
        Identity::Present(current_package_string(GetCurrentPackagePath).map(PathBuf::from))
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

/// `%LOCALAPPDATA%\Microsoft\WindowsApps\<PackageFamilyName>\<alias>` — where
/// Windows puts the execution-alias shim for a package. Pure, so the layout is
/// tested on every host.
fn alias_path(local_app_data: &Path, family: &str) -> PathBuf {
    local_app_data
        .join("Microsoft")
        .join("WindowsApps")
        .join(family)
        .join(ALIAS_EXE_NAME)
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
    let family = package_family_name()?;
    let local = std::env::var_os("LOCALAPPDATA")?;
    let path = alias_path(Path::new(&local), &family);
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
/// 0.3). Blocking is right here: both callers are synchronous Tauri commands
/// that already blocked on a registry read, and the task lookup is a local
/// OS call.
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

    fn root(p: &str) -> PathBuf {
        PathBuf::from(p)
    }

    #[test]
    fn no_identity_is_unpackaged() {
        assert!(!decide(
            &Identity::Absent,
            Some(&root("/anywhere/ajh-tauri.exe"))
        ));
    }

    /// The safe direction: an unreadable probe must not hand a packaged build
    /// back to the GitHub updater.
    #[test]
    fn unknown_probe_counts_as_packaged() {
        assert!(decide(
            &Identity::Unknown,
            Some(&root("/anywhere/ajh-tauri.exe"))
        ));
        assert!(decide(&Identity::Present(None), None));
    }

    #[test]
    fn identity_plus_exe_inside_the_package_is_packaged() {
        let install = root("C:/Program Files/WindowsApps/Publisher.App_1.2.3.0_x64__abc");
        let exe = install.join("ajh-tauri.exe");
        assert!(decide(&Identity::Present(Some(install)), Some(&exe)));
    }

    /// The inherited-identity case: an NSIS-installed exe spawned by the
    /// packaged app inherits package identity but runs from its own directory,
    /// and must keep the normal updater.
    #[test]
    fn identity_inherited_by_a_child_outside_the_package_is_not_packaged() {
        let install = root("C:/Program Files/WindowsApps/Publisher.App_1.2.3.0_x64__abc");
        let exe = root("C:/Users/tester/AppData/Local/AI Job Hunter/ajh-tauri.exe");
        assert!(!decide(&Identity::Present(Some(install)), Some(&exe)));
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
    fn alias_path_is_the_family_scoped_shim() {
        assert_eq!(
            alias_path(Path::new("C:/Users/t/AppData/Local"), "Publisher.App_abcdefg"),
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
