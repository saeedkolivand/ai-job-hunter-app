//! The portable-bundle surface of the `applications` store ([`DataStore`]): export
//! and the all-or-nothing import.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8).

use super::{Application, ApplicationStore, EVENT_SOURCE_USER};
use crate::data_store::DataStore;
use crate::error::{AppError, AppResult};

impl DataStore for ApplicationStore {
    fn key(&self) -> &'static str {
        "applications"
    }

    fn export(&self) -> serde_json::Value {
        // Export Applications; status history is audit-only + derivable, so it is
        // not part of the portable bundle (mirrors not exporting transient logs).
        serde_json::json!(self.list())
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data
            .as_array()
            .ok_or_else(|| AppError::Parse("applications: expected an array".into()))?;
        // Deserialize EVERY Application before mutating, so a malformed row aborts
        // the import without having cleared the existing tables.
        // `canonicalize_contact` folds a bundle exported by a pre-unification
        // build (`recipientName` only) onto the canonical contact pair, exactly
        // as the `unify_application_contact` migration does for rows in place —
        // otherwise the store would drop it, since it no longer writes the
        // deprecated columns.
        let apps: Vec<Application> = items
            .iter()
            .map(|item| {
                serde_json::from_value::<Application>(item.clone())
                    .map(Application::canonicalize_contact)
                    .map_err(AppError::from)
            })
            .collect::<AppResult<_>>()?;

        // Clear (both tables) + repopulate (each row + its seed event) in ONE
        // transaction: the full bundle replaces the old data or nothing changes.
        // `.transaction()` needs `&mut Connection`, so call on `&mut *guard`.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM applications", [])?;
        tx.execute("DELETE FROM status_events", [])?;
        for app in &apps {
            Self::write_row_conn(&tx, app)?;
            // Seed one event so an imported Application still carries a history
            // row — user-sourced, already-confirmed (a restored backup is
            // settled history, never a pending email-derived write).
            Self::append_event_conn(
                &tx,
                &app.id,
                "",
                app.status.as_id(),
                "imported",
                EVENT_SOURCE_USER,
                true,
                app.updated_at,
            )?;
        }
        tx.commit()?;
        Ok(apps.len())
    }
}
