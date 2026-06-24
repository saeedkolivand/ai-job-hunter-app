/// How to launch a detected browser.
///
/// Callers that need a plain binary path (e.g. chromiumoxide) call
/// [`BrowserLaunch::to_executable_path`] — it returns `Some` only for native
/// paths and Snap wrappers that live on the filesystem. Flatpak browsers are
/// launched via `flatpak run <id>` and cannot be used as a bare binary, so
/// that method returns `None` for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserLaunch {
    /// A regular on-disk binary (native package, Homebrew, Snap wrapper, etc.).
    NativePath(std::path::PathBuf),
    /// Installed as a Flatpak; launch with `flatpak run <app-id>`.
    FlatpakApp(String),
}

impl BrowserLaunch {
    /// Returns the binary path for callers that need a real file (e.g.
    /// chromiumoxide's `.chrome_executable()`). Returns `None` for Flatpak
    /// installs, because those cannot be invoked as a bare binary path.
    pub fn to_executable_path(&self) -> Option<std::path::PathBuf> {
        match self {
            BrowserLaunch::NativePath(p) => Some(p.clone()),
            BrowserLaunch::FlatpakApp(_) => None,
        }
    }

    /// A human-readable launch command string, suitable for display or logging.
    pub fn display_command(&self) -> String {
        match self {
            BrowserLaunch::NativePath(p) => p.to_string_lossy().into_owned(),
            BrowserLaunch::FlatpakApp(id) => format!("flatpak run {id}"),
        }
    }
}

/// Detect system Chrome, Chromium, Brave, Edge, or Vivaldi to avoid
/// chromiumoxide's 120 MB download. Returns how to launch it, or `None`.
///
/// Checks `$CHROME` first, then falls back to platform detection.
pub fn detect_system_chrome() -> Option<BrowserLaunch> {
    // Check $CHROME env var first.
    if let Ok(path) = std::env::var("CHROME") {
        let pb = std::path::PathBuf::from(path);
        if pb.exists() {
            return Some(BrowserLaunch::NativePath(pb));
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
fn detect_chrome_windows() -> Option<BrowserLaunch> {
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
                                return Some(BrowserLaunch::NativePath(pb));
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
fn detect_chrome_macos() -> Option<BrowserLaunch> {
    use std::process::Command;

    // Check well-known macOS application bundle paths first.
    let bundle_paths = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        "/Applications/Vivaldi.app/Contents/MacOS/Vivaldi",
    ];
    for path in &bundle_paths {
        let pb = std::path::PathBuf::from(path);
        if pb.exists() {
            return Some(BrowserLaunch::NativePath(pb));
        }
    }

    // Also check the user's ~/Applications folder.
    if let Some(home) = std::env::var_os("HOME") {
        let user_bundle_paths = [
            "Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            "Applications/Chromium.app/Contents/MacOS/Chromium",
            "Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
            "Applications/Vivaldi.app/Contents/MacOS/Vivaldi",
        ];
        for rel in &user_bundle_paths {
            let pb = std::path::PathBuf::from(&home).join(rel);
            if pb.exists() {
                return Some(BrowserLaunch::NativePath(pb));
            }
        }
    }

    // Fallback: binaries in PATH (e.g. installed via Homebrew).
    for bin in &[
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "brave-browser",
        "microsoft-edge",
        "microsoft-edge-stable",
        "vivaldi-stable",
    ] {
        let output = Command::new("which").arg(bin).output();
        if let Ok(out) = output {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    let pb = std::path::PathBuf::from(path);
                    if pb.exists() {
                        return Some(BrowserLaunch::NativePath(pb));
                    }
                }
            }
        }
    }
    None
}

/// All Flatpak app-ids we probe for Chromium-family browsers, in priority order.
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
const FLATPAK_IDS: &[&str] = &[
    "com.google.Chrome",
    "org.chromium.Chromium",
    "com.brave.Browser",
    "com.microsoft.Edge",
    "com.vivaldi.Vivaldi",
];

/// Detect Chrome-family browsers on Linux/Steam Deck across native packages,
/// Snap installs, and Flatpak sandboxed installs.
///
/// Detection order (first match wins):
/// 1. PATH (`which`) — covers native + distro packages
/// 2. Absolute fallback paths — `/opt/google/chrome/…`, `/opt/microsoft/msedge/…`, etc.
/// 3. Snap — `/snap/bin/<name>` wrappers
/// 4. Flatpak — exported wrappers (`/var/lib/flatpak/exports/bin/` and
///    `~/.local/share/flatpak/exports/bin/`) and, if those aren't present,
///    `flatpak list` probe (requires `flatpak` on PATH and is bounded by a
///    short timeout)
///
/// Safety: no untrusted data is interpolated into shell strings. All
/// `Command` calls use fixed arg vectors.
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub(crate) fn detect_chrome_linux() -> Option<BrowserLaunch> {
    use std::process::Command;

    // 1. PATH binaries — native packages and distro repos.
    let which_bins = [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "brave-browser",
        "microsoft-edge",
        "microsoft-edge-stable",
        "vivaldi-stable",
    ];
    for bin in &which_bins {
        let output = Command::new("which").arg(bin).output();
        if let Ok(out) = output {
            if out.status.success() {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !path.is_empty() {
                    let pb = std::path::PathBuf::from(&path);
                    if pb.exists() {
                        return Some(BrowserLaunch::NativePath(pb));
                    }
                }
            }
        }
    }

    // 2. Absolute fallback paths — common non-PATH install locations.
    let abs_paths: &[&str] = &[
        "/opt/google/chrome/google-chrome",
        "/opt/google/chrome-unstable/google-chrome-unstable",
        "/opt/chromium.org/chromium/chrome",
        "/opt/microsoft/msedge/msedge",
        "/opt/brave.com/brave/brave",
        "/opt/vivaldi/vivaldi",
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/usr/bin/brave-browser",
        "/usr/bin/microsoft-edge",
        "/usr/bin/microsoft-edge-stable",
        "/usr/bin/vivaldi-stable",
    ];
    for path in abs_paths {
        let pb = std::path::PathBuf::from(path);
        if pb.exists() {
            return Some(BrowserLaunch::NativePath(pb));
        }
    }

    // 3. Snap — the wrapper scripts live in /snap/bin/.
    let snap_bins: &[&str] = &[
        "/snap/bin/chromium",
        "/snap/bin/google-chrome",
        "/snap/bin/brave",
        "/snap/bin/microsoft-edge",
    ];
    for path in snap_bins {
        let pb = std::path::PathBuf::from(path);
        if pb.exists() {
            return Some(BrowserLaunch::NativePath(pb));
        }
    }

    // 4. Flatpak — probe exported wrappers first (cheap: stat only), then fall
    //    back to `flatpak info` if needed.
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    if let Some(result) = probe_flatpak(home.as_deref()) {
        return Some(result);
    }

    None
}

/// Probe Flatpak for a known Chrome-family browser, returning the first match.
///
/// Checks (in order):
/// 1. System-wide exported wrappers — `/var/lib/flatpak/exports/bin/<id>`
/// 2. Per-user exported wrappers   — `<home>/.local/share/flatpak/exports/bin/<id>`
/// 3. `flatpak info <id>` — only when the `flatpak` binary is available
///
/// Extracted as a standalone function so tests can supply a controlled `home`
/// directory without depending on the runner's real filesystem state.
///
/// Returns `FlatpakApp(id)` for the first installed app-id found, or `None`.
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub(crate) fn probe_flatpak(home: Option<&std::path::Path>) -> Option<BrowserLaunch> {
    let user_exports = home.map(|h| h.join(".local/share/flatpak/exports/bin"));

    for id in FLATPAK_IDS {
        // System-wide export (root-owned — readable by all users).
        let sys = std::path::PathBuf::from("/var/lib/flatpak/exports/bin").join(id);
        if sys.exists() {
            return Some(BrowserLaunch::FlatpakApp(id.to_string()));
        }
        // Per-user export.
        if let Some(ref base) = user_exports {
            if base.join(id).exists() {
                return Some(BrowserLaunch::FlatpakApp(id.to_string()));
            }
        }
    }

    // Last resort: ask `flatpak info` (only if `flatpak` binary is on PATH —
    // avoids hanging on systems that don't have Flatpak at all).
    if flatpak_binary_available() {
        for id in FLATPAK_IDS {
            if flatpak_app_installed(id) {
                return Some(BrowserLaunch::FlatpakApp(id.to_string()));
            }
        }
    }

    None
}

/// Returns true if the `flatpak` CLI is available on PATH.
/// Cheap: just checks the exit status of `flatpak --version`.
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn flatpak_binary_available() -> bool {
    use std::process::Command;
    Command::new("flatpak")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Returns true if `flatpak info <id>` exits 0 (app is installed).
/// Bounded: inherits the OS process timeout; `flatpak info` is fast.
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn flatpak_app_installed(app_id: &str) -> bool {
    use std::process::Command;
    // Fixed arg vector — `app_id` comes from our own constant slice, never
    // from user input, so no shell-injection risk. We use Command::new rather
    // than a shell so there is no interpolation path at all.
    Command::new("flatpak")
        .args(["info", app_id])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
