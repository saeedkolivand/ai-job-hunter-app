//! Email-confirmation watching (Task #23, auto-track Layer C) — the persisted
//! account/dedupe store behind IMAP-based application-confirmation matching.
//!
//! **PR A** shipped this store + the connect-time IMAP validation
//! ([`imap_client`]). **PR B** adds the read path: [`imap_client`]'s header/
//! body fetch, [`parser`] (pure RFC2047/MIME decode + fingerprint + candidate
//! extraction), [`matcher`] (pure token-Jaccard company/title scoring), and
//! [`poller`] (the Tauri-free tick orchestration tying the three together).
//! The actual background schedule lives ABOVE this module family, in the
//! top-level `email_watch_scheduler` (L2) — mirroring the `autopilot`/
//! `autopilot_scheduler` split, so this store never needs an upward reach
//! into `commands::notifications` itself.
//!
//! **v2 slice 1** adds [`intent`]: a pure 4-way (confirmation/rejection/
//! interview/offer) classifier over subject+body, plus the pure
//! `intent::next_status` status-ladder rule.
//!
//! **v2 slice 2** adds [`auto_write`]: the actual `ApplicationStore` write —
//! [`auto_write::apply_matched_intent`] turns one classified `EmailIntent`
//! into a real, but always UNCONFIRMED, status transition (gated first by
//! [`EmailWatchStore::auto_write_enabled`] — originally default ON, flipped to
//! default OFF once a residual the parser cannot close by content
//! inspection alone was found; see that fn's own doc). Still not wired into
//! [`poller`]'s tick itself — `run_tick` stays pure matching, and the
//! scheduler (L2, the one place in this module family with `AppHandle`
//! reach) is the natural future caller; no IPC in this slice either.
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
//!   `enabled` (the poller opt-in PR B will gate on — default OFF),
//!   `auto_write_enabled` (v2 slice 2's auto-write opt-in — default OFF, see
//!   [`EmailWatchStore::auto_write_enabled`]), and the poller's own watermark:
//!   `last_uid`/`uidvalidity`/`last_check_ms`.
//! - `seen` — `uid` (PK) → `matched_app_id` (nullable) + `ts`. Dedupes which
//!   messages have already been considered so the (future) poller never
//!   double-notifies for the same UID.

use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::db::{open, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::AppResult;

mod auth_results;
pub mod auto_write;
pub mod imap_client;
pub mod intent;
pub mod matcher;
pub mod parser;
pub mod poller;
pub mod status_ladder;

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
    /// The v2 auto-write opt-in — see [`EmailWatchStore::auto_write_enabled`].
    /// Surfaced here (not a second IPC round trip) so the settings UI reads
    /// it from the SAME `email_watch_status()` call it already makes.
    pub auto_write_enabled: bool,
}

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct EmailWatchStore {
    /// `parking_lot::Mutex` — not reentrant; never re-lock while a guard is
    /// held and never hold a guard across an `.await`. Every method
    /// takes/releases the lock and returns owned values.
    conn: Mutex<Connection>,
}

impl EmailWatchStore {
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
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
        },
        Migration {
            name: "add_auto_write_enabled",
            up: |conn| {
                // v2 slice 2 ORIGINALLY shipped this as `DEFAULT 1` (auto-write
                // ON by default); a later fix-forward round found a residual
                // the parser cannot close by content inspection alone (a
                // GENUINE `Authentication-Results` stamp from a known-stamping
                // host that simply carries no `dmarc=` clause for the attacker's
                // chosen `From:` domain -- indistinguishable from real grammar,
                // because it IS real grammar) and the owner decided auto-write
                // ships OFF by default instead: adjudication (every write lands
                // UNCONFIRMED, see `applications::StatusEvent::confirmed`)
                // remains the backstop, but the gate stops being load-bearing
                // for anyone who never explicitly opts in. `DEFAULT 0` is
                // edited HERE, in place, rather than shipped as a second
                // migration flipping the value with an `UPDATE` — this
                // migration is new on this branch and has never shipped (not
                // an ancestor of `origin/main`), so there is no released
                // schema, and no deliberately-set dev-build value, to
                // preserve; a second migration would be the wrong tool for a
                // value nobody has ever depended on. `DEFAULT 0` alone is the
                // whole migration — SQLite applies it retroactively to the row
                // the FIRST migration already seeded, so no separate backfill
                // UPDATE is needed either way.
                conn.execute_batch(
                    "ALTER TABLE account ADD COLUMN auto_write_enabled INTEGER NOT NULL DEFAULT 0;",
                )
            },
        },
    ];

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
        let auto_write_enabled = Self::auto_write_enabled_conn(&conn);
        EmailWatchStatus {
            connected: account.address.is_some(),
            address: account.address,
            enabled: account.enabled,
            last_check_at: account.last_check_ms,
            last_match_at,
            auto_write_enabled,
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
        let mut conn = self.conn.lock();
        let previous_address: Option<String> =
            conn.query_row("SELECT address FROM account WHERE id = 1", [], |row| {
                row.get(0)
            })?;
        let address_changed = previous_address.as_deref() != Some(address);

        // The address/host/port UPDATE and the (conditional) watermark-reset +
        // seen-wipe must land together or not at all — a crash/failure between
        // them would otherwise leave `previous_address == address` on retry (so
        // the cleanup branch never re-fires) while stale `seen` rows from the
        // OLD mailbox persist, exactly the per-mailbox UID-collision hazard this
        // method's own doc warns about. One transaction; `.transaction()` needs
        // `&mut Connection`, so call on the locked guard directly (it derefs
        // mutably).
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE account SET address = ?1, host = ?2, port = ?3 WHERE id = 1",
            params![address, host, i64::from(port)],
        )?;
        if address_changed {
            tx.execute(
                "UPDATE account SET last_uid = NULL, uidvalidity = NULL WHERE id = 1",
                [],
            )?;
            tx.execute("DELETE FROM seen", [])?;
        }
        tx.commit()?;
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

    /// The v2 auto-write opt-in — defaults OFF (same direction as
    /// [`Self::set_enabled`]'s poller opt-in now, though for a different
    /// reason: v2 originally shipped this ON, and it was flipped OFF once a
    /// residual the parser cannot close by content inspection alone was
    /// found — see the `add_auto_write_enabled` migration's own doc for the
    /// full reasoning). [`crate::email_watch::auto_write::
    /// apply_matched_intent`] checks this FIRST, before ever computing a
    /// status transition.
    pub fn auto_write_enabled(&self) -> bool {
        let conn = self.conn.lock();
        Self::auto_write_enabled_conn(&conn)
    }

    /// Set the auto-write opt-in. Same concurrent-clear guard as
    /// [`Self::set_enabled`] — an in-flight auto-write racing a
    /// `disconnect`/factory reset must not resurrect this on an
    /// already-wiped account. Returns whether the row was actually updated
    /// (`false` = lost the race to a clear — a no-op, not an error).
    pub fn set_auto_write_enabled(&self, enabled: bool) -> AppResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE account SET auto_write_enabled = ?1 WHERE id = 1 AND address IS NOT NULL",
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
    /// [`Self::reset_on_uidvalidity_change`]). The `MAX` in the `SET` clause
    /// enforces that monotonic invariant AT THE DATABASE, not just by
    /// caller convention — a caller that (mistakenly, or via a reordered
    /// concurrent write) passes a lower uid than what's already stored can
    /// never rewind the watermark.
    ///
    /// Same concurrent-clear guard as [`Self::set_enabled`]/[`Self::
    /// record_check`] (`AND address IS NOT NULL`): the poller's tick awaits a
    /// multi-second `spawn_blocking` IMAP round trip before calling this — a
    /// `disconnect`/factory reset landing during that window must not
    /// resurrect a watermark on the just-wiped account. Returns whether the
    /// row was actually updated (`false` = lost the race to a clear, or a
    /// value equal to what's already stored — a no-op, not an error).
    pub fn advance_last_uid(&self, uid: u32) -> AppResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE account SET last_uid = MAX(COALESCE(last_uid, 0), ?1) WHERE id = 1 AND address IS NOT NULL",
            params![i64::from(uid)],
        )?;
        Ok(affected > 0)
    }

    /// Mark `uid` as considered, optionally recording the application it
    /// matched. `INSERT OR IGNORE`: re-marking an already-seen uid is a no-op
    /// (the first stamp wins) — the poller's dedupe gate, not a poller-owned
    /// idempotency retry.
    ///
    /// Same concurrent-clear guard as [`Self::advance_last_uid`] — this is an
    /// `INSERT`, not an `UPDATE`, so the guard is expressed as a `SELECT …
    /// WHERE EXISTS` row source instead of a trailing `WHERE`: the literal
    /// values are only actually inserted when a connected account row still
    /// exists at the moment this runs. Returns whether a row was inserted
    /// (`false` = lost the race to a clear, or the uid was already seen).
    pub fn mark_seen(
        &self,
        uid: &str,
        matched_app_id: Option<&str>,
        ts_ms: u64,
    ) -> AppResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "INSERT OR IGNORE INTO seen (uid, matched_app_id, ts)
             SELECT ?1, ?2, ?3
             WHERE EXISTS (SELECT 1 FROM account WHERE id = 1 AND address IS NOT NULL)",
            params![uid, matched_app_id, ts_to_db(ts_ms)],
        )?;
        Ok(affected > 0)
    }

    /// `UIDVALIDITY` changed (the mailbox was recreated/renumbered by the
    /// server) — the stored `last_uid` watermark is meaningless against the
    /// new numbering and must be dropped, AND every `seen` row must go with
    /// it: numeric UIDs are unique only per (mailbox, UIDVALIDITY)
    /// generation, so a stale `seen` row from the OLD generation would make
    /// `has_seen` hit a false positive against a REUSED low uid in the
    /// re-scan window, silently swallowing a real confirmation forever
    /// (`seen` was otherwise only ever pruned on clear/disconnect/address-
    /// switch — the exact same per-generation hazard `connect`'s own
    /// address-changed branch already guards).
    ///
    /// Same concurrent-clear guard as [`Self::advance_last_uid`]: the
    /// `UPDATE` only touches a row with a connected account, and the `seen`
    /// wipe only runs when that `UPDATE` actually affected a row — a
    /// `disconnect`/factory reset landing mid-tick must not resurrect
    /// `uidvalidity`/`last_uid` on the just-wiped account (`clear()` already
    /// wiped `seen` itself in that case; wiping it again here would be a
    /// harmless no-op, but skipping it is more honest about what changed).
    /// Returns whether a reset actually happened (`false` when
    /// `new_uidvalidity` already matches the stored value, OR when the
    /// account vanished before this write landed).
    pub fn reset_on_uidvalidity_change(&self, new_uidvalidity: u32) -> AppResult<bool> {
        let mut conn = self.conn.lock();
        let current: Option<i64> =
            conn.query_row("SELECT uidvalidity FROM account WHERE id = 1", [], |row| {
                row.get(0)
            })?;
        if current == Some(i64::from(new_uidvalidity)) {
            return Ok(false);
        }
        // Same atomicity requirement as `connect`'s address-changed branch:
        // the watermark reset and the `seen` wipe must land together, or a
        // crash between them could leave stale `seen` rows attributed to a
        // UIDVALIDITY generation that no longer exists.
        let tx = conn.transaction()?;
        let affected = tx.execute(
            "UPDATE account SET uidvalidity = ?1, last_uid = NULL WHERE id = 1 AND address IS NOT NULL",
            params![i64::from(new_uidvalidity)],
        )?;
        if affected > 0 {
            tx.execute("DELETE FROM seen", [])?;
        }
        tx.commit()?;
        Ok(affected > 0)
    }

    /// Disconnect wipe: the account row back to its just-migrated defaults,
    /// and every `seen` row gone. Used ONLY by `email_watch_disconnect` (the
    /// keychain credential itself is removed separately by the command layer
    /// via `CredentialStore`) — see [`Self::factory_reset`] for the
    /// privacy-reset path, which is a DIFFERENT operation, not a caller of
    /// this one.
    ///
    /// **`auto_write_enabled` is DELIBERATELY not reset here** — every other
    /// account column goes back to its migrated default, but this one
    /// survives the wipe as-is. Sound for disconnect specifically because a
    /// disconnect/reconnect through `email_watch_disconnect` is the user
    /// re-authenticating the SAME mailbox address they already made this
    /// choice about — not a new account inheriting someone else's opt-in.
    /// WHATEVER they explicitly chose — opted out when the shipped default
    /// was ON, or (the current shipped default) opted IN against a
    /// default-OFF value — a disconnect/reconnect of that same address must
    /// not silently reset it back toward whichever default happens to be
    /// migrated at the time. Deliberately phrased direction-agnostically:
    /// this column's default has already flipped once (v2 shipped it ON; a
    /// residual the parser cannot close by content inspection alone — see
    /// `auto_write_enabled`'s own doc — moved it to OFF), and the safety
    /// property this guards is "preserve the user's own choice for the
    /// account that made it," not "preserve `false`" specifically. That
    /// reasoning stops applying the moment the NEXT connected mailbox might
    /// be a different account — which is exactly what a factory reset
    /// permits and disconnect does not, and is why the two paths are split.
    pub fn clear(&self) -> AppResult<()> {
        let mut conn = self.conn.lock();
        // Same atomicity requirement as `connect`'s address-changed branch —
        // the account wipe and the `seen` wipe must land together, or a
        // failure between them could leave stale `seen` rows attributed to an
        // address that no longer exists.
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE account SET address = NULL, host = NULL, port = NULL, enabled = 0,
             last_uid = NULL, uidvalidity = NULL, last_check_ms = NULL WHERE id = 1",
            [],
        )?;
        tx.execute("DELETE FROM seen", [])?;
        tx.commit()?;
        Ok(())
    }

    /// Privacy factory reset ([`crate::data_store::Resettable::reset`] in
    /// `commands/privacy.rs`)
    /// — wipes everything [`Self::clear`] wipes, PLUS `auto_write_enabled`,
    /// explicitly set to `0` (not "whatever the migration's current DEFAULT
    /// happens to be" — an explicit value, so this stays correct even if a
    /// future migration changes that DEFAULT again).
    ///
    /// NOT the same operation as [`Self::clear`] despite overlapping with
    /// it, and not a caller of it either (own transaction, same SQL
    /// inlined) — a factory reset's whole point is that whatever mailbox
    /// gets connected next may be a DIFFERENT account than whichever one
    /// made this opt-in choice, and that account never agreed to auto-write.
    /// `clear()`'s "preserve the user's own choice" reasoning is sound only
    /// because a disconnect/reconnect re-authenticates the SAME address; it
    /// does not transfer to a factory reset, and treating the two as one
    /// operation was this store's own earlier mistake (an approved fix that
    /// covered only the same-account case). ADR-0013 lists "opt-in default,
    /// nobody exposed without asking" as mitigation #1 for the
    /// `dmarc_pass_aligned` residual — inheriting a stranger account's
    /// opt-in through a factory reset would silently break that mitigation
    /// for the very account that never asked.
    pub fn factory_reset(&self) -> AppResult<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE account SET address = NULL, host = NULL, port = NULL, enabled = 0,
             last_uid = NULL, uidvalidity = NULL, last_check_ms = NULL,
             auto_write_enabled = 0 WHERE id = 1",
            [],
        )?;
        tx.execute("DELETE FROM seen", [])?;
        tx.commit()?;
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

    /// Connection-scoped read behind [`Self::auto_write_enabled`] and
    /// [`Self::status`] (which already holds `self.conn.lock()` and would
    /// deadlock calling the self-locking public method). A read failure
    /// (should not happen — migration `add_auto_write_enabled` seeds every
    /// row with this column) fails CLOSED (`false`), not toward whichever
    /// default the migration happens to seed. This is the FIRST gate in
    /// [`crate::email_watch::auto_write::apply_matched_intent`]; `true`
    /// here on a read error would run auto-write for a user this store
    /// cannot even confirm opted in — the exact outcome the opt-in exists
    /// to prevent, and worse now than it would have been when this line
    /// was `unwrap_or(true)`: that was written while the shipped default
    /// was ON, so failing toward it matched "assume nothing changed
    /// unexpectedly". The default flip (see the `add_auto_write_enabled`
    /// migration's own doc) inverted which direction is safe on error, and
    /// this fallback was not updated along with it — do not trust a
    /// comment's claim about which way a fallback fails; re-derive it from
    /// the CURRENT default and CURRENT gate order, the way the fix here
    /// had to be re-derived.
    fn auto_write_enabled_conn(conn: &Connection) -> bool {
        conn.query_row(
            "SELECT auto_write_enabled FROM account WHERE id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests;
