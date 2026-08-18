//! Pairing-token + consent-opt-in persistence — split out of `mod.rs` to stay
//! under the R8 hard LOC cap (`tests/architecture.rs`), the same discipline
//! `autotrack.rs`'s split already documents. Holds the on-disk read/write for
//! the token file and the two boolean opt-in files; the `BridgeState`
//! accessors, the `Resettable` wiring, and each mutator's own failure log
//! stay in the parent module — this is the pure fs layer underneath them.

use std::path::Path;

use serde_json::{json, Value};

use crate::observability::sanitize_reason;

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
    std::fs::write(
        data_dir.join(AUTOFILL_OPTIN_FILE),
        if enabled { "1" } else { "0" },
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
    std::fs::write(
        data_dir.join(AI_ASSIST_OPTIN_FILE),
        json!({ "enabled": enabled }).to_string(),
    )
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
        std::fs::write(&path, token)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── Mutation-check note ──────────────────────────────────────────────
    // The two `#[cfg(unix)]` tests below were mutation-checked in a
    // standalone repro against the PRE-FIX shape (`std::fs::write` then a
    // best-effort `set_permissions` afterward, `let _ =`-swallowed): BOTH
    // still PASS against that old code. Its final on-disk state — including
    // the pre-existing-file self-heal — is identical to the fix's, because
    // the old code's `set_permissions` call, though its failure was
    // discarded, still ran and still succeeded on every ordinary run. A
    // concurrent-poller race test (permissive umask, a 2,000,000-iteration
    // busy-poll thread racing the write, 4 independent runs) also never
    // observed the old code's create-then-chmod window even once — it is
    // real (two separate syscalls) but far too short (no I/O wait between
    // them) for a black-box test to reliably land inside. So: these tests
    // verify END-STATE correctness (and guard a future refactor that drops
    // the trailing `set_permissions` call entirely, which WOULD fail them)
    // — they do NOT, and structurally cannot, prove the TOCTOU window is
    // closed. That property is undetectable by test; it is verified by
    // reading `persist_token`'s doc comment above (the mode is now part of
    // the `open(2)` call itself, not a later chmod).

    #[test]
    fn persist_token_writes_readable_content() {
        let dir = TempDir::new().unwrap();
        persist_token(dir.path(), "abc123").unwrap();
        let read = std::fs::read_to_string(dir.path().join(TOKEN_FILE)).unwrap();
        assert_eq!(read, "abc123");
    }

    #[cfg(unix)]
    #[test]
    fn persist_token_creates_file_owner_only_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        persist_token(dir.path(), "abc123").unwrap();
        let mode = std::fs::metadata(dir.path().join(TOKEN_FILE))
            .unwrap()
            .permissions()
            .mode();
        // Mask off the file-type bits `st_mode` packs alongside the
        // permission bits — only the permission bits are under test.
        assert_eq!(
            mode & 0o777,
            0o600,
            "pairing token file must be owner-only (0o600), got {:o}",
            mode & 0o777
        );
    }

    #[cfg(unix)]
    #[test]
    fn persist_token_corrects_a_preexisting_wrong_permission_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join(TOKEN_FILE);
        // Simulate a token file that predates this fix — or was created on a
        // filesystem/umask combination that widened it — group- and
        // world-readable.
        std::fs::write(&path, "stale").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        persist_token(dir.path(), "fresh").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "a pre-existing wide-permission token file must self-heal on the next persist"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh");
    }
}
