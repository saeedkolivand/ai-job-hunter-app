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
        alias_directory(
            Path::new("C:/Users/t/AppData/Local"),
            "Publisher.App_abcdefg"
        )
        .join(ALIAS_EXE_NAME),
        root("C:/Users/t/AppData/Local/Microsoft/WindowsApps/Publisher.App_abcdefg/ajh-tauri.exe")
    );
}

/// The three states a caller has to handle. The middle one is the whole
/// point: a packaged build whose shim is missing publishes NOTHING rather
/// than falling back to the WindowsApps path.
#[test]
fn what_may_be_published_has_three_answers() {
    let alias = root(r"C:\Users\t\AppData\Local\Microsoft\WindowsApps\P_abc\ajh-tauri.exe");

    assert_eq!(
        published_exe(false, Some(alias.clone()), true),
        PublishedExe::Unpackaged
    );
    assert_eq!(
        published_exe(true, Some(alias.clone()), true),
        PublishedExe::Alias(alias.clone())
    );
    // Shim gone (a user can switch an execution alias off in Settings).
    assert_eq!(
        published_exe(true, Some(alias), false),
        PublishedExe::Unavailable
    );
    // …and the same when the alias path could not even be built.
    assert_eq!(published_exe(true, None, false), PublishedExe::Unavailable);
}

/// Not packaged here, so the real resolver reports `Unpackaged` — which is
/// what keeps every caller's `current_exe()` path byte-identical — and
/// neither startup-task entry point does anything.
#[tokio::test]
async fn nothing_is_published_or_driven_when_unpackaged() {
    assert_eq!(published_exe_path(), PublishedExe::Unpackaged);
    assert!(startup_task_enabled().await.is_none());
    assert!(set_startup_task(true).await.is_none());
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
