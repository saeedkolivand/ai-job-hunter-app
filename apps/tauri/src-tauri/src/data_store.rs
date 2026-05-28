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

pub trait DataStore {
    /// Key under which this store's data lives in the export bundle.
    fn key(&self) -> &'static str;

    /// Serialize all records as a JSON value (array, or object for single-row).
    fn export(&self) -> Value;

    /// Replace the store's contents from previously-exported data.
    /// Returns the number of records restored.
    fn import(&self, data: &Value) -> Result<usize, String>;
}
