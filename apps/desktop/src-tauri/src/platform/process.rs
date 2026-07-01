//! Shared subprocess helpers.
//!
//! On Windows, spawning a console subprocess flashes a transient console window;
//! `CREATE_NO_WINDOW` suppresses it. Every subprocess we spawn calls
//! [`NoWindow::no_window`] so this behavior is defined in exactly one place — new
//! spawn sites opt in with a single call. It is a compile-time no-op on macOS and
//! Linux, which don't show a window for a spawned child.

/// Adds `.no_window()` to the `Command` builders we use (`std` and `tokio`).
/// Returns `&mut Self` so it chains inside an existing builder expression.
pub trait NoWindow {
    /// Suppress the transient console window Windows shows when spawning a console
    /// subprocess. No-op on non-Windows.
    fn no_window(&mut self) -> &mut Self;
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

impl NoWindow for std::process::Command {
    fn no_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

impl NoWindow for tokio::process::Command {
    fn no_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            // tokio's Command exposes `creation_flags` inherently on Windows.
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

// ── CLI PATH resolution (macOS/Linux GUI launch) ─────────────────────────────────

/// A `PATH` that augments the process `PATH` with the user's login-shell `PATH`
/// and common CLI install dirs, so a GUI-launched app can still find user-installed
/// CLIs (e.g. `claude`). Returns `None` when no override is needed/available.
///
/// macOS/Linux apps launched from Finder/Dock/a file manager inherit only a minimal
/// `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin`) — **not** the `PATH` from the user's
/// shell profile — so CLIs installed via npm/nvm/Homebrew or the native installer go
/// undetected even though a terminal finds them. Computed once and cached. On Windows
/// this is `None` (GUI apps there already inherit the full `PATH`).
#[cfg(windows)]
pub fn cli_path() -> Option<std::ffi::OsString> {
    None
}

#[cfg(not(windows))]
pub fn cli_path() -> Option<std::ffi::OsString> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Option<std::ffi::OsString>> = OnceLock::new();
    CACHE.get_or_init(build_cli_path).clone()
}

#[cfg(not(windows))]
fn build_cli_path() -> Option<std::ffi::OsString> {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut dirs: Vec<String> = Vec::new();

    // Login-shell PATH first (the user's real environment), then the current
    // process PATH, then common fallback install dirs — de-duplicated, order kept.
    let sources = [
        login_shell_path().unwrap_or_default(),
        std::env::var("PATH").unwrap_or_default(),
        common_bin_dirs().join(":"),
    ];
    for src in sources {
        for dir in src.split(':') {
            if !dir.is_empty() && seen.insert(dir.to_string()) {
                dirs.push(dir.to_string());
            }
        }
    }

    if dirs.is_empty() {
        None
    } else {
        Some(std::ffi::OsString::from(dirs.join(":")))
    }
}

/// Common locations CLIs land in, used as a safety net if the login-shell probe
/// fails. Covers Homebrew (Intel + Apple Silicon), the Claude Code native
/// installer, and the popular per-user toolchains.
#[cfg(not(windows))]
fn common_bin_dirs() -> Vec<String> {
    let mut dirs = vec![
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
    ];
    if let Ok(home) = std::env::var("HOME") {
        for sub in [
            ".claude/local", // Claude Code native installer
            ".local/bin",
            "bin",
            ".npm-global/bin",
            ".bun/bin",
            ".deno/bin",
            ".volta/bin",
        ] {
            dirs.push(format!("{home}/{sub}"));
        }
    }
    dirs
}

/// The `PATH` from the user's login shell, which sources their profile/rc files.
/// Best-effort and time-bounded so a slow shell startup can't wedge detection.
#[cfg(not(windows))]
fn login_shell_path() -> Option<String> {
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    let shell = std::env::var("SHELL").ok()?;
    // `-lic`: login + interactive + command, so `PATH` set in either a profile
    // (`.zprofile`/`.bash_profile`) or an rc file (`.zshrc`/`.bashrc`) is applied.
    // stdin is null so an interactive rc can't block waiting for input.
    let child = Command::new(&shell)
        .args(["-lic", "echo -n \"$PATH\""])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(Duration::from_secs(4)) {
        Ok(Ok(out)) if out.status.success() => {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            (!path.is_empty()).then_some(path)
        }
        _ => None,
    }
}
