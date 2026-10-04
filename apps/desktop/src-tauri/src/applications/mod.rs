//! Application — the status-bearing aggregate root for a job pursuit.
//!
//! Per ADR `docs/knowledge/decision-records/0001-application-aggregate-split.md`, an **Application** is
//! the single source of truth for "am I pursuing this job, and how far along am
//! I". A [`crate::ai_generations`] generation is demoted to a **child Document**
//! (résumé/cover text) that references its parent via `application_id`.
//!
//! "Applied" is no longer "a generation exists for this URL" — it is now
//! "∃ Application(url) with status ≠ `saved`" ([`ApplicationStore::applied_job_urls`]).
//! An Application may have zero child generations (a `saved`/manual/external
//! doc-less pursuit) or many (one URL, separate résumé + cover actions).
//!
//! Persistence mirrors the sibling stores: a multi-row SQLite table opened with
//! the shared migration runner, plus an append-only `status_events` history.
//!
//! This file holds the store handle and `open`; every other slice of the same
//! store lives in a sibling module, split by responsibility (R8): `model` (the
//! types and size caps), `rows` (row mapping + connection-scoped primitives),
//! `reads`, `writes`, `answers` (the two answer merges), `status_events` (the
//! audit trail and its accept/reject), `reminders` (follow-up sweep + claim),
//! `contact` (the alias fold), `job_url` (the dedup-key normalizer),
//! `migrations` (schema + legacy backfill), `orphan_link` (the boot-time
//! generation link) and `backup` (export / import).

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::Connection;

use crate::db::run_migrations;
use crate::error::AppResult;

mod answers;
mod backup;
mod contact;
mod job_url;
mod migrations;
mod model;
mod orphan_link;
mod reads;
mod reminders;
mod rows;
mod status_events;
#[cfg(test)]
mod tests;
mod writes;

// Re-exported so `crate::applications::normalize_job_url` keeps resolving after
// the split (see `job_url` — a verbatim move, no behaviour change).
pub use job_url::normalize_job_url;
// The lookup-time leniency that sits BESIDE the normalizer, never inside it
// (issue #1128) — `pub(crate)`, because only in-process readers comparing two
// spellings of the same url may use it; see its own doc for why it must never
// touch a value on its way to being persisted.
pub(crate) use job_url::decode_unreserved;
// Same for the reminder projection the scheduler imports (see `reminders`).
pub use reminders::FollowUpCandidate;
// `status_events` split out purely for R8 LOC (own doc there). `StatusEvent`
// and the two source constants an OUTSIDE caller actually needs today are
// re-exported here so `crate::applications::StatusEvent`/`EVENT_SOURCE_USER`/
// `EVENT_SOURCE_EMAIL` keep resolving exactly as before the split (the
// `writes`/`backup` siblings' `set_status`/`upsert_internal`/`import` need
// `EVENT_SOURCE_USER` too; `crate::email_watch::auto_write` needs
// `EVENT_SOURCE_EMAIL`).
// `EVENT_SOURCE_EMAIL_REJECT` has no consumer outside `applications` yet — see
// `status_events::EVENT_SOURCE_EMAIL_REJECT` directly (reachable from any
// descendant module, e.g. `applications::tests`) rather than re-exporting an
// unused-in-production name here.
pub use status_events::StatusEvent;
pub(crate) use status_events::{EVENT_SOURCE_EMAIL, EVENT_SOURCE_USER};
// The aggregate's types and size caps (see `model`) and the answer-list helpers
// (see `answers`) keep resolving at `crate::applications::X` exactly as before
// the split.
pub(crate) use answers::normalize_question;
pub(crate) use model::{clamp_to_bytes, MAX_JOB_DESCRIPTION_BYTES};
pub use model::{
    make_application_id, Application, ApplicationMeta, ApplicationOrigin, ApplicationStatus,
};

/// Hard cap on the total number of `answers` entries [`ApplicationStore::merge_answers`]
/// / [`ApplicationStore::merge_answers_by_question`] will store per
/// application. `answers.save`'s per-call cap
/// (`extension_bridge::answers_save::MAX_ANSWERS_PER_CALL`) only bounds one
/// capture; this bounds the CUMULATIVE total across every capture on the same
/// application, so repeated captures (or a hostile/buggy collector called many
/// times) can't grow the stored list unboundedly. `pub(crate)` so
/// `extension_bridge`'s tests can seed right up to the cap without
/// duplicating the literal.
pub(crate) const MAX_TOTAL_ANSWERS: usize = 500;

/// 2026-06-11T00:00:00Z ms: a day before PR #359 (Applications) merged, so
/// `created_at` earlier is provably LEGACY (create-on-miss is correct); at/
/// after it, TREATED as modern — misclassifying legacy only strands it from
/// `find_for_job`, while the reverse RESURRECTS a deletion. Still relinked
/// later by `link_orphaned_generations`; only CREATING one is refused.
///
/// **Second line of defence, not the fix**: a genuinely pre-epoch row stays
/// pre-epoch forever, so this alone cannot stop create → delete → FK detaches
/// to NULL → still pre-epoch and unlinked → recreated. `migrations.rs`'s
/// one-shot backfill marker is what closes that; this just keeps a
/// never-backfilled legacy row from misclassifying as modern before the
/// marker exists.
const APPLICATIONS_FEATURE_EPOCH_MS: u64 = 1_781_136_000_000;

pub struct ApplicationStore {
    conn: Mutex<Connection>,
}

impl ApplicationStore {
    /// Open `applications.db`, run migrations, then run the lookup-only orphan
    /// link (every boot) and the one-time legacy backfill from the sibling
    /// `ai_generations.db` (own doc on [`Self::backfill_from_generations`]).
    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("applications.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, migrations::MIGRATIONS)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        // Both touch the SEPARATE ai_generations.db. ORDER IS LOAD-BEARING:
        // lookup-only `link_orphaned_generations` (own doc, safe every boot)
        // runs FIRST — reversed, the wide backfill's unconditional scan
        // resolves every row first, leaving nothing for the lookup-only pass.
        // Reordering alone does NOT stop resurrecting a deleted Application —
        // only `backfill_from_generations` running exactly once (own doc)
        // closes that.
        // `{}`/`e.code()`, never `{e}` — a `rusqlite::Error::InvalidPath` embeds
        // the offending path in its `Display`, which `AppError::from` preserves,
        // so interpolating `e` directly here would leak an absolute path into
        // the log (repo path-privacy rule). `.code()` is a fixed, path-free
        // category string, exactly what it exists for (own doc on `AppError::code`).
        if let Err(e) = store.link_orphaned_generations(data_dir) {
            log::warn!(
                "[applications] orphaned-generation link skipped (non-fatal): {}",
                e.code()
            );
        }
        // Pre-ADR-0001 legacy backfill: no Application ever existed for these.
        // No-ops immediately once its one-shot marker is set (own doc).
        if let Err(e) = store.backfill_from_generations(data_dir) {
            // Non-fatal: a backfill failure must never block app boot. Worst case
            // the table is empty and the user re-derives via the normal flow.
            log::warn!("[applications] backfill skipped (non-fatal): {}", e.code());
        }
        Ok(store)
    }

    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM applications", []).ok();
        conn.execute("DELETE FROM status_events", []).ok();
    }
}
