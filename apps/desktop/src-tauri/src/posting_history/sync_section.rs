//! The optional self-hosted sync's per-record view of `postingHistory`
//! (ADR-051): one record per ghost key, the key itself being the record id.

use serde_json::Value;

use super::{insert_conn, HistoryRow, PostingHistoryStore};
use crate::data_store::{
    array_records, parse_record, str_field, DataStore, SyncRecord, SyncSection,
};
use crate::error::AppResult;

impl SyncSection for PostingHistoryStore {
    fn section(&self) -> &'static str {
        self.key()
    }

    fn schema_version(&self) -> u32 {
        Self::MIGRATIONS.len() as u32
    }

    fn sync_records(&self) -> AppResult<Vec<SyncRecord>> {
        array_records(self.try_export()?, |row| str_field(row, "ghostKey"))
    }

    fn upsert_record(&self, id: &str, payload: &Value) -> AppResult<()> {
        let row = HistoryRow {
            ghost_key: id.to_string(),
            // A peer's source says nothing about THIS device's markers; empty
            // never counts as a repost, the next local sighting sets it.
            last_source: String::new(),
            ..parse_record(payload)?
        };
        insert_conn(&self.conn.lock(), &row)
    }

    fn delete_record(&self, id: &str) -> AppResult<()> {
        self.conn.lock().execute(
            "DELETE FROM posting_history WHERE ghost_key = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }
}
