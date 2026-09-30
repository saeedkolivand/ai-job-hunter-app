use super::*;

fn opencode_files(contents: &str) -> [(&'static str, String); 1] {
    [(".opencode/opencode.json", contents.to_string())]
}

#[test]
fn files_are_written_with_exact_contents() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = prepare_workspace(tmp.path(), "opencode", &opencode_files("{\"a\":1}")).unwrap();
    assert_eq!(
        fs::read_to_string(ws.path().join(".opencode/opencode.json")).unwrap(),
        "{\"a\":1}"
    );
}

/// The race CodeRabbit found on #1275: a second spawn of the same provider
/// must not touch the first spawn's config while the first is still running.
#[test]
fn concurrent_spawns_get_separate_dirs_and_keep_their_config() {
    let tmp = tempfile::tempdir().unwrap();
    let first = prepare_workspace(tmp.path(), "opencode", &opencode_files("first")).unwrap();
    let second = prepare_workspace(tmp.path(), "opencode", &opencode_files("second")).unwrap();

    assert_ne!(first.path(), second.path());
    assert_eq!(
        fs::read_to_string(first.path().join(".opencode/opencode.json")).unwrap(),
        "first"
    );
    drop(second);
    assert!(first.path().join(".opencode/opencode.json").exists());
}

#[test]
fn the_run_dir_is_removed_on_drop() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}")).unwrap();
    let dir = ws.path().to_path_buf();
    drop(ws);
    assert!(!dir.exists());
}

#[test]
fn each_run_starts_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let first = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}")).unwrap();
    fs::write(first.path().join("planted.md"), "permission: allow").unwrap();

    let second = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}")).unwrap();
    assert!(!second.path().join("planted.md").exists());
}

#[cfg(unix)]
#[test]
fn a_symlinked_workspace_root_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("elsewhere");
    fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, tmp.path().join("cli-workspaces")).unwrap();

    let err = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}"))
        .err()
        .unwrap();
    assert!(err.to_string().contains("link"), "{err}");
    assert!(
        fs::read_dir(&real).unwrap().next().is_none(),
        "nothing written through the link"
    );
}

/// A directory junction: the Windows reparse point that needs no admin
/// rights, so this runs on every Windows machine and CI runner.
#[cfg(windows)]
fn junction(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "mklink /J failed");
}

#[cfg(windows)]
#[test]
fn a_junctioned_workspace_root_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("elsewhere");
    fs::create_dir_all(&real).unwrap();
    junction(&tmp.path().join("cli-workspaces"), &real);

    let err = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}"))
        .err()
        .unwrap();
    assert!(err.to_string().contains("reparse point"), "{err}");
    // Path privacy: the error names the file, never the full path.
    let tmp_path = tmp.path().to_string_lossy().to_string();
    assert!(
        !err.to_string().contains(&tmp_path),
        "full path leaked: {err}"
    );
    assert!(
        fs::read_dir(&real).unwrap().next().is_none(),
        "nothing written through the junction"
    );
}

#[cfg(windows)]
#[test]
fn a_junctioned_provider_dir_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("elsewhere");
    fs::create_dir_all(&real).unwrap();
    fs::create_dir_all(tmp.path().join("cli-workspaces")).unwrap();
    junction(&tmp.path().join("cli-workspaces").join("opencode"), &real);

    let err = prepare_workspace(tmp.path(), "opencode", &opencode_files("{}"))
        .err()
        .unwrap();
    assert!(err.to_string().contains("reparse point"), "{err}");
    // Path privacy: the error names the file, never the full path.
    let tmp_path = tmp.path().to_string_lossy().to_string();
    assert!(
        !err.to_string().contains(&tmp_path),
        "full path leaked: {err}"
    );
    assert!(
        fs::read_dir(&real).unwrap().next().is_none(),
        "nothing written through the junction"
    );
}
