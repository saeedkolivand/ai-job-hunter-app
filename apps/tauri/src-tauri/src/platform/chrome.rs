/// Detect system Chrome or Edge to avoid chromiumoxide's 120 MB download.
/// Returns the path if found, None otherwise.
pub fn detect_system_chrome() -> Option<std::path::PathBuf> {
    // Check $CHROME env var first.
    if let Ok(path) = std::env::var("CHROME") {
        let pb = std::path::PathBuf::from(path);
        if pb.exists() {
            return Some(pb);
        }
    }

    #[cfg(target_os = "windows")]
    {
        detect_chrome_windows()
    }

    #[cfg(target_os = "macos")]
    {
        detect_chrome_macos()
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        detect_chrome_linux()
    }
}

#[cfg(target_os = "windows")]
fn detect_chrome_windows() -> Option<std::path::PathBuf> {
    use std::process::Command;

    // Query Windows registry for Chrome or Edge.
    for key in &[
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
    ] {
        let output = Command::new("reg")
            .args(&["query", key, "/ve"])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    if line.contains("REG_SZ") {
                        let parts: Vec<&str> = line.split("REG_SZ").collect();
                        if parts.len() > 1 {
                            let path = parts[1].trim();
                            let pb = std::path::PathBuf::from(path);
                            if pb.exists() {
                                return Some(pb);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn detect_chrome_macos() -> Option<std::path::PathBuf> {
    use std::process::Command;

    // Check well-known macOS application bundle paths first.
    let bundle_paths = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    ];
    for path in &bundle_paths {
        let pb = std::path::PathBuf::from(path);
        if pb.exists() {
            return Some(pb);
        }
    }

    // Also check the user's ~/Applications folder.
    if let Some(home) = std::env::var_os("HOME") {
        let user_bundle_paths = [
            "Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            "Applications/Chromium.app/Contents/MacOS/Chromium",
            "Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ];
        for rel in &user_bundle_paths {
            let pb = std::path::PathBuf::from(&home).join(rel);
            if pb.exists() {
                return Some(pb);
            }
        }
    }

    // Fallback: binaries in PATH (e.g. installed via Homebrew).
    for bin in &["google-chrome", "chromium", "chromium-browser"] {
        let output = Command::new("which").arg(bin).output();
        if let Ok(out) = output {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    let pb = std::path::PathBuf::from(path);
                    if pb.exists() {
                        return Some(pb);
                    }
                }
            }
        }
    }
    None
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn detect_chrome_linux() -> Option<std::path::PathBuf> {
    use std::process::Command;

    // Linux: try `which google-chrome` or `which chromium`.
    for bin in &["google-chrome", "chromium", "chromium-browser"] {
        let output = Command::new("which").arg(bin).output();
        if let Ok(out) = output {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    let pb = std::path::PathBuf::from(path);
                    if pb.exists() {
                        return Some(pb);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
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
        unsafe { std::env::set_var("CHROME", "/nonexistent/path/chrome.exe"); }
        let result = detect_system_chrome();
        // Should not return the non-existent path
        assert!(result.is_none() || result.unwrap().exists());
        unsafe { std::env::remove_var("CHROME"); }
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
}
