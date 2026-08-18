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

pub(super) fn persist_token(data_dir: &Path, token: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(data_dir.join(TOKEN_FILE), token)?;
    // Restrict the token file to the owner (best-effort) on unix so a
    // multi-user box can't read the pairing secret. Applies on both
    // first-create and rotate.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(
            data_dir.join(TOKEN_FILE),
            std::fs::Permissions::from_mode(0o600),
        );
    }
    Ok(())
}
