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
