//! Binary detection: the base `Command` every CLI agent spawns through
//! (Windows `.cmd`-shim resolution, augmented `PATH`), and the cached
//! `<binary> --version` probe that backs the `system_health` poll.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::process::Command;

use crate::platform::NoWindow;

/// How long a `<binary> --version` result is trusted before re-probing. Install
/// status changes rarely, so [`detect_cached`] collapses the 5 s health poll from
/// a subprocess spawn per tick to at most one per binary per TTL.
const DETECT_TTL: Duration = Duration::from_secs(300);

pub(super) struct Detected {
    pub(super) ok: bool,
    pub(super) version: Option<String>,
    pub(super) at: Instant,
}

pub(super) fn detect_cache() -> &'static Mutex<HashMap<String, Detected>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Detected>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Base command for a CLI agent, with `args` applied: console window hidden on
/// Windows, and an augmented `PATH` so a GUI-launched macOS/Linux app can find the
/// binary (Finder/Dock apps inherit only a minimal `PATH`, missing
/// npm/nvm/Homebrew/native installs).
///
/// On **Windows** it first resolves the binary on `PATH` × `PATHEXT`
/// ([`crate::platform::resolve_cli_binary`]): a `.cmd`/`.bat` shim (how npm-global
/// CLIs like `gemini`/`codex`/`agy` install — there is no `.exe`) is launched
/// through `cmd.exe /C`, since `CreateProcess` cannot execute a batch file
/// directly. Each element of `args` is passed as a **separate argv entry** — never
/// concatenated into a shell string — so `cmd.exe` performs no word-splitting. If
/// resolution fails we fall through to a bare spawn so the OS still surfaces
/// `NotFound`.
///
/// **CVE-2024-24576 invariant:** because the program spawned is `cmd.exe` (not the
/// `.cmd`), Rust's batch-argument escaping does NOT engage — so `args` here MUST
/// carry only fixed, trusted flags (model/sandbox flags, `--version`), never
/// untrusted prompt/JD text. That text goes on stdin via
/// [`super::PromptDelivery::Stdin`]; [`super::spawn`] only appends to argv for the
/// unused [`super::PromptDelivery::Arg`], which no backend constructs. See the
/// [`super::PromptDelivery`] type docs.
pub(super) fn cli_command(binary: &str, args: &[String]) -> Command {
    #[cfg(windows)]
    if let Some(resolved) = crate::platform::resolve_cli_binary(binary) {
        let mut cmd = if resolved.needs_cmd_wrapper {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(&resolved.path).args(args);
            c
        } else {
            let mut c = Command::new(&resolved.path);
            c.args(args);
            c
        };
        cmd.no_window();
        return cmd;
    }

    let mut cmd = Command::new(binary);
    cmd.args(args);
    cmd.no_window();
    if let Some(path) = crate::platform::cli_path() {
        cmd.env("PATH", path);
    }
    cmd
}

/// Whether the binary is installed (`<binary> --version` succeeds), plus its
/// reported version. Mirrors `ollama::reachable_model()` as the health signal.
pub async fn detect(binary: &str) -> (bool, Option<String>) {
    let fut = cli_command(binary, &["--version".to_string()]).output();
    match tokio::time::timeout(Duration::from_secs(5), fut).await {
        Ok(Ok(out)) if out.status.success() => {
            let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
            (true, (!v.is_empty()).then_some(v))
        }
        _ => (false, None),
    }
}

/// Cached [`detect`]: re-probes a binary at most once per [`DETECT_TTL`], so the
/// recurring `system_health` poll stops spawning a subprocess every few seconds.
/// The lock is released before the `.await`, never held across it.
pub async fn detect_cached(binary: &str) -> (bool, Option<String>) {
    {
        let cache = detect_cache().lock();
        if let Some(d) = cache.get(binary) {
            if d.at.elapsed() < DETECT_TTL {
                return (d.ok, d.version.clone());
            }
        }
    }
    let (ok, version) = detect(binary).await;
    detect_cache().lock().insert(
        binary.to_string(),
        Detected {
            ok,
            version: version.clone(),
            at: Instant::now(),
        },
    );
    (ok, version)
}

/// Drop all cached detection results so the next [`detect_cached`] re-probes.
/// Called right after an in-app install (#22) so freshly-installed agents show as
/// available immediately instead of after the [`DETECT_TTL`].
pub fn clear_detect_cache() {
    detect_cache().lock().clear();
}
