//! The launch-time pointer file the app writes and this CLI reads back: the
//! data dir, and the pairing token inside it (with the UNC/`..`/relative-path guard). Pure
//! filesystem reads, no socket. Split out of `agent_cli.rs` under R8's LOC cap.

use super::*;
// ── agent-CLI pointer (written by `super::register::write_agent_pointer`) ──

#[derive(Debug, Deserialize)]
pub(super) struct AgentPointer {
    #[serde(rename = "dataDir")]
    pub(super) data_dir: String,
}

/// Read + parse the pointer file, or `None` on any I/O/parse failure. The
/// caller reports this as `app_not_located`, NOT `app_not_running`: the app
/// may well be running and simply not have written a pointer yet (it is
/// written on launch, so a build predating it leaves none), and conflating
/// the two sends anyone debugging this to look at the wrong thing — it did
/// exactly that during this feature's own end-to-end verification.
/// Never logs/echoes the path itself (path privacy).
pub(super) fn read_agent_pointer() -> Option<AgentPointer> {
    let path = crate::platform::config::agent_pointer_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Reject a `dataDir` that is not an absolute LOCAL path (MEDIUM fix —
/// security review). The pointer file is written by the app itself, but this
/// CLI treats its `dataDir` value as arriving from disk and joins it
/// unvalidated — a UNC path (`\\attacker.example.com\share`) turns the
/// read below into an outbound SMB/WebDAV session on Windows, leaking NTLM
/// credentials to whatever host it names: a network primitive smuggled
/// through a file read, on a path `tests/egress.rs`'s allowlist does not
/// cover. Windows treats `/` and `\` interchangeably as path separators, so
/// the two leading bytes are checked as EITHER separator in EITHER
/// combination (`\\`, `//`, `\/`, `/\`) — confirmed against `ntpath` (a
/// faithful model of Windows path parsing) that a mixed-separator UNC root
/// still parses as absolute UNC and, before this fix, passed a check that
/// only matched the two same-separator prefixes literally (a MEDIUM fix,
/// security review round 2 — a straight string-prefix match doesn't survive
/// Windows' separator equivalence). Plain byte checks, so this runs before
/// ANY filesystem call.
fn is_safe_local_data_dir(data_dir: &str) -> bool {
    let bytes = data_dir.as_bytes();
    let is_sep = |b: Option<&u8>| matches!(b, Some(b'\\') | Some(b'/'));
    if is_sep(bytes.first()) && is_sep(bytes.get(1)) {
        return false;
    }
    Path::new(data_dir).is_absolute()
}

/// Read the persisted pairing token from `data_dir` — the exact file
/// [`super::super::persist::persist_token`] writes, read the same way
/// [`super::super::persist::load_or_create_token`] does (trimmed, empty ⇒ absent).
/// `None` (never a filesystem read) for a `dataDir` [`is_safe_local_data_dir`]
/// rejects — the caller reports the same `pairing_token_unavailable` sentinel
/// as any other absent/unreadable token.
pub(super) fn read_pairing_token(data_dir: &str) -> Option<String> {
    if !is_safe_local_data_dir(data_dir) {
        return None;
    }
    let text = std::fs::read_to_string(Path::new(data_dir).join(TOKEN_FILE)).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests;
