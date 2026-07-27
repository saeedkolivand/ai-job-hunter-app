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

/// The whitespace set SQLite's two-argument `TRIM(x, y)` must strip so the
/// `unify_application_contact` emptiness test agrees with Rust's `str::trim` —
/// see `super::contact` for the other half of that lockstep.
///
/// Bare `TRIM(x)` strips ONLY U+0020, whereas `str::trim` strips all Unicode
/// `White_Space`. So a `"\u{A0}"` (NBSP — endemic in text copied out of scraped
/// HTML) or a TAB read as NON-empty in SQL and as empty in Rust, and the same
/// row folded one way through the in-place migration and the other way through
/// a restored bundle.
///
/// Covers space, TAB, LF, CR and NBSP — everything a real contact field picks up
/// from HTML or a paste. Rust stays a strict SUPERSET for exotic separators
/// (U+2028, U+3000, …); that residual divergence is documented and accepted in
/// `super::contact` rather than chased with an unbounded `char()` list.
const SQL_TRIM_CHARS: &str = "' ' || char(9) || char(10) || char(13) || char(160)";

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
            // **PAIR-ATOMIC.** The promotion moves name AND email together, and
            // only when the WHOLE canonical pair is empty. Promoting each column
            // independently would fuse two different people — person A's name
            // (already in `contact_name`) with person B's address (in
            // `recipient_email`) — and that fused identity feeds the
            // apply-by-email `mailto:` sink, i.e. a message addressed to B under
            // A's name. Same rule, same emptiness test (whitespace-only values
            // are reachable from pre-trim builds — see [`SQL_TRIM_CHARS`]), as
            // `super::Application::canonicalize_contact`, which applies it to an
            // imported bundle — the two MUST stay in lockstep.
            //
            // Statement 1 preserves an apply-by-email contact that statement 2
            // will NOT promote (the canonical pair is occupied by someone else)
            // by appending it to `notes`. Without it that person is lost for
            // good: the store stops reading the deprecated columns, and an
            // export mirrors the canonical pair, so an export/import round trip
            // could never recover them. Mirrors the note this migration's
            // import-time twin builds — see `super::contact`.
            //
            // NON-DESTRUCTIVE: the deprecated columns are never written, so a
            // row whose two pairs genuinely differed still has its old alias
            // value on disk too. The store simply stops reading them
            // (`super::SELECT_COLS` / `super::ApplicationStore::write_row_conn`).
            //
            // Idempotent: statement 1 skips a note it already appended (`instr`
            // on the exact line), and statement 2's `WHERE` no longer matches a
            // row it already promoted.
            conn.execute_batch(&format!(
                "UPDATE applications
                    SET notes = CASE WHEN notes = '' THEN '' ELSE notes || char(10) || char(10) END
                                || 'Apply-by-email: ' || recipient_name || ' <' || recipient_email || '>'
                  WHERE (TRIM(recipient_name, {WS}) <> '' OR TRIM(recipient_email, {WS}) <> '')
                    AND NOT (TRIM(contact_name, {WS}) = '' AND TRIM(contact_email, {WS}) = '')
                    AND (recipient_name <> contact_name OR recipient_email <> contact_email)
                    AND instr(
                          notes,
                          'Apply-by-email: ' || recipient_name || ' <' || recipient_email || '>'
                        ) = 0;

                 UPDATE applications
                    SET contact_name  = recipient_name,
                        contact_email = recipient_email
                  WHERE TRIM(contact_name, {WS}) = '' AND TRIM(contact_email, {WS}) = ''
                    AND (TRIM(recipient_name, {WS}) <> '' OR TRIM(recipient_email, {WS}) <> '');",
                WS = SQL_TRIM_CHARS
            ))
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
