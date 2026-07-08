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

// ── Windows binary resolution (PATH × PATHEXT, `.cmd`/`.bat` shims) ────────────────
//
// npm-global CLIs (`gemini`, `codex`, `agy`, …) install on Windows as **`.cmd`
// shims** (e.g. `…\npm\gemini.cmd`) with no `.exe`. `CreateProcess` — and thus a
// bare `Command::new("gemini")` — only launches `.com`/`.exe`; it does not consult
// `PATHEXT`, so the shim is reported "not found" for both detection and spawn.
// This resolver reproduces the shell's own lookup (search each `PATH` dir for the
// name plus each `PATHEXT` extension) and reports whether the hit is a batch shim
// that must be run through `cmd.exe`. It lives here so the "env access only in
// `platform/**`" rule holds (PATH/PATHEXT are the only env reads).

/// A CLI binary resolved on Windows: the concrete path found on `PATH`, and
/// whether it is a `.cmd`/`.bat` shim that must be launched through `cmd.exe`.
#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCli {
    /// Concrete resolved path, e.g. `…\npm\gemini.cmd` or `…\claude.exe`.
    pub path: std::path::PathBuf,
    /// `true` for a `.cmd`/`.bat` shim → launch via `cmd.exe /C <path> <args…>`.
    pub needs_cmd_wrapper: bool,
}

/// The Windows default when `PATHEXT` is unset.
#[cfg(windows)]
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD;.VBS;.JS;.WSF";

/// Resolve `binary` to a concrete path on Windows, searching `PATH` × `PATHEXT`
/// the way the shell does. `None` when nothing matches, so callers fall back to a
/// bare spawn and let the OS surface `NotFound`.
#[cfg(windows)]
pub fn resolve_cli_binary(binary: &str) -> Option<ResolvedCli> {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_PATHEXT.to_string());
    resolve_cli_binary_in(binary, path_var.as_os_str(), &pathext)
}

/// Pure core of [`resolve_cli_binary`] — `PATH`/`PATHEXT` are injected so it is
/// unit-testable with a fake directory and no process-env mutation.
#[cfg(windows)]
fn resolve_cli_binary_in(
    binary: &str,
    path_var: &std::ffi::OsStr,
    pathext: &str,
) -> Option<ResolvedCli> {
    use std::path::Path;

    // An already-qualified path (a `<AGENT>_BIN` override like `C:\tools\gemini.cmd`,
    // or anything containing a separator) is honoured directly if it exists.
    let raw = Path::new(binary);
    if raw.is_absolute() || raw.components().count() > 1 {
        return raw.is_file().then(|| resolved(raw.to_path_buf()));
    }

    // Try the bare name first (covers a name that already carries an extension),
    // then the name + each `PATHEXT` entry.
    let exts = std::iter::once(String::new()).chain(
        pathext
            .split(';')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .map(str::to_string),
    );
    let exts: Vec<String> = exts.collect();

    for dir in std::env::split_paths(path_var) {
        for ext in &exts {
            let candidate = dir.join(format!("{binary}{ext}"));
            if candidate.is_file() {
                return Some(resolved(candidate));
            }
        }
    }
    None
}

/// Tag a resolved path with whether it is a batch shim (`.cmd`/`.bat`).
#[cfg(windows)]
fn resolved(path: std::path::PathBuf) -> ResolvedCli {
    let needs_cmd_wrapper = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
        .unwrap_or(false);
    ResolvedCli {
        path,
        needs_cmd_wrapper,
    }
}

#[cfg(all(test, windows))]
mod windows_resolver_tests {
    use super::*;
    use std::fs;

    // Hermetic: a fake PATH dir + injected PATHEXT, so no process-env mutation and
    // no assumption about which binaries exist on the runner.

    #[test]
    fn resolves_cmd_shim_and_flags_the_wrapper() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("foo.cmd"), "@echo off\r\n").unwrap();
        let path_var = std::ffi::OsString::from(dir.path());

        let r = resolve_cli_binary_in("foo", &path_var, ".COM;.EXE;.BAT;.CMD").unwrap();
        assert!(r.needs_cmd_wrapper, ".cmd shim must be flagged for cmd.exe");
        assert_eq!(
            r.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase(),
            "foo.cmd"
        );
    }

    #[test]
    fn resolves_exe_directly_without_wrapper() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("bar.exe"), b"MZ").unwrap();
        let path_var = std::ffi::OsString::from(dir.path());

        let r = resolve_cli_binary_in("bar", &path_var, ".COM;.EXE;.BAT;.CMD").unwrap();
        assert!(!r.needs_cmd_wrapper, ".exe is launched directly");
        assert_eq!(
            r.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase(),
            "bar.exe"
        );
    }

    #[test]
    fn missing_binary_resolves_to_none() {
        let dir = tempfile::tempdir().unwrap();
        let path_var = std::ffi::OsString::from(dir.path());
        assert!(resolve_cli_binary_in("nope", &path_var, ".COM;.EXE;.BAT;.CMD").is_none());
    }
}
