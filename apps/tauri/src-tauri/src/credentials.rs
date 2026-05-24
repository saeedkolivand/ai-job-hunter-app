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
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "com.ajh.tauri";

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
        std::fs::create_dir_all(data_dir).ok();
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

    pub fn set(&self, board_id: &str, username: &str, password: &str) -> Result<(), String> {
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
        self.save_meta(meta);
        Ok(())
    }

    pub fn remove(&self, board_id: &str) -> Result<(), String> {
        if let Ok(entry) = Entry::new(SERVICE, board_id) {
            // Ignore "not found" — the metadata may exist without a keychain
            // entry if the keychain was cleared externally.
            entry.delete_credential().ok();
        }
        let mut meta = self.load_meta();
        meta.remove(board_id);
        self.save_meta(meta);
        Ok(())
    }

    /// Returns (username, password) for internal use only (scraper/applier).
    /// Never exposed directly over IPC — the renderer only ever sees metadata.
    pub fn get_decrypted(&self, board_id: &str) -> Option<(String, String)> {
        let meta = self.load_meta();
        let m = meta.get(board_id)?;
        let password = Entry::new(SERVICE, board_id)
            .ok()?
            .get_password()
            .ok()?;
        Some((m.username.clone(), password))
    }

    /// Returns every stored (boardId, username, password) tuple.
    #[allow(dead_code)]
    pub fn get_all_decrypted(&self) -> Vec<(String, String, String)> {
        self.list()
            .into_iter()
            .filter_map(|m| {
                self.get_decrypted(&m.board_id)
                    .map(|(user, pass)| (m.board_id, user, pass))
            })
            .collect()
    }

    // ── Meta persistence ──────────────────────────────────────────────────────

    fn load_meta(&self) -> HashMap<String, CredentialMeta> {
        let mut guard = self.cache.lock().unwrap();
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

    fn save_meta(&self, meta: HashMap<String, CredentialMeta>) {
        if let Ok(json) = serde_json::to_string_pretty(&meta) {
            std::fs::write(&self.meta_file, json).ok();
        }
        self.cache.lock().unwrap().0 = Some(meta);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_available() {
        let temp_dir = TempDir::new().unwrap();
        let store = CredentialStore::new(&temp_dir.path().to_path_buf());
        assert!(store.is_available());
    }

    #[test]
    fn test_list_empty() {
        let temp_dir = TempDir::new().unwrap();
        let store = CredentialStore::new(&temp_dir.path().to_path_buf());
        let creds = store.list();
        assert!(creds.is_empty());
    }

    #[test]
    fn test_credential_meta_serialization() {
        let meta = CredentialMeta {
            board_id: "linkedin".to_string(),
            username: "test@example.com".to_string(),
            saved_at: 1234567890,
        };
        
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: CredentialMeta = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.board_id, "linkedin");
        assert_eq!(deserialized.username, "test@example.com");
        assert_eq!(deserialized.saved_at, 1234567890);
    }

    #[test]
    fn test_now_ms() {
        let ts = now_ms();
        assert!(ts > 0);
        // Should be roughly current time (within 1 second)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!((now as i64 - ts as i64).abs() < 1000);
    }
}
