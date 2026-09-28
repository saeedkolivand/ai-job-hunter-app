//! macOS/Linux CLI `PATH` resolution.
//!
//! Apps launched from Finder/Dock/a file manager inherit only a minimal `PATH`
//! (`/usr/bin:/bin:/usr/sbin:/sbin`) — **not** the `PATH` from the user's shell
//! profile — so CLIs installed via npm/nvm/Homebrew or a native installer go
//! undetected even though a terminal finds them. [`cli_path`] augments the
//! process `PATH` with a login-shell probe plus common install dirs.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

#[cfg(test)]
mod tests;

/// How long a failed/timed-out login-shell probe is trusted before retrying. A
/// *successful* probe has no TTL — it stays cached until [`reset_cli_path_cache`]
/// (an install/"Re-check" invalidates it explicitly, not on a timer). Short
/// enough that a machine with a permanently stuck login shell (e.g. a broken rc
/// file) still recovers on its own; long enough that it isn't re-probed — up to
/// its 4 s timeout — on every single CLI spawn.
const FAILED_PROBE_TTL: Duration = Duration::from_secs(60);

/// A memoized [`build_cli_path`] result.
struct CliPathCache {
    path: Option<OsString>,
    /// Whether the login-shell probe itself succeeded. A failed/timed-out probe
    /// is cached too (see [`FAILED_PROBE_TTL`]), just not indefinitely — past
    /// its TTL, [`cli_path`] retries instead of wedging "not detected" for the
    /// rest of the process (a slow rc file, e.g. oh-my-zsh/nvm, costs at most
    /// one probe per TTL window, not every probe).
    login_shell_ok: bool,
    /// When this entry was built — only consulted for a failed probe.
    at: Instant,
}

fn cache() -> &'static Mutex<Option<CliPathCache>> {
    static CACHE: OnceLock<Mutex<Option<CliPathCache>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// A `PATH` that augments the process `PATH` with the user's login-shell `PATH`
/// and common CLI install dirs, so a GUI-launched app can still find
/// user-installed CLIs (e.g. `claude`). `None` when no override is
/// needed/available. Memoized: a success is cached indefinitely, a failure only
/// for [`FAILED_PROBE_TTL`]; see [`reset_cli_path_cache`].
pub fn cli_path() -> Option<OsString> {
    {
        let cache = cache().lock();
        if let Some(entry) = cache.as_ref() {
            let usable = entry.login_shell_ok || entry.at.elapsed() < FAILED_PROBE_TTL;
            if usable {
                return entry.path.clone();
            }
        }
    }
    let (path, login_shell_ok) = build_cli_path();
    *cache().lock() = Some(CliPathCache {
        path: path.clone(),
        login_shell_ok,
        at: Instant::now(),
    });
    path
}

/// Force the next [`cli_path`] call to re-probe from scratch, including a login
/// shell that previously timed out. `cli_agents_redetect`'s "Re-check" calls
/// this so a slow-starting rc file can recover without an app restart.
pub fn reset_cli_path_cache() {
    *cache().lock() = None;
}

fn build_cli_path() -> (Option<OsString>, bool) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut dirs: Vec<String> = Vec::new();

    let login_shell = login_shell_path();
    let login_shell_ok = login_shell.is_some();

    // Login-shell PATH first (the user's real environment), then the current
    // process PATH, then common fallback install dirs — de-duplicated, order kept.
    let sources = [
        login_shell.unwrap_or_default(),
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

    let path = (!dirs.is_empty()).then(|| OsString::from(dirs.join(":")));
    (path, login_shell_ok)
}

/// Common locations CLIs land in, used as a safety net if the login-shell probe
/// fails. Covers Homebrew (Intel/Apple Silicon/Linuxbrew), Snap, the popular
/// per-user Node version managers (nvm/fnm/asdf/mise), pnpm's global bin, and
/// the Claude Code native installer.
fn common_bin_dirs() -> Vec<String> {
    let mut dirs = vec![
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        "/home/linuxbrew/.linuxbrew/bin".to_string(),
        "/snap/bin".to_string(),
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
            ".asdf/shims",                          // asdf
            ".local/share/mise/shims",              // mise
            ".fnm/aliases/default/bin",             // fnm (curl installer)
            ".local/share/fnm/aliases/default/bin", // fnm (XDG/package-manager installer)
            "Library/pnpm",                         // pnpm global bin (macOS)
            ".local/share/pnpm",                    // pnpm global bin (Linux)
        ] {
            dirs.push(format!("{home}/{sub}"));
        }
        dirs.extend(nvm_bin_dirs(&Path::new(&home).join(".nvm/versions/node")));
    }
    dirs
}

/// Every installed nvm Node version's `bin` dir (`~/.nvm/versions/node/*/bin`).
/// nvm has no fixed "current" path outside a sourced shell function, so every
/// installed version is offered as a fallback. `nvm_node_root` is injected so
/// this is unit-testable against a temp dir standing in for
/// `~/.nvm/versions/node`.
fn nvm_bin_dirs(nvm_node_root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(nvm_node_root) else {
        return Vec::new();
    };
    let mut dirs: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.path().join("bin").to_string_lossy().into_owned())
        .collect();
    dirs.sort();
    dirs
}

/// The `PATH` from the user's login shell, which sources their profile/rc files.
/// Best-effort and time-bounded so a slow shell startup can't wedge detection.
fn login_shell_path() -> Option<String> {
    let shell = std::env::var("SHELL").ok()?;
    // `-lic`: login + interactive + command, so `PATH` set in either a profile
    // (`.zprofile`/`.bash_profile`) or an rc file (`.zshrc`/`.bashrc`) is applied.
    // `printenv PATH` (not `echo "$PATH"`) reads the resulting process
    // environment directly: every shell exports it colon-joined the same way,
    // including fish, whose own `$PATH` variable expansion joins with spaces
    // instead. stdin is null so an interactive rc can't block waiting for input.
    let child = Command::new(&shell)
        .args(["-lic", "printenv PATH"])
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
        // An interactive rc can print a banner/motd to stdout ahead of the
        // command's own output — take the last non-empty line so a banner never
        // gets glued onto PATH.
        Ok(Ok(out)) if out.status.success() => {
            last_non_empty_line(&String::from_utf8_lossy(&out.stdout))
        }
        _ => None,
    }
}

/// The last non-empty, trimmed line of `s`.
fn last_non_empty_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map(str::to_string)
}
