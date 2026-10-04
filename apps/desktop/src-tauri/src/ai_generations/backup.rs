//! The portable-bundle surface of the `ai_generations` store ([`DataStore`]):
//! export and the all-or-nothing import.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8).

use super::quality_report::sanitize_quality_report;
use super::rows::insert_row;
use super::{AiGenerationRecord, AiGenerationStore};
use crate::data_store::DataStore;
use crate::error::{AppError, AppResult};

impl DataStore for AiGenerationStore {
    fn key(&self) -> &'static str {
        "aiGenerations"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::json!(self.list())
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data.as_array().ok_or("aiGenerations: expected an array")?;
        // Deserialize EVERY record before mutating the store, so a malformed row
        // aborts the import without having cleared the table.
        let records: Vec<AiGenerationRecord> = items
            .iter()
            .map(|item| serde_json::from_value(item.clone()).map_err(AppError::from))
            .collect::<AppResult<_>>()?;

        // `clear_all` + the repopulation loop run in ONE transaction: either the
        // whole replace lands or the old rows are left untouched on any failure.
        // `Connection::transaction` needs `&mut Connection`, so take the lock and
        // call on `&mut *guard`.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM ai_generations", [])?;
        for rec in &records {
            // L-5: the IPC save path (`commands::ai_generations::ai_generations_save`)
            // already guards `quality_report` against QUALITY_REPORT_MAX_BYTES; a
            // restored backup bundle is an equally untrusted write path (a
            // user-supplied file, not our own renderer) and must get the same
            // guard, not just whatever byte count the bundle happened to carry.
            // A byte clamp would truncate mid-JSON (unparseable = silently "no
            // report" via `merge_quality_report`'s first guard); the empty
            // sentinel below reaches that same "no report" outcome by design.
            let quality_report = sanitize_quality_report(rec.quality_report.clone(), "import");
            // Backup-restore is a live re-infection path for pre-#955 mojibake —
            // `insert_row` applies `repaired_generation_texts`. It runs on this
            // transaction (it does not go through `insert()`, which takes
            // `&self.conn.lock()` and would deadlock here).
            insert_row(&tx, rec, &quality_report)?;
        }
        tx.commit()?;
        Ok(records.len())
    }
}
