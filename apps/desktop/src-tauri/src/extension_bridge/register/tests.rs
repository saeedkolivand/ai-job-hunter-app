use super::*;

// Only used by the Linux-only test below (the native-manifest path is a
// Linux-only registration branch).
#[cfg(target_os = "linux")]
use super::super::NATIVE_HOST_MANIFEST;

/// `#[serial]`: mutates the process-global `USERPROFILE`/`HOME` that
/// `platform::config::home_dir` (and therefore `agent_pointer_path`)
/// reads — same discipline as `platform::config`'s own env tests.
#[test]
#[serial_test::serial]
fn write_agent_pointer_writes_exe_path_and_data_dir() {
    let home = tempfile::TempDir::new().unwrap();
    // Routes through `platform::config`'s test-only guards rather than
    // touching `std::env` here directly — R4 ("env access only in
    // platform/**") text-scans every non-test-named file, including a
    // `#[cfg(test)]` module embedded in one like this.
    let _guard = crate::platform::config::HomeDirGuard::set(home.path());
    // The ordinary install: no AppImage environment, so the pointer must
    // carry the running binary itself. Pinned rather than inherited — a
    // developer running this suite from inside some AppImage's terminal
    // would otherwise see a different `exePath` than CI does.
    let _appimage = crate::platform::config::AppImageGuard::set(None, None);

    let data_dir = tempfile::TempDir::new().unwrap();
    write_agent_pointer(data_dir.path());

    let pointer_path = home.path().join(".ajh-agent").join("agent.json");
    let contents = std::fs::read_to_string(&pointer_path)
        .expect("pointer file must exist under <home>/.ajh-agent/agent.json");
    let v: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert_eq!(
        v["exePath"],
        std::env::current_exe().unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(v["dataDir"], data_dir.path().to_string_lossy().as_ref());
}

/// The branch the CALLER used to decide, untested, one revert away from
/// publishing a path that stops existing the moment the app closes: inside
/// an AppImage the pointer must carry the `.AppImage` file, not the
/// transient mount `current_exe()` returns.
///
/// Drives `write_agent_pointer` rather than `register_native_host`
/// deliberately. That function is the pointer write PLUS the
/// browser-manifest registration, and on Windows the latter writes real
/// `HKCU\Software\{Mozilla,Google}\NativeMessagingHosts` values pointing
/// at the manifest — under a temp data dir, that would repoint the
/// developer's own native-messaging host at a directory this test then
/// deletes. So the resolution moved INTO `write_agent_pointer` (see its
/// doc) and the caller no longer has a choice to get wrong.
///
/// Mutation-visible: swap `agent_cli_exe_path()` for
/// `std::env::current_exe()` inside `write_agent_pointer` and this fails
/// with the test binary's own path.
#[test]
#[serial_test::serial]
fn the_pointer_publishes_the_appimage_path_not_the_transient_mount() {
    let home = tempfile::TempDir::new().unwrap();
    let _guard = crate::platform::config::HomeDirGuard::set(home.path());

    // The AppImage predicate, satisfied for real: an existing FILE in
    // `$APPIMAGE`, and an `$APPDIR` this process's own binary lives under
    // (see `platform::config::launched_appimage` for why both).
    let running = std::env::current_exe().unwrap();
    let image = tempfile::NamedTempFile::new().unwrap();
    let _appimage = crate::platform::config::AppImageGuard::set(
        Some(image.path()),
        Some(running.parent().unwrap()),
    );

    let data_dir = tempfile::TempDir::new().unwrap();
    write_agent_pointer(data_dir.path());

    let pointer_path = home.path().join(".ajh-agent").join("agent.json");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer_path).unwrap()).unwrap();
    assert_eq!(
        v["exePath"],
        image.path().to_string_lossy().as_ref(),
        "the pointer must publish the durable .AppImage path a user can type"
    );
    assert_ne!(
        v["exePath"],
        running.to_string_lossy().as_ref(),
        "…and never the mount path, which is gone once the app exits"
    );
}

#[test]
#[serial_test::serial]
fn write_agent_pointer_is_idempotent_and_overwrites() {
    let home = tempfile::TempDir::new().unwrap();
    let _guard = crate::platform::config::HomeDirGuard::set(home.path());

    let data_dir_a = tempfile::TempDir::new().unwrap();
    let data_dir_b = tempfile::TempDir::new().unwrap();
    write_agent_pointer(data_dir_a.path());
    write_agent_pointer(data_dir_b.path());

    let pointer_path = home.path().join(".ajh-agent").join("agent.json");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer_path).unwrap()).unwrap();
    assert_eq!(
        v["dataDir"],
        data_dir_b.path().to_string_lossy().as_ref(),
        "a second launch's pointer must overwrite the first, not append"
    );
}

/// The agent-CLI pointer must survive the sandbox guard — it's what an
/// in-sandbox `snap run … agent mcp` invocation reads from the confined
/// HOME. Mutation-visible: reorder
/// `write_agent_pointer(data_dir)` back below the `if sandboxed { …
/// return; }` guard and this fails.
#[test]
#[serial_test::serial]
fn register_native_host_still_writes_the_agent_pointer_when_sandboxed() {
    let home = tempfile::TempDir::new().unwrap();
    let _guard = crate::platform::config::HomeDirGuard::set(home.path());
    let data_dir = tempfile::TempDir::new().unwrap();

    register_native_host_inner(data_dir.path(), true);

    let pointer_path = home.path().join(".ajh-agent").join("agent.json");
    assert!(
        pointer_path.exists(),
        "the agent-CLI pointer must be written even inside a sandbox"
    );
    assert_eq!(
        std::fs::read_dir(data_dir.path()).unwrap().count(),
        0,
        "no browser-manifest state should be written under data_dir"
    );
}

/// `#[cfg(target_os = "linux")]`, so no CI job actually runs this
/// assertion — CI's Rust jobs are `--test architecture`, `--test
/// egress`, one targeted `--lib` test on Windows, `--test mcp_smoke`,
/// and `cargo mutants --in-diff`; none is an ubuntu `--lib` run.
/// `cargo clippy --all-targets` on ubuntu at least keeps this compiling,
/// so it can't rot into a build break unnoticed. The actual mutation
/// coverage is the cross-platform sibling test above (real on every
/// host) plus THIS test running for real on a Linux dev machine's own
/// pre-push `cargo test`. Unlike the per-Flatpak-app guard dirs below,
/// the NATIVE Linux browser paths (`write_manifest`, not
/// `write_manifest_if_app_dir_exists`) are written unconditionally with
/// no directory precondition — so a `HomeDirGuard`-scoped temp HOME with
/// nothing pre-created still catches a deleted `if sandboxed { … return;
/// }` guard: delete it and Firefox's manifest appears here.
#[cfg(target_os = "linux")]
#[test]
#[serial_test::serial]
fn register_native_host_skips_the_native_browser_manifest_when_sandboxed() {
    let home = tempfile::TempDir::new().unwrap();
    let _guard = crate::platform::config::HomeDirGuard::set(home.path());
    let data_dir = tempfile::TempDir::new().unwrap();

    register_native_host_inner(data_dir.path(), true);

    let firefox_manifest = home
        .path()
        .join(".mozilla/native-messaging-hosts")
        .join(NATIVE_HOST_MANIFEST);
    assert!(
        !firefox_manifest.exists(),
        "a sandboxed process must not register the browser-spawned native-messaging host"
    );
}
