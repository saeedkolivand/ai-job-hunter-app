//! Uniform interface for exporting/restoring a persistent store.
//!
//! Each user-data store implements `DataStore` so the backup commands in
//! `commands/data.rs` can serialize every store into one bundle and restore it
//! with `import`. Import is REPLACE semantics — the store is cleared and
//! repopulated from the bundle, preserving record ids.
//!
//! Secrets (credentials/keychain), ephemeral caches (live postings, company
//! briefs), and the transient job-execution log are intentionally NOT stores
//! here and are excluded from backups.

use serde_json::Value;

use crate::error::AppResult;

pub trait DataStore {
    /// Key under which this store's data lives in the export bundle.
    fn key(&self) -> &'static str;

    /// Serialize all records as a JSON value (array, or object for single-row).
    fn export(&self) -> Value;

    /// Replace the store's contents from previously-exported data.
    /// Returns the number of records restored.
    fn import(&self, data: &Value) -> AppResult<usize>;
}

/// A persistent store that a full factory reset must wipe.
///
/// Distinct from (and a superset of) [`DataStore`]: a factory reset clears the
/// backup-able stores **and** the things excluded from backups — secrets, the
/// ephemeral caches, and the job-execution log. Implemented per managed wrapper
/// type and driven by a registry (see `commands/privacy.rs`), so a new store is
/// wiped on reset by registering it once — never by editing the reset command.
///
/// Pure by design (no Tauri here) so it can live in this shared-infra layer; the
/// `AppHandle`-resolving registry that calls `reset` lives in the shell layer.
pub trait Resettable {
    /// Clear all of the store's persisted contents.
    fn reset(&self);
}
