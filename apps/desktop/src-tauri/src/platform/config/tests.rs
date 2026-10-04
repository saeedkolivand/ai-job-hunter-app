use super::*;

// Env-var override and default are exercised in a single test: `AJH_DATA_DIR`
// is process-global, so splitting them into parallel tests races (one's
// remove_var can land between the other's set_var and read).
// `#[serial]` because this mutates the process-global var directly (it is
// testing the resolver itself, so it cannot go through `DataDirGuard`).
// Without it, it races any other `#[serial]` mutator — which was a real
// gap once a second test elsewhere began scoping the same variable.
#[test]
#[serial_test::serial]
fn data_dir_honors_env_then_falls_back() {
    // Override via env var.
    unsafe {
        std::env::set_var(DATA_DIR_ENV, "/custom/path");
    }
    assert_eq!(data_dir().to_string_lossy(), "/custom/path");

    // Default falls back to USERPROFILE/HOME and ends with .ajh.
    unsafe {
        std::env::remove_var(DATA_DIR_ENV);
    }
    assert!(data_dir().to_string_lossy().contains(FALLBACK_DIR_NAME));
}

// `home_dir`/`USERPROFILE` mutate process-global env — `#[serial]` so this
// can't race `data_dir_honors_env_then_falls_back` (a different var) or any
// other `#[serial]` mutator in this module.
#[test]
#[serial_test::serial]
fn home_dir_honors_userprofile_before_home() {
    // `HomeDirGuard`'s `Drop` restores whatever `USERPROFILE`/`HOME` were
    // BEFORE this test ran, even on a panicking assert below (MEDIUM fix
    // — security review): the manual save/restore block this replaces
    // only ran on a normal return, so a failing assert used to leak
    // mutated env into every later test in this process. The path here
    // is never read (this test overwrites both vars immediately below,
    // for each state it exercises) — only the guard's captured
    // "restore to" values matter.
    let _guard = HomeDirGuard::set(std::path::Path::new("/unused-initial"));

    // USERPROFILE wins when both are set (the Windows case: HOME is
    // typically unset there, but this pins the precedence regardless).
    unsafe {
        std::env::set_var("USERPROFILE", "/from/userprofile");
        std::env::set_var("HOME", "/from/home");
    }
    assert_eq!(home_dir().unwrap().to_string_lossy(), "/from/userprofile");

    // HOME alone (USERPROFILE unset) — the pre-fix behavior, still honored.
    unsafe {
        std::env::remove_var("USERPROFILE");
    }
    assert_eq!(home_dir().unwrap().to_string_lossy(), "/from/home");

    // Neither set — this is what silently broke on Windows before the fix
    // (HOME-only never resolved there).
    unsafe {
        std::env::remove_var("HOME");
    }
    assert_eq!(home_dir(), None);
}

#[test]
#[serial_test::serial]
fn agent_pointer_path_sits_beside_not_inside_the_data_dir_fallback() {
    let home = std::path::Path::new("/home/tester");
    let _guard = HomeDirGuard::set(home);
    let path = agent_pointer_path().unwrap();
    assert_eq!(
        path,
        home.join(AGENT_POINTER_DIR_NAME)
            .join(AGENT_POINTER_FILE_NAME)
    );
    // Never the bare FALLBACK_DIR_NAME (`.ajh`) — that name is the data-dir
    // fallback, and a pointer file living there would be mistakable for it.
    assert!(!path.starts_with(home.join(FALLBACK_DIR_NAME)));
}

/// Every branch of the resolver, on EVERY host: the env reads are not
/// `#[cfg]`-gated (see `agent_cli_exe_path`'s doc), so a Windows/macOS
/// run covers the AppImage branch too instead of leaving it to a
/// Linux-only CI leg.
///
/// The AppImage case is built out of REAL filesystem objects — a temp
/// FILE for `$APPIMAGE` and this test binary's own parent dir for
/// `$APPDIR` — because two of the three conditions are facts about the
/// disk and the running process, and a string-only fixture could not
/// exercise either.
///
/// Mutation-visible, and each mutation is caught by a DIFFERENT case:
/// drop `is_file()` → case 3 publishes `gone.AppImage`; drop the
/// `starts_with($APPDIR)` check → case 2 publishes another app's
/// AppImage; drop the `$APPDIR` requirement (its `?` and the empty check,
/// leaving `starts_with("")`, which is true for every path) → case 4
/// publishes it with no `$APPDIR` set at all. All three verified by
/// applying them.
#[test]
#[serial_test::serial]
fn agent_cli_exe_path_publishes_the_appimage_only_when_this_process_is_that_appimage() {
    let running = std::env::current_exe().unwrap();
    let appdir = running.parent().unwrap().to_path_buf();
    let image = tempfile::NamedTempFile::new().unwrap();
    let image_path = image.path().to_path_buf();
    let elsewhere = tempfile::TempDir::new().unwrap();

    // 1. The real thing: `$APPIMAGE` names a file that exists, `$APPDIR`
    //    is set, and this process's image lives under it.
    {
        let _guard = AppImageGuard::set(Some(&image_path), Some(&appdir));
        assert_eq!(
            agent_cli_exe_path().unwrap(),
            image_path,
            "inside its own AppImage the durable path is the .AppImage file, not the \
                 transient mount `current_exe()` returns"
        );
    }

    // 2. THE BUG THIS PREDICATE EXISTS FOR: another AppImage app's
    //    environment, inherited through a terminal it spawned. The file
    //    exists — it is a real AppImage — but it is not ours, and this
    //    binary does not live under its mount point.
    {
        let _guard = AppImageGuard::set(Some(&image_path), Some(elsewhere.path()));
        assert_eq!(
            agent_cli_exe_path().unwrap(),
            running,
            "a .deb build must never publish the AppImage of whatever app spawned its \
                 terminal"
        );
    }

    // 3. `$APPIMAGE` names nothing on disk (a stale inherited value, or a
    //    deleted image).
    {
        let missing = elsewhere.path().join("gone.AppImage");
        let _guard = AppImageGuard::set(Some(&missing), Some(&appdir));
        assert_eq!(agent_cli_exe_path().unwrap(), running);
    }

    // 4. `$APPDIR` unset, and 5. empty — the runtime always exports it, so
    //    either state means the pair did not come from a live AppImage.
    //    (On Windows `set_var(k, "")` removes the variable, which lands in
    //    the same branch — both are "no usable `$APPDIR`".)
    {
        let _guard = AppImageGuard::set(Some(&image_path), None);
        assert_eq!(agent_cli_exe_path().unwrap(), running);
    }
    {
        let _guard = AppImageGuard::set(Some(&image_path), Some(Path::new("")));
        assert_eq!(
            agent_cli_exe_path().unwrap(),
            running,
            "`Path::starts_with(\"\")` is true for every path — an empty $APPDIR must not \
                 satisfy the check"
        );
    }

    // 6. Neither var set: the ordinary non-AppImage install.
    {
        let _guard = AppImageGuard::set(None, None);
        assert_eq!(agent_cli_exe_path().unwrap(), running);
    }
    // 7. …and an empty `$APPIMAGE`, which names no file.
    {
        let _guard = AppImageGuard::set(Some(Path::new("")), Some(&appdir));
        assert_eq!(agent_cli_exe_path().unwrap(), running);
    }
}
