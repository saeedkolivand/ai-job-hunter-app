//! Schema migrations for `applications.db`.
//!
//! Split out of [`super`] so the store body stays under the architecture LOC cap
//! (`tests/architecture.rs` R8) and so the append-only migration list has an
//! obvious home as it grows. Index in this slice = `PRAGMA user_version` (1-based,
//! see [`crate::db::run_migrations`]), so entries are **append-only**: never
//! reorder, never delete, never edit an already-shipped body.
//!
//! Every migration is additive and forward-safe — `ADD COLUMN` with a default, or
//! a guarded backfill `UPDATE`. Nothing here drops a column or a table.

use crate::db::Migration;

/// The ordered migration list for `applications.db`. Append only.
pub(super) const MIGRATIONS: &[Migration] = &[
    Migration {
        name: "create_applications",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS applications (
                    id              TEXT PRIMARY KEY,
                    status          TEXT NOT NULL DEFAULT 'saved',
                    applied_at      INTEGER,
                    created_at      INTEGER NOT NULL,
                    updated_at      INTEGER NOT NULL,
                    job_url         TEXT NOT NULL DEFAULT '',
                    board           TEXT NOT NULL DEFAULT '',
                    company         TEXT NOT NULL DEFAULT '',
                    title           TEXT NOT NULL DEFAULT '',
                    candidate       TEXT NOT NULL DEFAULT '',
                    answers         TEXT NOT NULL DEFAULT '[]',
                    brief           TEXT NOT NULL DEFAULT '',
                    notes           TEXT NOT NULL DEFAULT '',
                    next_action_at  INTEGER,
                    comp            TEXT NOT NULL DEFAULT '',
                    contact_name    TEXT NOT NULL DEFAULT '',
                    contact_email   TEXT NOT NULL DEFAULT ''
                );
                CREATE INDEX IF NOT EXISTS idx_applications_job_url
                    ON applications(job_url);",
            )
        },
    },
    Migration {
        name: "create_status_events",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS status_events (
                    application_id  TEXT NOT NULL,
                    from_status     TEXT NOT NULL DEFAULT '',
                    to_status       TEXT NOT NULL,
                    at              INTEGER NOT NULL,
                    note            TEXT NOT NULL DEFAULT ''
                );
                CREATE INDEX IF NOT EXISTS idx_status_events_app
                    ON status_events(application_id);",
            )
        },
    },
    Migration {
        name: "add_applications_job_description",
        up: |conn| {
            conn.execute_batch(
                "ALTER TABLE applications ADD COLUMN job_description TEXT NOT NULL DEFAULT '';",
            )
        },
    },
    Migration {
        name: "add_applications_job_summary",
        up: |conn| {
            conn.execute_batch(
                "ALTER TABLE applications ADD COLUMN job_summary TEXT NOT NULL DEFAULT ''",
            )
        },
    },
    Migration {
        name: "add_applications_recipient",
        up: |conn| {
            conn.execute_batch(
                "ALTER TABLE applications ADD COLUMN recipient_name TEXT NOT NULL DEFAULT '';
                 ALTER TABLE applications ADD COLUMN recipient_email TEXT NOT NULL DEFAULT '';",
            )
        },
    },
    Migration {
        name: "add_applications_salary",
        up: |conn| {
            // Nullable, NOT text-default: NULL means "unknown salary" (mirrors
            // applied_at/next_action_at), never 0 — a 0 would read as a real
            // (wrong) salary downstream.
            conn.execute_batch(
                "ALTER TABLE applications ADD COLUMN salary_min REAL;
                 ALTER TABLE applications ADD COLUMN salary_max REAL;
                 ALTER TABLE applications ADD COLUMN salary_currency TEXT;",
            )
        },
    },
    Migration {
        name: "unify_application_contact",
        up: |conn| {
            // Contact unification: `contact_name`/`contact_email` become THE
            // single primary contact per application; `recipient_name`/
            // `recipient_email` (the apply-by-email pair) are demoted to
            // deprecated aliases of it — see `super::Application`.
            //
            // Backfill promotes an alias-only value onto the canonical pair; a
            // row that already has a canonical value keeps it (canonical wins).
            // NON-DESTRUCTIVE on purpose: the deprecated columns are left
            // exactly as they are, so a row whose two pairs genuinely differed
            // still has its old alias value on disk. The store simply stops
            // reading and writing them (`super::SELECT_COLS` /
            // `super::ApplicationStore::write_row_conn`).
            //
            // Idempotent: re-running finds the canonical column already
            // non-empty (or both empty) and writes the same value back.
            conn.execute_batch(
                "UPDATE applications
                    SET contact_name  = COALESCE(NULLIF(contact_name,  ''), recipient_name),
                        contact_email = COALESCE(NULLIF(contact_email, ''), recipient_email)
                  WHERE contact_name = '' OR contact_email = '';",
            )
        },
    },
    Migration {
        name: "add_applications_next_action_notified_at",
        up: |conn| {
            // Reminder dedupe marker: epoch-ms of the follow-up notification
            // already raised for the CURRENT `next_action_at`. NULL = not yet
            // notified, so a fresh/rescheduled reminder fires exactly once.
            // Nullable (never 0) for the same reason as `next_action_at`.
            // Deliberately NOT part of the `Application` wire type — it is
            // backend bookkeeping, invisible to the renderer.
            conn.execute_batch(
                "ALTER TABLE applications ADD COLUMN next_action_notified_at INTEGER;",
            )
        },
    },
];
