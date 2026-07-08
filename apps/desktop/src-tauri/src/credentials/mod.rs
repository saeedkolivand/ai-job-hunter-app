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
use std::sync::OnceLock;

use keyring_core::Entry;
use serde::{Deserialize, Serialize};

use crate::db::now_ms;
use crate::error::{AppError, AppResult};

pub(crate) const SERVICE: &str = "com.ajh.tauri";

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

    /// Whether the OS secret store is actually reachable — mirrors Electron's
    /// `safeStorage.isEncryptionAvailable()`. Real, READ-ONLY probe (see
    /// [`probe_keyring_available`]): true on Windows/macOS and any Linux with a
    /// working Secret Service; false on a headless box with no secure storage, so
    /// the renderer's "no OS encryption" warning finally fires.
    ///
    /// The outcome is memoized in a process-lifetime `OnceLock`: the backend is
    /// probed exactly once and the result is cached for the life of the process —
    /// it will NOT re-probe if the OS secret backend goes up or down mid-session
    /// (process-sticky by design).
    pub fn is_available(&self) -> bool {
        static AVAILABLE: OnceLock<bool> = OnceLock::new();
        *AVAILABLE.get_or_init(probe_keyring_available)
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

/// Well-known slot read (never written) by [`probe_keyring_available`]. Because
/// the probe is read-only, on a working backend this always reads back as
/// `NoEntry`.
const AVAILABILITY_PROBE_SLOT: &str = "__ajh_keyring_availability_probe__";

/// Read-only probe of the OS keyring backend, backing
/// [`CredentialStore::is_available`]. READS a well-known sentinel slot and
/// interprets the `keyring-core` v1 outcome:
/// - `Ok(_)` or `Err(Error::NoEntry)` → the backend is reachable (the entry is
///   simply absent) → available.
/// - any other `Err` (`NoDefaultStore` from `Entry::new`, `NoStorageAccess` /
///   `PlatformFailure` from a locked store, `NotSupportedByStore` on a headless
///   Linux box with no Secret Service, …) → the backend can't be reached →
///   unavailable.
///
/// Never writes a value, so it cannot trigger a macOS keychain-access prompt or
/// leave a stray entry behind. Only `Error::NoEntry` counts as "backend works
/// but empty"; every other variant is treated as unavailable.
fn probe_keyring_available() -> bool {
    // `Entry::new` returns `Err(Error::NoDefaultStore)` when the keyring was
    // never initialized (e.g. `init_keyring` failed) → backend unavailable.
    let Ok(entry) = Entry::new(SERVICE, AVAILABILITY_PROBE_SLOT) else {
        return false;
    };
    match entry.get_password() {
        // Present, or absent (`NoEntry`) — either way the backend is reachable.
        Ok(_) | Err(keyring_core::Error::NoEntry) => true,
        // Any other error means the backend/platform can't be reached.
        Err(_) => false,
    }
}

/// Read a single credential directly from the OS keychain by its full `slot`
/// name (e.g. `"ai:adzuna-app-key"`), without needing an `AppHandle` or the
/// metadata cache.
///
/// Returns:
/// - `Ok(Some(value))` — credential found and non-empty.
/// - `Ok(None)` — credential not set or stored empty (`NoEntry`).
/// - `Err(AppError::Storage)` — OS keyring is unavailable (locked store,
///   permission denied, etc.); this is a genuine fault, NOT "key absent".
///
/// Caller policy for the `Err` case:
/// - **Critical keys** (a credential the operation cannot proceed without)
///   should surface the `Err` so the user learns the keyring is broken, rather
///   than silently behaving as if the key were unset.
/// - **Optional keys** (e.g. the aggregator's third-party Adzuna / JSearch API
///   keys) may degrade gracefully: log the error and treat the key as absent
///   (`None`). Never crash a user-triggered action over a missing or unreadable
///   optional key — the board simply yields keyless-empty results.
///
/// Workers (scrapers, autopilot tasks) that have no `AppHandle` use this
/// instead of `get_provider_key`; it reuses the same keyring backend
/// initialized at startup by [`init_keyring`].
pub fn read_credential(slot: &str) -> AppResult<Option<String>> {
    let entry = Entry::new(SERVICE, slot)
        .map_err(|e| AppError::Storage(format!("keyring entry error for {slot}: {e}")))?;
    match entry.get_password() {
        Ok(pw) => Ok(if pw.is_empty() { None } else { Some(pw) }),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Storage(format!(
            "keyring read error for {slot}: {e}"
        ))),
    }
}

/// Test-only keyring harness shared across the workspace's unit tests.
///
/// `keyring_core::set_default_store` installs a PROCESS-GLOBAL store, and Cargo
/// runs the lib test binary multi-threaded. To avoid one test replacing the
/// store out from under another (which would orphan already-created mock
/// `Cred`s), every test that needs the keyring goes through this single `Once`,
/// installing keyring-core's in-memory `mock::Store` exactly once for the whole
/// binary. Tests then isolate themselves by using unique slot names rather than
/// by swapping stores. Lives here (not in `test.rs`) so sibling modules like the
/// aggregator can share the same install path.
#[cfg(test)]
pub(crate) fn install_mock_keyring() {
    use std::sync::Once;
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
    });
}

#[cfg(test)]
mod test;
