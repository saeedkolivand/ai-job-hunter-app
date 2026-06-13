//! Manual referral helper — a local-only store of "referral contacts" (people
//! the user wants to ask for a referral at a target company).
//!
//! There is **no LinkedIn scraping** in this feature: every person detail is
//! entered manually by the user, and `linkedin_url` is just an optional free-text
//! field — never fetched. Persistence mirrors [`crate::ai_generations`]: a
//! multi-row SQLite table opened with the shared migration runner, indexed by
//! `job_url` so a found job's referral contacts can be listed cheaply.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data_store::DataStore;
use crate::db::{run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::AppResult;

/// One locally-stored referral contact. Optional drafts/notes are empty strings
/// on the wire when unset (the renderer treats `""` as "none").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferralContact {
    pub id: String,
    /// The job this referral targets (links to the autopilot found job; indexed).
    pub job_url: String,
    pub company_name: String,
    pub person_name: String,
    #[serde(default)]
    pub person_role: String,
    /// Manual free text — never fetched/scraped.
    #[serde(default)]
    pub linkedin_url: String,
    #[serde(default)]
    pub email_draft: String,
    #[serde(default)]
    pub message_draft: String,
    #[serde(default)]
    pub invite_note_draft: String,
    /// `'email' | 'linkedin_message' | 'connection_note'`.
    pub channel: String,
    /// `'draft' | 'sent' | 'replied'`.
    pub status: String,
    #[serde(default)]
    pub notes: String,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct ReferralStore {
    conn: Mutex<Connection>,
}

impl ReferralStore {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_referrals",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS referrals (
                    id                TEXT PRIMARY KEY,
                    job_url           TEXT NOT NULL DEFAULT '',
                    company_name      TEXT NOT NULL DEFAULT '',
                    person_name       TEXT NOT NULL DEFAULT '',
                    person_role       TEXT NOT NULL DEFAULT '',
                    linkedin_url      TEXT NOT NULL DEFAULT '',
                    email_draft       TEXT NOT NULL DEFAULT '',
                    message_draft     TEXT NOT NULL DEFAULT '',
                    invite_note_draft TEXT NOT NULL DEFAULT '',
                    channel           TEXT NOT NULL DEFAULT 'email',
                    status            TEXT NOT NULL DEFAULT 'draft',
                    notes             TEXT NOT NULL DEFAULT '',
                    created_at        INTEGER NOT NULL DEFAULT 0,
                    updated_at        INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS idx_referrals_job_url
                    ON referrals(job_url);",
            )
        },
    }];

    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let path = data_dir.join("referrals.db");
        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
        run_migrations(&conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM referrals", []).ok();
    }

    /// All contacts, most-recently-updated first.
    pub fn list(&self) -> Vec<ReferralContact> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT id, job_url, company_name, person_name, person_role, linkedin_url,
                    email_draft, message_draft, invite_note_draft, channel, status, notes,
                    created_at, updated_at
             FROM referrals ORDER BY updated_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], row_to_record)
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    /// Contacts linked to `job_url`, most-recently-updated first.
    pub fn list_by_job(&self, job_url: &str) -> Vec<ReferralContact> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT id, job_url, company_name, person_name, person_role, linkedin_url,
                    email_draft, message_draft, invite_note_draft, channel, status, notes,
                    created_at, updated_at
             FROM referrals WHERE job_url = ?1 ORDER BY updated_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(params![job_url], row_to_record)
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    /// Insert (when `rec.id` is new) or overwrite an existing row by id. The
    /// caller sets `updated_at` on every write and supplies a `created_at` for
    /// fresh inserts; on conflict the existing row's `created_at` is kept
    /// atomically in SQL (`COALESCE(referrals.created_at, excluded.created_at)`),
    /// so `created_at` is immutable-after-insert with no read-then-write race.
    pub fn upsert(&self, rec: &ReferralContact) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO referrals
             (id, job_url, company_name, person_name, person_role, linkedin_url,
              email_draft, message_draft, invite_note_draft, channel, status, notes,
              created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
             ON CONFLICT(id) DO UPDATE SET
              job_url = ?2, company_name = ?3, person_name = ?4, person_role = ?5,
              linkedin_url = ?6, email_draft = ?7, message_draft = ?8,
              invite_note_draft = ?9, channel = ?10, status = ?11, notes = ?12,
              created_at = COALESCE(referrals.created_at, excluded.created_at),
              updated_at = ?14",
            params![
                rec.id,
                rec.job_url,
                rec.company_name,
                rec.person_name,
                rec.person_role,
                rec.linkedin_url,
                rec.email_draft,
                rec.message_draft,
                rec.invite_note_draft,
                rec.channel,
                rec.status,
                rec.notes,
                ts_to_db(rec.created_at),
                ts_to_db(rec.updated_at),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM referrals WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Map a DB row (the full 14-column projection) to a record. Shared by every
/// read query so the column order lives in one place.
fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<ReferralContact> {
    Ok(ReferralContact {
        id: row.get(0)?,
        job_url: row.get(1)?,
        company_name: row.get(2)?,
        person_name: row.get(3)?,
        person_role: row.get(4)?,
        linkedin_url: row.get(5)?,
        email_draft: row.get(6)?,
        message_draft: row.get(7)?,
        invite_note_draft: row.get(8)?,
        channel: row.get(9)?,
        status: row.get(10)?,
        notes: row.get(11)?,
        created_at: ts_from_db(row.get::<_, i64>(12)?),
        updated_at: ts_from_db(row.get::<_, i64>(13)?),
    })
}

#[cfg(test)]
mod test;

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn make_referral_id() -> String {
    format!("ref-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
}

impl DataStore for ReferralStore {
    fn key(&self) -> &'static str {
        "referrals"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::json!(self.list())
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data.as_array().ok_or("referrals: expected an array")?;
        self.clear_all();
        let mut count = 0;
        for item in items {
            let record: ReferralContact =
                serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
            self.upsert(&record)?;
            count += 1;
        }
        Ok(count)
    }
}
