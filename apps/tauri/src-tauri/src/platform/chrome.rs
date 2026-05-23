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

    #[cfg(not(target_os = "windows"))]
    {
        detect_chrome_unix()
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

#[cfg(not(target_os = "windows"))]
fn detect_chrome_unix() -> Option<std::path::PathBuf> {
    use std::process::Command;

    // Unix: try `which google-chrome` or `which chromium`.
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
