//! The optional self-hosted sync's per-record view of
//! `applicationStatusEvents` (ADR-051): one record per status-history event.
//!
//! An event's SQLite `rowid` only means something on the device that wrote
//! it, so the record id is the event's content key instead —
//! `(applicationId, at, fromStatus, toStatus, source)`. `note` and
//! `confirmed` and `dismissed` are left out of the key on purpose: they are what can
//! change on an existing event (a pending email-derived event being accepted or
//! refused), and a change must update the record rather than add a second one.
//! A dismissal is one-way: an older payload of the same event (still pending, or
//! from a peer that predates `dismissed`) never un-dismisses it.
//!
//! Unlike a backup restore, an event whose application is not in the store
//! yet is still written: the two sections arrive as separate records, and the
//! application may simply come later in the same pull.

use rusqlite::params;
use serde_json::Value;

use super::status_events_backup::StatusEventRecord;
use super::{migrations, ApplicationStatusEvents};
use crate::data_store::{
    array_records, composite_id, parse_record, split_id, DataStore, SyncRecord, SyncSection,
};
use crate::db::ts_to_db;
use crate::error::{AppError, AppResult};

/// The record id of one event.
fn event_id(ev: &StatusEventRecord) -> String {
    composite_id(&[
        &ev.application_id,
        &ev.at.to_string(),
        &ev.from_status,
        &ev.to_status,
        &ev.source,
    ])
}

/// The event an id names, with `note`/`confirmed`/`dismissed` taken from `payload`.
fn event_of(id: &str, payload: &Value) -> AppResult<StatusEventRecord> {
    let key = split_id(id, 5)?;
    let at = key[1]
        .parse()
        .map_err(|_| AppError::Validation("status event id: bad timestamp".into()))?;
    let body: StatusEventRecord = parse_record(payload)?;
    Ok(StatusEventRecord {
        application_id: key[0].clone(),
        at,
        from_status: key[2].clone(),
        to_status: key[3].clone(),
        source: key[4].clone(),
        ..body
    })
}

impl SyncSection for ApplicationStatusEvents<'_> {
    fn section(&self) -> &'static str {
        self.key()
    }

    fn schema_version(&self) -> u32 {
        migrations::MIGRATIONS.len() as u32
    }

    fn sync_records(&self) -> AppResult<Vec<SyncRecord>> {
        array_records(self.try_export()?, |row| {
            parse_record::<StatusEventRecord>(row)
                .ok()
                .map(|ev| event_id(&ev))
        })
    }

    fn upsert_record(&self, id: &str, payload: &Value) -> AppResult<()> {
        let ev = event_of(id, payload)?;
        let mut guard = self.0.conn.lock();
        let tx = guard.transaction()?;
        let updated = tx.execute(
            "UPDATE status_events SET note = ?6,
                 confirmed = CASE WHEN dismissed = 1 OR ?8 = 1 THEN 1 ELSE ?7 END,
                 dismissed = MAX(dismissed, ?8)
             WHERE application_id = ?1 AND at = ?2 AND from_status = ?3
               AND to_status = ?4 AND source = ?5",
            params![
                ev.application_id,
                ts_to_db(ev.at),
                ev.from_status,
                ev.to_status,
                ev.source,
                ev.note,
                i64::from(ev.confirmed),
                i64::from(ev.dismissed),
            ],
        )?;
        if updated == 0 {
            tx.execute(
                "INSERT INTO status_events
                    (application_id, from_status, to_status, at, note, source, confirmed, dismissed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    ev.application_id,
                    ev.from_status,
                    ev.to_status,
                    ts_to_db(ev.at),
                    ev.note,
                    ev.source,
                    i64::from(ev.confirmed),
                    i64::from(ev.dismissed),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn delete_record(&self, id: &str) -> AppResult<()> {
        let key = split_id(id, 5)?;
        let at: u64 = key[1]
            .parse()
            .map_err(|_| AppError::Validation("status event id: bad timestamp".into()))?;
        self.0.conn.lock().execute(
            "DELETE FROM status_events
             WHERE application_id = ?1 AND at = ?2 AND from_status = ?3
               AND to_status = ?4 AND source = ?5",
            params![key[0], ts_to_db(at), key[2], key[3], key[4]],
        )?;
        Ok(())
    }
}
