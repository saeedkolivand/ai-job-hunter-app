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
