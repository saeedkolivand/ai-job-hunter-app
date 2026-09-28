use super::*;
use std::path::PathBuf;

#[test]
fn firefox_manifest_has_allowed_extensions_and_stdio() {
    let exe = PathBuf::from(if cfg!(windows) {
        r"C:\Program Files\AI Job Hunter\app.exe"
    } else {
        "/opt/aijobhunter/app"
    });
    let bytes = manifest_json(&exe, true);
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["name"], NATIVE_HOST_NAME);
    assert_eq!(v["type"], "stdio");
    assert_eq!(v["allowed_extensions"][0], FIREFOX_GECKO_ID);
    assert!(v.get("allowed_origins").is_none());
    // The exe path round-trips (serde_json escaped any backslashes).
    assert_eq!(v["path"], exe.to_string_lossy().as_ref());
}

#[test]
fn chrome_manifest_has_allowed_origins_with_trailing_slash() {
    let exe = PathBuf::from("/opt/aijobhunter/app");
    let bytes = manifest_json(&exe, false);
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["type"], "stdio");
    assert_eq!(v["allowed_origins"][0], CHROME_ALLOWED_ORIGIN);
    assert!(v["allowed_origins"][0].as_str().unwrap().ends_with('/'));
    assert!(v.get("allowed_extensions").is_none());
}

#[test]
fn write_manifest_if_app_dir_exists_skips_when_guard_absent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let guard = tmp.path().join("nonexistent-app");
    let target = guard.join("config/NativeMessagingHosts/host.json");
    // Must NOT create any file or directory when the guard dir is absent.
    write_manifest_if_app_dir_exists("test", &guard, &target, b"{}");
    assert!(
        !target.exists(),
        "should not create file when guard is absent"
    );
    assert!(!guard.exists(), "should not create guard dir");
}

#[test]
fn write_manifest_if_app_dir_exists_writes_when_guard_present() {
    let tmp = tempfile::TempDir::new().unwrap();
    let guard = tmp.path().join("com.google.Chrome");
    std::fs::create_dir_all(&guard).unwrap();
    let target = guard
        .join("config/google-chrome/NativeMessagingHosts")
        .join("host.json");
    write_manifest_if_app_dir_exists("com.google.Chrome", &guard, &target, b"{\"ok\":true}");
    assert!(
        target.exists(),
        "manifest should be written when guard exists"
    );
    let content = std::fs::read(&target).unwrap();
    assert_eq!(content, b"{\"ok\":true}");
}
