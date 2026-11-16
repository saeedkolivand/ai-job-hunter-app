use super::*;

#[test]
fn maps_every_channel() {
    use PackageFlavour::{MsStore, Snap};
    // (flavour, flatpak, appimage, windows, macos) -> label
    let cases = [
        (Some(MsStore), false, false, true, false, "msstore"),
        (None, false, false, true, false, "windows-installer"),
        (None, false, false, false, true, "macos"),
        (Some(Snap), false, false, false, false, "snap"),
        (None, true, false, false, false, "flatpak"),
        (None, false, true, false, false, "appimage"),
        (None, false, false, false, false, "deb-or-rpm"),
        // A snap wins over an inherited AppImage/Flatpak environment.
        (Some(Snap), true, true, false, false, "snap"),
    ];
    for (flavour, flatpak, appimage, windows, macos, want) in cases {
        assert_eq!(decide(flavour, flatpak, appimage, windows, macos), want);
    }
}

#[test]
fn detect_returns_a_known_label() {
    let known = [
        "snap",
        "flatpak",
        "appimage",
        "deb-or-rpm",
        "msstore",
        "windows-installer",
        "macos",
    ];
    assert!(known.contains(&detect()));
}
