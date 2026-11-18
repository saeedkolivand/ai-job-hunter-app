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

use crate::error::{AppError, AppResult};

pub trait DataStore {
    /// Key under which this store's data lives in the export bundle.
    fn key(&self) -> &'static str;

    /// Serialize all records as a JSON value (array, or object for single-row).
    fn export(&self) -> Value;

    /// Replace the store's contents from previously-exported data.
    /// Returns the number of records restored.
    fn import(&self, data: &Value) -> AppResult<usize>;
}

/// Insert a section into a bundle, or leave it out (logged by code) when its
/// read failed, so a restore keeps what it has instead of wiping it.
pub fn omit_on_error(
    stores: &mut serde_json::Map<String, Value>,
    key: &str,
    section: AppResult<Value>,
    what: &str,
) {
    match section {
        Ok(v) => {
            stores.insert(key.to_string(), v);
        }
        Err(e) => log::warn!("[data] {what} left out of the backup: {}", e.code()),
    }
}

/// One record of an export-bundle section as the optional self-hosted sync
/// sees it (ADR-051): a stable id inside the section and the record's JSON,
/// in the same shape the section exports.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncRecord {
    pub id: String,
    pub payload: Value,
}

/// Per-record access to an export-bundle section, for the sync engine
/// (`crate::sync`). Unlike [`DataStore::import`], nothing here clears a store:
/// each call writes or deletes exactly one record inside the store's own
/// transaction. Implemented next to each store's `DataStore` impl.
pub trait SyncSection {
    /// The bundle section key; the same string as the store's [`DataStore::key`].
    fn section(&self) -> &'static str;

    /// The store's schema version: its migration count (or a fixed value for a
    /// store without migrations). Compared across devices, never down-converted.
    fn schema_version(&self) -> u32;

    /// Every record of the section, with its id. A single-object section
    /// returns one record whose id is the section key.
    fn sync_records(&self) -> AppResult<Vec<SyncRecord>>;

    /// Insert or replace the whole record `id` with `payload`.
    fn upsert_record(&self, id: &str, payload: &Value) -> AppResult<()>;

    /// Delete record `id`. Absent is not an error. A single-object section has
    /// nothing to delete and returns `Ok`.
    fn delete_record(&self, id: &str) -> AppResult<()>;
}

/// The id of a record keyed by several columns: their JSON array, so no
/// column value can be mistaken for a separator.
pub fn composite_id(parts: &[&str]) -> String {
    serde_json::to_string(parts).unwrap_or_default()
}

/// The `n` columns of a [`composite_id`].
pub fn split_id(id: &str, n: usize) -> AppResult<Vec<String>> {
    let parts: Vec<String> = serde_json::from_str(id)?;
    if parts.len() != n {
        return Err(AppError::Validation(format!(
            "sync record id has {} parts, expected {n}",
            parts.len()
        )));
    }
    Ok(parts)
}

/// The records of an exported array section, each keyed by `id_of`. A row
/// without an id is left out (it could never be addressed on another device).
pub fn array_records(
    exported: Value,
    id_of: impl Fn(&Value) -> Option<String>,
) -> AppResult<Vec<SyncRecord>> {
    let Value::Array(rows) = exported else {
        return Err(AppError::Validation("sync: section is not an array".into()));
    };
    Ok(rows
        .into_iter()
        .filter_map(|payload| id_of(&payload).map(|id| SyncRecord { id, payload }))
        .collect())
}

/// A string field of an exported row, for [`array_records`].
pub fn str_field(row: &Value, key: &str) -> Option<String> {
    row.get(key)?.as_str().map(str::to_string)
}

/// Parse one sync payload into the store's own record type.
pub fn parse_record<T: serde::de::DeserializeOwned>(payload: &Value) -> AppResult<T> {
    Ok(serde_json::from_value(payload.clone())?)
}

/// [`SyncSection`] for a single-object section (one settings row): the whole
/// object is one record, upsert is the store's own import, and there is
/// nothing to delete.
macro_rules! single_object_sync_section {
    ($store:ty, $schema:expr) => {
        impl $crate::data_store::SyncSection for $store {
            fn section(&self) -> &'static str {
                $crate::data_store::DataStore::key(self)
            }
            fn schema_version(&self) -> u32 {
                $schema
            }
            fn sync_records(
                &self,
            ) -> $crate::error::AppResult<Vec<$crate::data_store::SyncRecord>> {
                Ok(vec![$crate::data_store::SyncRecord {
                    id: $crate::data_store::DataStore::key(self).to_string(),
                    payload: $crate::data_store::DataStore::export(self),
                }])
            }
            fn upsert_record(
                &self,
                _id: &str,
                payload: &serde_json::Value,
            ) -> $crate::error::AppResult<()> {
                $crate::data_store::DataStore::import(self, payload).map(|_| ())
            }
            fn delete_record(&self, _id: &str) -> $crate::error::AppResult<()> {
                Ok(())
            }
        }
    };
}
pub(crate) use single_object_sync_section;

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
