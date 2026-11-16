use super::*;

const NO_ENV: fn(&str) -> Option<OsString> = |_| None;
const NO_FILES: fn(&Path) -> bool = |_| false;

/// Linux, no packaging, no probes firing, unless the arguments say otherwise.
fn linux(
    flavour: Option<PackageFlavour>,
    env: impl Fn(&str) -> Option<OsString>,
    exists: impl Fn(&Path) -> bool,
    appimage: bool,
) -> &'static str {
    detect_from(flavour, env, exists, || appimage, false, false)
}

#[test]
fn nothing_on_linux_is_deb_or_rpm() {
    assert_eq!(linux(None, NO_ENV, NO_FILES, false), "deb-or-rpm");
}

#[test]
fn snap_flavour_is_snap_even_with_other_markers_present() {
    let env = |k: &str| (k == "FLATPAK_ID").then(|| OsString::from("x"));
    assert_eq!(
        linux(Some(PackageFlavour::Snap), env, |_| true, true),
        "snap"
    );
}

#[test]
fn flatpak_is_the_env_var_or_the_info_file() {
    let env = |k: &str| (k == "FLATPAK_ID").then(|| OsString::from("org.example.App"));
    assert_eq!(linux(None, env, NO_FILES, false), "flatpak");
    let info_only = |p: &Path| p == Path::new("/.flatpak-info");
    assert_eq!(linux(None, NO_ENV, info_only, false), "flatpak");
    // Some other file existing is not a Flatpak.
    assert_eq!(
        linux(None, NO_ENV, |p| p == Path::new("/x"), false),
        "deb-or-rpm"
    );
}

#[test]
fn appimage_probe_decides_after_flatpak() {
    assert_eq!(linux(None, NO_ENV, NO_FILES, true), "appimage");
    let env = |k: &str| (k == "FLATPAK_ID").then(|| OsString::from("x"));
    assert_eq!(linux(None, env, NO_FILES, true), "flatpak");
}

#[test]
fn appimage_probe_is_not_run_when_already_decided() {
    let env = |k: &str| (k == "FLATPAK_ID").then(|| OsString::from("x"));
    let label = detect_from(
        None,
        env,
        NO_FILES,
        || panic!("must not be probed"),
        false,
        false,
    );
    assert_eq!(label, "flatpak");
}

#[test]
fn windows_and_macos_are_decided_by_target() {
    let win = |flavour| detect_from(flavour, NO_ENV, NO_FILES, || false, true, false);
    assert_eq!(win(Some(PackageFlavour::MsStore)), "msstore");
    assert_eq!(win(None), "windows-installer");
    assert_eq!(
        detect_from(None, NO_ENV, NO_FILES, || false, false, true),
        "macos"
    );
}
