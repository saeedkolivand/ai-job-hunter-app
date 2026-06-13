use parking_lot::Mutex;
/// OS-native credential store for the Tauri shell.
///
/// Passwords are stored in the OS secret service — the same underlying
/// primitives Electron's `safeStorage` uses:
///   Windows  → DPAPI  (Credential Manager)
///   macOS    → Keychain
///   Linux    → Secret Service API (libsecret / gnome-keyring / kwallet)
///
/// ── Layout ───────────────────────────────────────────────────────────────────
/// Passwords  → OS keychain, service = "com.ajh.tauri", username = boardId
/// Metadata   → <dataDir>/credential-meta.json
///             { [boardId]: { board_id, username, saved_at } }
///
/// The renderer only ever receives metadata — passwords stay in Rust and are
/// looked up by scrapers/appliers via `get_decrypted(board_id)`.
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use keyring_core::Entry;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const SERVICE: &str = "com.ajh.tauri";

/// Initialise the OS-native keyring backend. Must be called once before any
/// `Entry` operations. Returns [`AppError::Storage`] if the platform secret
/// store cannot be opened, so the caller can degrade gracefully instead of
/// aborting startup.
pub fn init_keyring() -> AppResult<()> {
    use std::collections::HashMap;
    let cfg = HashMap::new();
    #[cfg(target_os = "windows")]
    {
        use windows_native_keyring_store::Store;
        let store = Store::new_with_configuration(&cfg).map_err(|e| {
            AppError::Storage(format!("Windows Credential Manager unavailable: {e}"))
        })?;
        keyring_core::set_default_store(store);
    }
    #[cfg(target_os = "macos")]
    {
        use apple_native_keyring_store::keychain::Store;
        let store = Store::new_with_configuration(&cfg)
            .map_err(|e| AppError::Storage(format!("macOS Keychain unavailable: {e}")))?;
        keyring_core::set_default_store(store);
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        use dbus_secret_service_keyring_store::Store;
        let store = Store::new_with_configuration(&cfg)
            .map_err(|e| AppError::Storage(format!("Secret Service unavailable: {e}")))?;
        keyring_core::set_default_store(store);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialMeta {
    pub board_id: String,
    pub username: String,
    pub saved_at: u64,
}

#[derive(Default)]
struct MetaCache(Option<HashMap<String, CredentialMeta>>);

pub struct CredentialStore {
    meta_file: PathBuf,
    cache: Mutex<MetaCache>,
}

impl CredentialStore {
    pub fn new(data_dir: &PathBuf) -> Self {
        if let Err(e) = std::fs::create_dir_all(data_dir) {
            log::warn!("[credentials] failed to create data dir (metadata writes may fail): {e}");
        }
        Self {
            meta_file: data_dir.join("credential-meta.json"),
            cache: Mutex::new(MetaCache::default()),
        }
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// True on all supported platforms. Mirrors Electron's
    /// `safeStorage.isEncryptionAvailable()`.
    pub fn is_available(&self) -> bool {
        true
    }

    pub fn list(&self) -> Vec<CredentialMeta> {
        let meta = self.load_meta();
        meta.values().cloned().collect()
    }

    pub fn set(&self, board_id: &str, username: &str, password: &str) -> AppResult<()> {
        Entry::new(SERVICE, board_id)
            .map_err(|e| format!("keyring entry error: {e}"))?
            .set_password(password)
            .map_err(|e| format!("keyring set error: {e}"))?;

        let mut meta = self.load_meta();
        meta.insert(
            board_id.to_string(),
            CredentialMeta {
                board_id: board_id.to_string(),
                username: username.to_string(),
                saved_at: now_ms(),
            },
        );
        self.save_meta(meta)?;
        Ok(())
    }

    pub fn remove(&self, board_id: &str) -> AppResult<()> {
        if let Ok(entry) = Entry::new(SERVICE, board_id) {
            // Ignore "not found" — the metadata may exist without a keychain
            // entry if the keychain was cleared externally.
            entry.delete_credential().ok();
        }
        let mut meta = self.load_meta();
        meta.remove(board_id);
        self.save_meta(meta)?;
        Ok(())
    }

    /// Remove every stored credential — board passwords and all AI/provider keys
    /// (including the Ollama account key). Driven off the metadata index, so it
    /// needs no hardcoded provider list. Used by the factory reset.
    pub fn clear_all(&self) -> AppResult<()> {
        for meta in self.list() {
            self.remove(&meta.board_id)?;
        }
        Ok(())
    }

    /// Returns (username, password) for internal use only (scraper/applier).
    /// Never exposed directly over IPC — the renderer only ever sees metadata.
    pub fn get_decrypted(&self, board_id: &str) -> Option<(String, String)> {
        let meta = self.load_meta();
        let m = meta.get(board_id)?;
        let password = Entry::new(SERVICE, board_id).ok()?.get_password().ok()?;
        Some((m.username.clone(), password))
    }

    // ── Meta persistence ──────────────────────────────────────────────────────

    fn load_meta(&self) -> HashMap<String, CredentialMeta> {
        let mut guard = self.cache.lock();
        if let Some(ref c) = guard.0 {
            return c.clone();
        }
        let loaded: HashMap<String, CredentialMeta> = std::fs::read_to_string(&self.meta_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        guard.0 = Some(loaded.clone());
        loaded
    }

    fn save_meta(&self, meta: HashMap<String, CredentialMeta>) -> AppResult<()> {
        let json = serde_json::to_string_pretty(&meta)
            .map_err(|e| AppError::Parse(format!("serialize credential metadata: {e}")))?;
        std::fs::write(&self.meta_file, json)
            .map_err(|e| AppError::Storage(format!("write credential metadata: {e}")))?;
        self.cache.lock().0 = Some(meta);
        Ok(())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod test;
