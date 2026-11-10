//! Sanitized record of a total bridge bind failure (#1442): when no port in
//! [`super::PORT_RANGE`] binds, the app drops one small file in its data dir that
//! the agent CLI reads to say "bridge could not bind" instead of "app not running".
//! Holds only ports + OS error codes/kinds — never paths or usernames.

use std::io;
use std::path::Path;

use super::{BIND_DIAG_FILE, PORT_RANGE};

/// One port's failure, e.g. `47615: os error 10048 (AddrInUse)`.
pub(super) fn describe(port: u16, e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(code) => format!("{port}: os error {code} ({:?})", e.kind()),
        None => format!("{port}: {:?}", e.kind()),
    }
}

/// Persist the per-port failures (best-effort; a write failure only loses the hint).
pub(super) fn record(dir: &Path, failures: &[String]) {
    let _ = std::fs::write(dir.join(BIND_DIAG_FILE), failures.join("; "));
}

/// Remove the record after a successful bind.
pub(super) fn clear(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(BIND_DIAG_FILE));
}

/// Read the recorded failure (CLI side). Caller must have validated `dir` already
/// (the CLI only gets here after `read_pairing_token` accepted the same dir).
pub(crate) fn read(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(BIND_DIAG_FILE)).ok()?;
    let t: String = text.trim().chars().take(512).collect();
    (!t.is_empty()).then_some(t)
}

/// The CLI-facing message for a recorded failure `detail`.
pub(crate) fn cli_message(detail: &str) -> String {
    format!(
        "the app's local bridge could not bind ports {}-{} at its last launch ({detail}). \
         If the app is running, close whatever holds those ports or reboot, then relaunch the app; \
         if it is not running, just start it.",
        PORT_RANGE.start(),
        PORT_RANGE.end()
    )
}

#[cfg(test)]
mod tests;
