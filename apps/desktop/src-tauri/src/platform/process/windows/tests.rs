use std::fs;

use super::*;

// Hermetic: a fake PATH dir + injected PATHEXT, so no process-env mutation and
// no assumption about which binaries exist on the runner.

#[test]
fn resolves_cmd_shim_and_flags_the_wrapper() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("foo.cmd"), "@echo off\r\n").unwrap();
    let path_var = std::ffi::OsString::from(dir.path());

    let r = resolve_cli_binary_in("foo", &path_var, ".COM;.EXE;.BAT;.CMD").unwrap();
    assert!(r.needs_cmd_wrapper, ".cmd shim must be flagged for cmd.exe");
    assert_eq!(
        r.path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase(),
        "foo.cmd"
    );
}

#[test]
fn resolves_exe_directly_without_wrapper() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bar.exe"), b"MZ").unwrap();
    let path_var = std::ffi::OsString::from(dir.path());

    let r = resolve_cli_binary_in("bar", &path_var, ".COM;.EXE;.BAT;.CMD").unwrap();
    assert!(!r.needs_cmd_wrapper, ".exe is launched directly");
    assert_eq!(
        r.path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase(),
        "bar.exe"
    );
}

#[test]
fn missing_binary_resolves_to_none() {
    let dir = tempfile::tempdir().unwrap();
    let path_var = std::ffi::OsString::from(dir.path());
    assert!(resolve_cli_binary_in("nope", &path_var, ".COM;.EXE;.BAT;.CMD").is_none());
}

/// Regression for #1292: npm writes an extensionless `#!/bin/sh` shim next to
/// `<name>.cmd`. The bare name must never win over the `.cmd` sibling.
#[test]
fn extensionless_shell_shim_loses_to_its_cmd_sibling() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("opencode"), "#!/bin/sh\n").unwrap();
    fs::write(dir.path().join("opencode.cmd"), "@echo off\r\n").unwrap();
    let path_var = std::ffi::OsString::from(dir.path());

    let r = resolve_cli_binary_in("opencode", &path_var, ".COM;.EXE;.BAT;.CMD").unwrap();
    assert!(
        r.needs_cmd_wrapper,
        "must resolve to the .cmd shim, not the shell script"
    );
    assert_eq!(
        r.path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase(),
        "opencode.cmd"
    );
}

#[test]
fn extensionless_only_file_is_not_a_hit() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("tool"), "#!/bin/sh\n").unwrap();
    let path_var = std::ffi::OsString::from(dir.path());

    assert!(
        resolve_cli_binary_in("tool", &path_var, ".COM;.EXE;.BAT;.CMD").is_none(),
        "a file Windows can't execute is not a hit"
    );
}

#[test]
fn explicit_extension_is_still_honoured() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("tool.cmd"), "@echo off\r\n").unwrap();
    let path_var = std::ffi::OsString::from(dir.path());

    let r = resolve_cli_binary_in("tool.cmd", &path_var, ".COM;.EXE;.BAT;.CMD").unwrap();
    assert_eq!(
        r.path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase(),
        "tool.cmd"
    );
}

#[test]
fn path_order_beats_pathext_order() {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    fs::write(dir_a.path().join("x.cmd"), "@echo off\r\n").unwrap();
    fs::write(dir_b.path().join("x.exe"), b"MZ").unwrap();
    let path_var = std::env::join_paths([dir_a.path(), dir_b.path()]).unwrap();

    // PATHEXT lists .EXE before .CMD, but dir_a (earlier on PATH) holds only
    // the .cmd — PATH order must still win.
    let r = resolve_cli_binary_in("x", &path_var, ".COM;.EXE;.BAT;.CMD").unwrap();
    assert_eq!(
        r.path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase(),
        "x.cmd",
        "PATH order (dir_a first) must beat PATHEXT order (.exe before .cmd)"
    );
}
