//! Sanitized record of a total bridge bind failure (#1442): when no port in
//! [`super::PORT_RANGE`] binds, the app drops one small file in its data dir that
//! the agent CLI reads to say "bridge could not bind" instead of "app not running".
//! Holds only ports + OS error codes/kinds — never paths or usernames.

use std::io;
use std::path::Path;

use super::BIND_DIAG_FILE;

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

const MAX_READ: u64 = 1024;

/// Read the recorded failure (CLI side). `dir` must already be validated (the CLI only gets
/// here after `read_pairing_token` accepted it). Regular file only (no symlink/FIFO), at most
/// [`MAX_READ`] bytes, and the content must match [`record`]'s own grammar, else `None`:
/// the file is never echoed unvalidated.
pub(super) fn read(dir: &Path) -> Option<String> {
    use std::io::Read;
    let path = dir.join(BIND_DIAG_FILE);
    if !std::fs::symlink_metadata(&path).ok()?.is_file() {
        return None;
    }
    let mut buf = Vec::new();
    std::fs::File::open(&path)
        .ok()?
        .take(MAX_READ)
        .read_to_end(&mut buf)
        .ok()?;
    let text = String::from_utf8_lossy(&buf).trim().to_string();
    (buf.len() < MAX_READ as usize && matches_grammar(&text)).then_some(text)
}

/// `"; "`-separated items, each `<1-5 digits>: os error <digits> (<word>)` or `<digits>: <word>`.
fn matches_grammar(text: &str) -> bool {
    let word = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    let digits =
        |d: &str, max: usize| (1..=max).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit());
    !text.is_empty()
        && text.split("; ").all(|item| {
            let Some((port, rest)) = item.split_once(": ") else {
                return false;
            };
            digits(port, 5)
                && match rest.strip_prefix("os error ") {
                    Some(r) => r
                        .split_once(" (")
                        .and_then(|(code, k)| k.strip_suffix(')').map(|k| (code, k)))
                        .is_some_and(|(code, k)| digits(code, 10) && word(k)),
                    None => word(rest),
                }
        })
}

#[cfg(test)]
mod tests;
