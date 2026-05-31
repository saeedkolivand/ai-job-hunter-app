use super::*;

#[test]
fn test_detect_system_chrome_env_var() {
    // Test that environment variable is checked
    // This test verifies the function doesn't panic
    let _ = detect_system_chrome();
}

#[test]
fn test_detect_system_chrome_nonexistent_env() {
    // Test with non-existent env var
    unsafe {
        std::env::set_var("CHROME", "/nonexistent/path/chrome.exe");
    }
    let result = detect_system_chrome();
    // Should not return the non-existent path
    assert!(result.is_none() || result.unwrap().exists());
    unsafe {
        std::env::remove_var("CHROME");
    }
}

#[cfg(target_os = "windows")]
#[test]
fn test_detect_chrome_windows_registry_keys() {
    // Test that the function doesn't panic when checking registry
    let _ = detect_chrome_windows();
}

#[cfg(target_os = "macos")]
#[test]
fn test_detect_chrome_macos_bundles() {
    // Test that the function doesn't panic when checking bundle paths
    let _ = detect_chrome_macos();
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
#[test]
fn test_detect_chrome_linux_binaries() {
    // Test that the function doesn't panic when checking for binaries
    let _ = detect_chrome_linux();
}
