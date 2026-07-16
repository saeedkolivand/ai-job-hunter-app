//! Email-confirmation watching (Task #23, auto-track Layer C) — the persisted
//! account/dedupe store behind IMAP-based application-confirmation matching.
//!
//! **PR A scope**: this store + the connect-time IMAP validation
//! ([`imap_client`]) only. No poller, no parser, no matcher yet — those land in
//! PR B, which will read/write this same schema (the `seen` dedupe table and
//! `reset_on_uidvalidity_change` exist now, unused by any command, so PR B
//! extends this file instead of reshaping it).
//!
//! **Backup/reset posture**: unlike most per-domain SQLite stores in this
//! crate, `EmailWatchStore` is **NOT** a [`crate::data_store::DataStore`] (not
//! included in the backup/export bundle) — like `CredentialStore`, its
//! contents are machine-local mailbox bookkeeping (a UID watermark + a dedupe
//! log), meaningless on another machine and not user content worth restoring.
//! It **IS** [`crate::data_store::Resettable`]: a factory reset wipes both
//! tables (see `commands/privacy.rs`). The app password itself is never stored
//! here — it lives in the OS keychain under [`CREDENTIAL_SLOT`], set/cleared by
//! the command layer via [`crate::credentials::CredentialStore`].
//!
//! Schema (`email_watch.db`):
//! - `account` — single row (`id = 1`, mirrors `ai_config`'s singleton-row
//!   pattern): `address`/`host`/`port` (the configured mailbox; host/port
//!   default to Gmail's IMAP endpoint — v1 is Gmail-branded, but the value is
//!   DATA, not hardcoded, so a future non-Gmail provider needs no code change),
//!   `enabled` (the poller opt-in PR B will gate on — default OFF), and the
//!   poller's own watermark: `last_uid`/`uidvalidity`/`last_check_ms`.
//! - `seen` — `uid` (PK) → `matched_app_id` (nullable) + `ts`. Dedupes which
//!   messages have already been considered so the (future) poller never
//!   double-notifies for the same UID.

use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::db::{open, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::AppResult;

pub mod imap_client;

/// OS-keychain slot for the IMAP app password (never persisted in SQLite,
/// never logged, never returned over IPC). Read/written via
/// [`crate::credentials::CredentialStore`], the same instance-based path
/// already used for `ai:*` provider keys.
pub const CREDENTIAL_SLOT: &str = "email-imap";

// ── Types ─────────────────────────────────────────────────────────────────────

/// Raw persisted account row. `address` is `None` until the first successful
/// `connect`; every other field is `None`/`false`/default until set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmailWatchAccount {
    pub address: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub enabled: bool,
    pub last_uid: Option<u32>,
    pub uidvalidity: Option<u32>,
    pub last_check_ms: Option<u64>,
}

/// The IPC read model returned by every `email_watch_*` command.
/// `connected` means "an account has been configured" (a successful
/// `connect`), not "an IMAP socket is open right now" — there is no
/// persistent live connection; PR A/B both connect fresh per check.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailWatchStatus {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_match_at: Option<u64>,
}

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct EmailWatchStore {
    /// `parking_lot::Mutex` — not reentrant; never re-lock while a guard is
    /// held and never hold a guard across an `.await`. Every method
    /// takes/releases the lock and returns owned values.
    conn: Mutex<Connection>,
}

impl EmailWatchStore {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_email_watch",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS account (
                    id            INTEGER PRIMARY KEY CHECK (id = 1),
                    address       TEXT,
                    host          TEXT,
                    port          INTEGER,
                    enabled       INTEGER NOT NULL DEFAULT 0,
                    last_uid      INTEGER,
                    uidvalidity   INTEGER,
                    last_check_ms INTEGER
                );
                INSERT OR IGNORE INTO account (id, enabled) VALUES (1, 0);
                CREATE TABLE IF NOT EXISTS seen (
                    uid            TEXT PRIMARY KEY,
                    matched_app_id TEXT,
                    ts             INTEGER NOT NULL
                );",
            )
        },
    }];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("email_watch.db");
        let mut conn = open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ── Reads ──────────────────────────────────────────────────────────────────

    pub fn account(&self) -> EmailWatchAccount {
        let conn = self.conn.lock();
        Self::account_conn(&conn)
    }

    /// The full IPC read model (account fields + the last-match watermark
    /// derived from `seen`).
    pub fn status(&self) -> EmailWatchStatus {
        let conn = self.conn.lock();
        let account = Self::account_conn(&conn);
        let last_match_at = Self::last_match_at_conn(&conn);
        EmailWatchStatus {
            connected: account.address.is_some(),
            address: account.address,
            enabled: account.enabled,
            last_check_at: account.last_check_ms,
            last_match_at,
        }
    }

    /// Whether `uid` has already been considered (dedupe gate for the future
    /// poller — unused by any command in PR A, exercised directly by tests so
    /// the schema/uniqueness contract is locked before PR B builds on it).
    pub fn has_seen(&self, uid: &str) -> bool {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT 1 FROM seen WHERE uid = ?1",
            params![uid],
            |_| Ok(()),
        )
        .is_ok()
    }

    // ── Writes ─────────────────────────────────────────────────────────────────

    /// Upsert the configured mailbox. Never touches `enabled`/`last_check_ms`
    /// — a reconnect (even to a different address/host) always preserves the
    /// current opt-in; only [`Self::clear`] resets that.
    ///
    /// The UID watermark (`last_uid`/`uidvalidity`) and the `seen` dedupe
    /// table are preserved ONLY when `address` is unchanged from what's
    /// already stored (a same-mailbox reconnect, e.g. re-entering a rotated
    /// app password). Connecting to a genuinely DIFFERENT address clears both
    /// — numeric IMAP UIDs are per-mailbox, so carrying a UID/seen row over
    /// from a different account could collide with the new mailbox's own
    /// numbering and silently suppress a real future match.
    pub fn connect(&self, address: &str, host: &str, port: u16) -> AppResult<()> {
        let conn = self.conn.lock();
        let previous_address: Option<String> =
            conn.query_row("SELECT address FROM account WHERE id = 1", [], |row| {
                row.get(0)
            })?;
        let address_changed = previous_address.as_deref() != Some(address);

        conn.execute(
            "UPDATE account SET address = ?1, host = ?2, port = ?3 WHERE id = 1",
            params![address, host, i64::from(port)],
        )?;
        if address_changed {
            conn.execute(
                "UPDATE account SET last_uid = NULL, uidvalidity = NULL WHERE id = 1",
                [],
            )?;
            conn.execute("DELETE FROM seen", [])?;
        }
        Ok(())
    }

    /// The poller opt-in (default OFF; independent of whether an account is
    /// configured — connecting does NOT auto-enable, per ADR-0005's
    /// default-OFF posture).
    ///
    /// Guarded by `address IS NOT NULL`: a concurrent `disconnect`/factory
    /// reset (`clear`) can land between a caller's read and this write (e.g.
    /// while a command is mid-`spawn_blocking` IMAP validation) — without the
    /// guard this write would resurrect `enabled` on an already-wiped
    /// account (worst case: `enabled=1` with `address=NULL`, which a LATER
    /// `connect` would silently inherit, since `connect` deliberately never
    /// touches `enabled`). Returns whether the row was actually updated
    /// (`false` = lost the race to a clear — a no-op, not an error).
    pub fn set_enabled(&self, enabled: bool) -> AppResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE account SET enabled = ?1 WHERE id = 1 AND address IS NOT NULL",
            params![i64::from(enabled)],
        )?;
        Ok(affected > 0)
    }

    /// Record a successful connectivity check (`connect`/`check_now`). Same
    /// concurrent-clear guard and no-op-reporting contract as
    /// [`Self::set_enabled`] — a `disconnect` racing a multi-second
    /// `spawn_blocking` IMAP validation must not resurrect `last_check_ms` on
    /// an already-wiped account.
    pub fn record_check(&self, ts_ms: u64) -> AppResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE account SET last_check_ms = ?1 WHERE id = 1 AND address IS NOT NULL",
            params![ts_to_db(ts_ms)],
        )?;
        Ok(affected > 0)
    }

    /// Advance the persisted UID watermark after processing messages up
    /// through `uid` (the poller always calls this with the highest UID it
    /// just handled — monotonic, never rewound except by
    /// [`Self::reset_on_uidvalidity_change`]). Unused by any command in PR A;
    /// exercised directly by tests so the watermark contract is locked before
    /// PR B's poller depends on it.
    pub fn advance_last_uid(&self, uid: u32) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE account SET last_uid = ?1 WHERE id = 1",
            params![i64::from(uid)],
        )?;
        Ok(())
    }

    /// Mark `uid` as considered, optionally recording the application it
    /// matched. `INSERT OR IGNORE`: re-marking an already-seen uid is a no-op
    /// (the first stamp wins) — the poller's dedupe gate, not a poller-owned
    /// idempotency retry.
    pub fn mark_seen(&self, uid: &str, matched_app_id: Option<&str>, ts_ms: u64) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO seen (uid, matched_app_id, ts) VALUES (?1, ?2, ?3)",
            params![uid, matched_app_id, ts_to_db(ts_ms)],
        )?;
        Ok(())
    }

    /// `UIDVALIDITY` changed (the mailbox was recreated/renumbered by the
    /// server) — the stored `last_uid` watermark is meaningless against the
    /// new numbering and must be dropped. Returns whether a reset happened
    /// (`false` when `new_uidvalidity` already matches the stored value, in
    /// which case `last_uid` is left untouched).
    pub fn reset_on_uidvalidity_change(&self, new_uidvalidity: u32) -> AppResult<bool> {
        let conn = self.conn.lock();
        let current: Option<i64> =
            conn.query_row("SELECT uidvalidity FROM account WHERE id = 1", [], |row| {
                row.get(0)
            })?;
        let changed = current != Some(i64::from(new_uidvalidity));
        if changed {
            conn.execute(
                "UPDATE account SET uidvalidity = ?1, last_uid = NULL WHERE id = 1",
                params![i64::from(new_uidvalidity)],
            )?;
        }
        Ok(changed)
    }

    /// Full wipe: the account row back to its just-migrated defaults, and
    /// every `seen` row gone. Used by BOTH `email_watch_disconnect` (the
    /// keychain credential itself is removed separately by the command layer
    /// via `CredentialStore`) and the factory reset (`Resettable::reset` in
    /// `commands/privacy.rs`).
    pub fn clear(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE account SET address = NULL, host = NULL, port = NULL, enabled = 0,
             last_uid = NULL, uidvalidity = NULL, last_check_ms = NULL WHERE id = 1",
            [],
        )?;
        conn.execute("DELETE FROM seen", [])?;
        Ok(())
    }

    // ── Connection-bound helpers ─────────────────────────────────────────────

    fn account_conn(conn: &Connection) -> EmailWatchAccount {
        conn.query_row(
            "SELECT address, host, port, enabled, last_uid, uidvalidity, last_check_ms
             FROM account WHERE id = 1",
            [],
            |row| {
                Ok(EmailWatchAccount {
                    address: row.get(0)?,
                    host: row.get(1)?,
                    port: row
                        .get::<_, Option<i64>>(2)?
                        .and_then(|p| u16::try_from(p).ok()),
                    enabled: row.get::<_, i64>(3)? != 0,
                    last_uid: row
                        .get::<_, Option<i64>>(4)?
                        .and_then(|v| u32::try_from(v).ok()),
                    uidvalidity: row
                        .get::<_, Option<i64>>(5)?
                        .and_then(|v| u32::try_from(v).ok()),
                    last_check_ms: row.get::<_, Option<i64>>(6)?.map(ts_from_db),
                })
            },
        )
        .unwrap_or_default()
    }

    fn last_match_at_conn(conn: &Connection) -> Option<u64> {
        conn.query_row(
            "SELECT MAX(ts) FROM seen WHERE matched_app_id IS NOT NULL",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .ok()
        .flatten()
        .map(ts_from_db)
    }
}

#[cfg(test)]
mod tests;
