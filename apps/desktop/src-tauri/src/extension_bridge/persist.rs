//! Pairing-token + consent-opt-in persistence — split out of `mod.rs` to stay
//! under the R8 hard LOC cap (`tests/architecture.rs`), the same discipline
//! `autotrack.rs`'s split already documents. Holds the on-disk read/write for
//! the token file and the two boolean opt-in files; the `BridgeState`
//! accessors, the `Resettable` wiring, and each mutator's own failure log
//! stay in the parent module — this is the pure fs layer underneath them.

use std::path::Path;

use serde_json::{json, Value};

use crate::observability::sanitize_reason;
use crate::platform::fs::write_atomic;

use super::{AI_ASSIST_OPTIN_FILE, AUTOFILL_OPTIN_FILE, TOKEN_FILE};

/// A 32-byte random token, lowercase hex (64 chars).
pub(super) fn new_token() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Read the persisted token, or create + persist a fresh one on first run (or
/// if the stored value is corrupt/empty).
pub(super) fn load_or_create_token(data_dir: &Path) -> String {
    let path = data_dir.join(TOKEN_FILE);
    if let Ok(s) = std::fs::read_to_string(&path) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let fresh = new_token();
    if let Err(e) = persist_token(data_dir, &fresh) {
        let reason = sanitize_reason(&e.to_string());
        log::warn!("[extension_bridge] failed to persist initial token (non-fatal): {reason}");
    }
    fresh
}

/// Read the persisted autofill opt-in (`"1"` ⇒ on). Absent / any other value ⇒
/// OFF, so a first run and a corrupt flag both default to the safe (off) state.
pub(super) fn load_autofill_optin(data_dir: &Path) -> bool {
    std::fs::read_to_string(data_dir.join(AUTOFILL_OPTIN_FILE))
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

pub(super) fn persist_autofill_optin(data_dir: &Path, enabled: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    write_atomic(
        &data_dir.join(AUTOFILL_OPTIN_FILE),
        if enabled { b"1" } else { b"0" },
    )
}

/// Read the persisted AI-answer-assist opt-in. Absent file / parse failure →
/// OFF (the safe state), mirroring [`load_autofill_optin`]'s degrade-to-off
/// discipline. Only the `enabled` flag is honored: an OLD file that also
/// carried a `provider`/`model`/`base_url` snapshot is still read fine — the
/// extra fields are ignored, so a user who opted in before task #16 stays
/// opted in (the active provider is resolved from the backend
/// [`crate::ai_config::AiConfigStore`] at answer-time, never that stale
/// snapshot).
pub(super) fn load_ai_assist_optin(data_dir: &Path) -> bool {
    std::fs::read_to_string(data_dir.join(AI_ASSIST_OPTIN_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("enabled").and_then(Value::as_bool))
        .unwrap_or(false)
}

pub(super) fn persist_ai_assist_optin(data_dir: &Path, enabled: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let json = json!({ "enabled": enabled }).to_string();
    write_atomic(&data_dir.join(AI_ASSIST_OPTIN_FILE), json.as_bytes())
}

/// Persist the pairing token to `data_dir` — the single write path both
/// first-create ([`load_or_create_token`]) and rotation
/// ([`super::BridgeState::regenerate_token`]) share, so a permissions fix
/// here covers both.
///
/// On unix the file is opened with `0o600` baked into the *creation* syscall
/// itself (`OpenOptions::mode`), not applied afterward: the mode is part of
/// the same `open(2)` call that creates the inode, so a brand-new token file
/// is owner-only from its very first byte on disk — there is no window where
/// it briefly exists at the process umask (which is what a separate
/// `fs::write` then `set_permissions` call leaves open, and on a multi-user
/// box the umask can be group- or world-readable).
///
/// `OpenOptions::mode` only takes effect when the file is actually CREATED —
/// unix `open(2)` ignores the mode argument for a file that already exists
/// (only `O_TRUNC` applies). So an already-existing file with the wrong
/// permissions (e.g. one written before this fix shipped) would keep them
/// across the `truncate(true)` open; the explicit trailing
/// `set_permissions` call below corrects that case too, on every write —
/// first-create AND rotate self-heal a stale wrong-permission file, not just
/// a fresh one.
pub(super) fn persist_token(data_dir: &Path, token: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join(TOKEN_FILE);

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(token.as_bytes())?;
        // Re-assert 0o600 even on the just-created-correctly path (a no-op
        // there) so the ONE call also corrects a pre-existing file the
        // create-mode couldn't touch (see the doc above). A box where this
        // fails silently would keep a readable pairing secret — that is a
        // real credential leak, not a cosmetic failure, so it is logged loud
        // and propagated rather than swallowed with `let _ =`.
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = file.set_permissions(std::fs::Permissions::from_mode(0o600)) {
            let reason = sanitize_reason(&e.to_string());
            log::warn!(
                "[extension_bridge] pairing token file could not be locked down to \
                 owner-only (0o600): {reason}"
            );
            return Err(e);
        }
    }

    #[cfg(not(unix))]
    {
        write_atomic(&path, token.as_bytes())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
