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

    use crate::platform::NoWindow;

    // Query Windows registry for Chrome or Edge.
    for key in &[
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\msedge.exe",
    ] {
        let output = Command::new("reg")
            .args(["query", key, "/ve"])
            .no_window()
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

// ── Browser user-data roots ──────────────────────────────────────────────────
//
// The cookie-import flow (scraping::board_login::import) needs the *user-data
// root* of each installed Chromium browser — the directory that holds both the
// profiles (`Default`, `Profile N`, each with `Network/Cookies`) and the
// `Local State` file (which carries the DPAPI/Keychain-wrapped os_crypt key).
// This is distinct from the executable path returned by detect_system_chrome().

/// A Chromium browser family we know how to locate a user-data root for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromiumBrowser {
    Chrome,
    Edge,
    Brave,
}

impl ChromiumBrowser {
    pub const ALL: [ChromiumBrowser; 3] = [
        ChromiumBrowser::Chrome,
        ChromiumBrowser::Edge,
        ChromiumBrowser::Brave,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ChromiumBrowser::Chrome => "chrome",
            ChromiumBrowser::Edge => "edge",
            ChromiumBrowser::Brave => "brave",
        }
    }
}

/// Discover the user-data root for each installed Chromium browser, if present.
/// Returns only roots that actually exist on disk. Never errors — a missing
/// browser is simply absent from the result.
pub fn detect_chromium_user_data_roots() -> Vec<(ChromiumBrowser, std::path::PathBuf)> {
    ChromiumBrowser::ALL
        .into_iter()
        .filter_map(|b| chromium_user_data_root(b).map(|p| (b, p)))
        .filter(|(_, p)| p.exists())
        .collect()
}

#[cfg(target_os = "windows")]
fn chromium_user_data_root(browser: ChromiumBrowser) -> Option<std::path::PathBuf> {
    // %LOCALAPPDATA%\<Vendor>\<Product>\User Data
    let local = std::env::var_os("LOCALAPPDATA")?;
    let rel = match browser {
        ChromiumBrowser::Chrome => ["Google", "Chrome", "User Data"],
        ChromiumBrowser::Edge => ["Microsoft", "Edge", "User Data"],
        ChromiumBrowser::Brave => ["BraveSoftware", "Brave-Browser", "User Data"],
    };
    let mut p = std::path::PathBuf::from(local);
    for seg in rel {
        p.push(seg);
    }
    Some(p)
}

#[cfg(target_os = "macos")]
fn chromium_user_data_root(browser: ChromiumBrowser) -> Option<std::path::PathBuf> {
    // ~/Library/Application Support/<Vendor>/<Product>
    let home = std::env::var_os("HOME")?;
    let rel: &[&str] = match browser {
        ChromiumBrowser::Chrome => &["Library", "Application Support", "Google", "Chrome"],
        ChromiumBrowser::Edge => &["Library", "Application Support", "Microsoft Edge"],
        ChromiumBrowser::Brave => &[
            "Library",
            "Application Support",
            "BraveSoftware",
            "Brave-Browser",
        ],
    };
    let mut p = std::path::PathBuf::from(home);
    for seg in rel {
        p.push(seg);
    }
    Some(p)
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn chromium_user_data_root(browser: ChromiumBrowser) -> Option<std::path::PathBuf> {
    // ~/.config/<product>
    let home = std::env::var_os("HOME")?;
    let rel: &[&str] = match browser {
        ChromiumBrowser::Chrome => &[".config", "google-chrome"],
        ChromiumBrowser::Edge => &[".config", "microsoft-edge"],
        ChromiumBrowser::Brave => &[".config", "BraveSoftware", "Brave-Browser"],
    };
    let mut p = std::path::PathBuf::from(home);
    for seg in rel {
        p.push(seg);
    }
    Some(p)
}

#[cfg(test)]
mod test;
