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

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ai_generations::ApplicationAnswer;
use crate::data_store::DataStore;
use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db};
use crate::error::{AppError, AppResult};

mod contact;
mod job_url;
mod migrations;
mod reminders;

// Re-exported so `crate::applications::normalize_job_url` keeps resolving after
// the split (see `job_url` — a verbatim move, no behaviour change).
pub use job_url::normalize_job_url;
// Same for the reminder projection the scheduler imports (see `reminders`).
pub use reminders::FollowUpCandidate;

/// The user-mutable lifecycle of an [`Application`].
///
/// **Future-proof:** [`ApplicationStatus::from_id`] never hard-rejects an unknown
/// string (a newer build, or an imported backup, may carry a stage this build
/// doesn't know) — it falls back to [`ApplicationStatus::Saved`] rather than
/// erroring. The ordered registry the shared-TS `APPLICATION_STAGES` mirrors is
/// [`ApplicationStatus::ALL`]; a parity test pins the two together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplicationStatus {
    /// Pre-apply: bookmarked from a posting, not yet applied. The only pre-apply
    /// stage, and the only status that does NOT mark a job "applied".
    Saved,
    Applied,
    Screening,
    Interviewing,
    Offer,
    Accepted,
    Rejected,
    Ghosted,
    Withdrawn,
}

impl ApplicationStatus {
    /// The ordered stage registry — the single Rust-side source of truth that the
    /// shared-TS `APPLICATION_STAGES` ids mirror (drift fails a parity test).
    pub const ALL: &'static [ApplicationStatus] = &[
        ApplicationStatus::Saved,
        ApplicationStatus::Applied,
        ApplicationStatus::Screening,
        ApplicationStatus::Interviewing,
        ApplicationStatus::Offer,
        ApplicationStatus::Accepted,
        ApplicationStatus::Rejected,
        ApplicationStatus::Ghosted,
        ApplicationStatus::Withdrawn,
    ];

    /// The camelCase wire id (matches the serde `rename_all` form + the TS union).
    pub fn as_id(self) -> &'static str {
        match self {
            ApplicationStatus::Saved => "saved",
            ApplicationStatus::Applied => "applied",
            ApplicationStatus::Screening => "screening",
            ApplicationStatus::Interviewing => "interviewing",
            ApplicationStatus::Offer => "offer",
            ApplicationStatus::Accepted => "accepted",
            ApplicationStatus::Rejected => "rejected",
            ApplicationStatus::Ghosted => "ghosted",
            ApplicationStatus::Withdrawn => "withdrawn",
        }
    }

    /// Parse a stored/wire id. **Never fails** — an unknown variant (a stage a
    /// newer build wrote, or a typo in an imported bundle) maps to the safe
    /// default `Saved` so the row stays usable instead of crashing a load.
    pub fn from_id(s: &str) -> ApplicationStatus {
        match s {
            "saved" => ApplicationStatus::Saved,
            "applied" => ApplicationStatus::Applied,
            "screening" => ApplicationStatus::Screening,
            "interviewing" => ApplicationStatus::Interviewing,
            "offer" => ApplicationStatus::Offer,
            "accepted" => ApplicationStatus::Accepted,
            "rejected" => ApplicationStatus::Rejected,
            "ghosted" => ApplicationStatus::Ghosted,
            "withdrawn" => ApplicationStatus::Withdrawn,
            _ => ApplicationStatus::Saved,
        }
    }

    /// Terminal = the pursuit is closed and would not normally reopen. `ghosted`
    /// is intentionally **soft**-terminal (treated as reopenable), so it is
    /// excluded here — a ghosted pursuit can still revive.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            ApplicationStatus::Accepted
                | ApplicationStatus::Rejected
                | ApplicationStatus::Withdrawn
        )
    }

    /// Pre-apply = the user has NOT applied yet. Only `saved` qualifies; it is also
    /// the sole status that leaves a found job's `applied` badge off.
    pub fn is_pre_apply(self) -> bool {
        matches!(self, ApplicationStatus::Saved)
    }
}

/// How an [`Application`] first came into being — the creation trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationOrigin {
    /// "Save" on the Jobs/discovery page → `saved`.
    Saved,
    /// Apply / Generate (résumé/cover) flow → `applied`.
    Generate,
    /// Manually tracked by the user (the `/applications` page) → `applied`.
    Manual,
    /// Backfilled from a pre-split `ai_generations` row → `applied`.
    Backfill,
}

/// Metadata describing the job an [`Application`] targets — passed to
/// [`ApplicationStore::upsert_for_origin`] by every creation trigger. Each field
/// is merged (non-empty wins) so separate actions on one URL layer onto a single
/// aggregate instead of clobbering each other.
#[derive(Debug, Clone, Default)]
pub struct ApplicationMeta {
    pub company: String,
    pub title: String,
    pub candidate: String,
    pub brief: String,
    pub job_description: String,
    /// Merged by QUESTION rather than wholesale-replaced like the scalar
    /// fields above — see [`ApplicationStore::merge_answers_by_question`]
    /// (this struct's merge path, used by every in-app writer) vs
    /// [`ApplicationStore::merge_answers`] (the extension's separate
    /// append-only capture path). A non-empty `answers` here (re)writes
    /// matching questions and adds new ones; it never drops an existing
    /// answer for a question this call doesn't mention.
    pub answers: Vec<ApplicationAnswer>,
    pub job_summary: String,
    /// Scraped salary range (Adzuna only, today) — grounds the salary application
    /// answer before it falls back to a web lookup. `None` when unknown.
    pub salary_min: Option<f64>,
    pub salary_max: Option<f64>,
    /// ISO-4217 currency for `salary_min`/`salary_max`.
    pub salary_currency: Option<String>,
}

/// Server-side cap on a stored job description, in BYTES. Mirrors the renderer
/// Zod cap in packages/shared/src/schemas/index.ts — client validation is UX-only;
/// this store write is the real trust boundary (the extension import path persists
/// attacker-influenced page HTML, which never passes through the Zod schema).
// ponytail: matches the renderer Zod cap; client validation is UX-only — the Rust store is the real boundary.
// `pub(crate)` so the IPC command layer (`commands::applications`) can reject an
// oversized description up-front against the SAME cap the store clamps to, instead
// of hardcoding a second literal.
pub(crate) const MAX_JOB_DESCRIPTION_BYTES: usize = 200_000;

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

/// Clamp `s` to at most `max` bytes, cutting on a UTF-8 char boundary so the
/// result is always valid UTF-8. Truncate (never reject): over-cap input is
/// clamped, not dropped.
///
/// Shared by every IPC entry point that accepts free text, so an untrusted
/// caller can't hand the backend unbounded work. Zod's `.max()` on the request
/// schema is renderer-side only — serde does not enforce it — so the cap has to
/// exist on this side too.
pub(crate) fn clamp_to_bytes(mut s: String, max: usize) -> String {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s
}

/// Clamp a job description to at most `MAX_JOB_DESCRIPTION_BYTES` bytes.
fn clamp_job_description(jd: String) -> String {
    clamp_to_bytes(jd, MAX_JOB_DESCRIPTION_BYTES)
}

/// The aggregate root. Owns identity, status, the job link, and the audit fields
/// moved off `ai_generations` (company/title/candidate/answers/brief) plus the
/// new user-facing tracking fields (notes/next_action/comp/contact).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub id: String,
    pub status: ApplicationStatus,
    /// First time the status left `saved` (became `applied`+). `None` while still
    /// `saved`. ms since epoch.
    pub applied_at: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
    /// Normalized job URL (see [`normalize_job_url`]). Empty for a manual,
    /// link-less pursuit; non-empty values are the dedup key.
    pub job_url: String,
    pub board: String,
    pub company: String,
    pub title: String,
    pub candidate: String,
    pub answers: Vec<ApplicationAnswer>,
    pub brief: String,
    #[serde(default)]
    pub job_description: String,
    #[serde(default)]
    pub notes: String,
    /// A user-set reminder timestamp (ms) for the next thing to do. `None` = unset.
    pub next_action_at: Option<u64>,
    /// Epoch-ms of the follow-up notification already raised for the CURRENT
    /// [`Application::next_action_at`]; `None` = not yet announced. Cleared by
    /// [`ApplicationStore::update_fields`] whenever the due date moves, so one
    /// reminder notifies exactly once per due date.
    ///
    /// Backend bookkeeping the renderer never renders — but it IS on the wire so
    /// it survives a backup: [`DataStore::export`] serializes the aggregate, and
    /// dropping this field meant restoring a backup re-announced every reminder
    /// the user had already seen.
    #[serde(default)]
    pub next_action_notified_at: Option<u64>,
    #[serde(default)]
    pub comp: String,
    /// **The** primary contact for this pursuit (recruiter / hiring manager /
    /// apply-by-email recipient — one person, one field). Canonical: this is the
    /// only contact pair the store reads or writes.
    #[serde(default)]
    pub contact_name: String,
    /// Canonical primary contact email — see [`Application::contact_name`].
    #[serde(default)]
    pub contact_email: String,
    #[serde(default)]
    pub job_summary: String,
    /// **Deprecated alias** of [`Application::contact_name`], kept on the wire so
    /// existing renderer/extension callers keep working. Reads always mirror the
    /// canonical value (see [`row_to_application`]); writes fold onto the
    /// canonical pair (see [`ApplicationStore::update_fields`] /
    /// [`Application::canonicalize_contact`]). The `recipient_*` COLUMNS still
    /// exist in SQLite (migrations are additive-only) but are no longer read or
    /// written.
    #[serde(default)]
    pub recipient_name: String,
    /// **Deprecated alias** of [`Application::contact_email`] — see
    /// [`Application::recipient_name`].
    #[serde(default)]
    pub recipient_email: String,
    /// Scraped salary range (Adzuna only, today) — grounds the salary application
    /// answer before it falls back to a web lookup. `None` when unknown, or on an
    /// Application persisted before this field existed.
    #[serde(default)]
    pub salary_min: Option<f64>,
    #[serde(default)]
    pub salary_max: Option<f64>,
    /// ISO-4217 currency for `salary_min`/`salary_max`.
    #[serde(default)]
    pub salary_currency: Option<String>,
}

/// One append-only status-history row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    pub application_id: String,
    /// Empty for the seed event of a freshly-created Application.
    pub from_status: String,
    pub to_status: String,
    pub at: u64,
    #[serde(default)]
    pub note: String,
}

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

    /// Forward-safe, idempotent, **lookup-only** backfill: link every orphaned
    /// `ai_generations` row (non-empty `job_url`, `application_id` still NULL)
    /// to its Application, keyed by normalized `job_url` — the SAME key
    /// [`Self::upsert_for_origin`] merges new saves on. Returns the number of
    /// rows actually linked, so a caller (and a test) can tell a real backfill
    /// from a no-op one.
    ///
    /// **Deliberately never creates an Application — the reason this is a
    /// SEPARATE function from [`Self::backfill_from_generations`], not a
    /// second pass through it.** That backfill predates ADR-0001 entirely: for
    /// those rows no Application ever existed anywhere, so creating one is the
    /// correct — the only possible — repair. The rows THIS backfill targets are
    /// different in kind: every staged-pipeline run is launched FROM an
    /// Application's own page, so the Application for an orphaned row almost
    /// always already exists, created independently of the generation save
    /// that failed to link to it (see `commands::resume_pipeline::mod::
    /// persist_document`'s own doc for how that save came to omit the FK). A
    /// row this lookup cannot match therefore means one of two things, and
    /// BOTH must leave `application_id` NULL rather than guess:
    ///
    /// * the user has since deleted that Application — creating one here would
    ///   RESURRECT it, which is exactly the defect a lookup-only backfill
    ///   avoids (an upsert-style backfill would recreate it silently, and the
    ///   user would never know their delete was undone);
    /// * the row is a genuinely unlinked generation with no Application to
    ///   find (an id-less/unsaved apply attempt, a stale/renamed url).
    ///
    /// A WRONG link is worse than a missing one: it would put someone's résumé
    /// under an unrelated Application and delete it along with THAT
    /// Application the next time the user deletes it. `Self::find_by_job_url`
    /// is the existing READ-ONLY primitive for exactly this — the same one
    /// `extension_bridge`'s `applied.check` handler uses because it must never
    /// fetch-or-write either.
    ///
    /// **Safe to re-run, and re-run on every boot on purpose** (same posture
    /// as `backfill_from_generations`): the `WHERE application_id IS NULL`
    /// selection is itself the idempotency guard — a row this backfill already
    /// linked is excluded from the next run's scan, and a row it could not
    /// match stays eligible, so a later boot picks it up the moment its
    /// Application exists (e.g. the user re-generates, re-creating the link
    /// through the ordinary save path).
    ///
    /// No new schema migration: `AiGenerationStore`'s own `add_application_id`
    /// migration already added the column and its index long before this
    /// backfill was written, and this is DATA repair, not a schema change — so
    /// there is nothing to append to either store's position-indexed
    /// `MIGRATIONS` array for it.
    fn link_orphaned_generations(&self, data_dir: &Path) -> AppResult<usize> {
        let gen_path = data_dir.join("ai_generations.db");
        if !gen_path.exists() {
            return Ok(0); // fresh install — nothing to link
        }
        let gen_conn = crate::db::open(&gen_path)?;
        if !crate::db::column_exists(&gen_conn, "ai_generations", "application_id") {
            // AiGenerationStore has never run its own migrations on this
            // install yet — nothing to backfill against.
            return Ok(0);
        }

        let mut stmt = gen_conn.prepare(
            "SELECT id, job_url FROM ai_generations \
             WHERE job_url != '' AND (application_id IS NULL OR application_id = '')",
        )?;
        let orphans: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);

        let mut linked = 0usize;
        for (gen_id, job_url) in orphans {
            let normalized = normalize_job_url(&job_url);
            // Read-only lookup — see this function's own doc for why an
            // upsert is never the right call here.
            let Some(app) = self.find_by_job_url(&normalized) else {
                continue; // no resolvable Application — stays NULL, never guessed
            };
            gen_conn.execute(
                "UPDATE ai_generations SET application_id = ?2 WHERE id = ?1",
                params![gen_id, app.id],
            )?;
            linked += 1;
        }
        Ok(linked)
    }

    // ── Reads ─────────────────────────────────────────────────────────────────

    pub fn list(&self) -> Vec<Application> {
        let conn = self.conn.lock();
        conn.prepare(&format!("{SELECT_COLS} ORDER BY updated_at DESC"))
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], row_to_application)
                    .ok()
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
    }

    pub fn get(&self, id: &str) -> Option<Application> {
        let conn = self.conn.lock();
        conn.prepare(&format!("{SELECT_COLS} WHERE id = ?1"))
            .ok()
            .and_then(|mut stmt| stmt.query_row(params![id], row_to_application).ok())
    }

    /// History for one Application, oldest-first.
    pub fn events(&self, id: &str) -> Vec<StatusEvent> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT application_id, from_status, to_status, at, note
             FROM status_events WHERE application_id = ?1 ORDER BY at ASC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(params![id], |row| {
                Ok(StatusEvent {
                    application_id: row.get(0)?,
                    from_status: row.get(1)?,
                    to_status: row.get(2)?,
                    at: ts_from_db(row.get::<_, i64>(3)?),
                    note: row.get(4)?,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    /// Most-recent Application for a normalized url, if any — the row a per-job
    /// upsert merges into so one job keeps a single aggregate. `pub(crate)` so
    /// `extension_bridge`'s `applied.check` handler can run the same read-only
    /// lookup (it never fetches or writes — see `resolve_applied_check`).
    pub(crate) fn find_by_job_url(&self, normalized: &str) -> Option<Application> {
        if normalized.is_empty() {
            return None;
        }
        let conn = self.conn.lock();
        conn.prepare(&format!(
            "{SELECT_COLS} WHERE job_url = ?1 ORDER BY created_at DESC LIMIT 1"
        ))
        .ok()
        .and_then(|mut stmt| stmt.query_row(params![normalized], row_to_application).ok())
    }

    /// Normalized non-empty urls of Applications that are NOT `saved` — the set
    /// that derives a found job's `applied` flag (was: "a generation exists").
    pub fn applied_job_urls(&self) -> std::collections::HashSet<String> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT DISTINCT job_url FROM applications WHERE job_url != '' AND status != 'saved'",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(0))
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    // ── Writes ────────────────────────────────────────────────────────────────

    /// The single dedup/merge entry point for all four creation triggers
    /// (Save→`saved`, Apply/Generate→`applied`, Manual→`applied`, Backfill).
    /// Normalizes `job_url`, merges meta onto any existing Application for that
    /// url, and returns the (new or existing) Application id.
    ///
    /// `applied`: `Some(true)` forces `applied`; `Some(false)`/`None` defers to the
    /// origin (Save stays `saved`; Generate/Manual/Backfill apply).
    pub fn upsert_for_origin(
        &self,
        job_url: &str,
        board: &str,
        meta: &ApplicationMeta,
        origin: ApplicationOrigin,
        applied: Option<bool>,
    ) -> AppResult<String> {
        let normalized = normalize_job_url(job_url);
        let origin_applies = matches!(
            origin,
            ApplicationOrigin::Generate | ApplicationOrigin::Manual | ApplicationOrigin::Backfill
        );
        let applied_at = if applied == Some(true) || origin_applies {
            Some(now_ms())
        } else {
            None
        };
        // The Rust store is the real trust boundary: clamp the JD here so BOTH the
        // import funnel and direct IPC callers are capped (the renderer Zod cap is
        // UX-only and the import path never passes through it). Truncate, never drop.
        let clamped_jd = clamp_job_description(meta.job_description.clone());
        self.upsert_internal(&normalized, board, meta, &clamped_jd, applied_at)
    }

    /// Core upsert: `normalized` is already normalized; `applied_at` Some marks the
    /// Application `applied`, None keeps it `saved`. Merges into an existing row
    /// when the (non-empty) url already has an Application; only ever advances OUT
    /// of `saved`, never demotes an applied+ row. `answers` merge by QUESTION via
    /// [`Self::merge_answers_by_question`] — not a wholesale replace.
    fn upsert_internal(
        &self,
        normalized: &str,
        board: &str,
        meta: &ApplicationMeta,
        // Already clamped by the caller (`upsert_for_origin`) to
        // `MAX_JOB_DESCRIPTION_BYTES`; both write branches below use this, never
        // `meta.job_description`, so the cap can't be bypassed.
        clamped_jd: &str,
        applied_at: Option<u64>,
    ) -> AppResult<String> {
        let pick = |inc: &str, ex: &str| -> String {
            if inc.trim().is_empty() {
                ex.to_string()
            } else {
                inc.to_string()
            }
        };

        // Lookup + write share ONE lock/transaction (`row_by_job_url_conn`, not
        // self-locking `find_by_job_url`) — the old separately-released lookup
        // lock left a gap where a concurrent `merge_answers` commit got clobbered.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;

        if let Some(existing) = Self::row_by_job_url_conn(&tx, normalized)? {
            let now = now_ms();
            let (status, new_applied_at) = if existing.status.is_pre_apply() && applied_at.is_some()
            {
                (
                    ApplicationStatus::Applied,
                    applied_at.or(existing.applied_at),
                )
            } else {
                (existing.status, existing.applied_at)
            };
            let answers =
                Self::merge_answers_by_question(existing.answers.clone(), meta.answers.clone());
            let app = Application {
                id: existing.id.clone(),
                status,
                applied_at: new_applied_at,
                created_at: existing.created_at,
                updated_at: now,
                job_url: normalized.to_string(),
                board: pick(board, &existing.board),
                company: pick(&meta.company, &existing.company),
                title: pick(&meta.title, &existing.title),
                candidate: pick(&meta.candidate, &existing.candidate),
                answers,
                brief: pick(&meta.brief, &existing.brief),
                job_description: pick(clamped_jd, &existing.job_description),
                notes: existing.notes.clone(),
                next_action_at: existing.next_action_at,
                // Carried through untouched: a re-scrape/re-track must never
                // re-arm a reminder the user has already been notified about.
                next_action_notified_at: existing.next_action_notified_at,
                comp: existing.comp.clone(),
                contact_name: existing.contact_name.clone(),
                contact_email: existing.contact_email.clone(),
                job_summary: pick(&meta.job_summary, &existing.job_summary),
                // Deprecated aliases: always the canonical value, never a
                // second source of truth (see `Application::recipient_name`).
                recipient_name: existing.contact_name.clone(),
                recipient_email: existing.contact_email.clone(),
                // COALESCE(new, old): a re-scrape/re-track fills salary the first
                // time it becomes known, but never clobbers an already-known value
                // with an unknown (`None`) one.
                salary_min: meta.salary_min.or(existing.salary_min),
                salary_max: meta.salary_max.or(existing.salary_max),
                salary_currency: meta
                    .salary_currency
                    .clone()
                    .or_else(|| existing.salary_currency.clone()),
            };
            // Row write + status event share the transaction opened above.
            Self::write_row_conn(&tx, &app)?;
            if status != existing.status {
                Self::append_event_conn(
                    &tx,
                    &app.id,
                    existing.status.as_id(),
                    status.as_id(),
                    "",
                    now,
                )?;
            }
            tx.commit()?;
            return Ok(app.id);
        }

        let now = now_ms();
        let status = if applied_at.is_some() {
            ApplicationStatus::Applied
        } else {
            ApplicationStatus::Saved
        };
        let app = Application {
            id: make_application_id(),
            status,
            applied_at,
            created_at: now,
            updated_at: now,
            job_url: normalized.to_string(),
            board: board.to_string(),
            company: meta.company.clone(),
            title: meta.title.clone(),
            candidate: meta.candidate.clone(),
            // Route the new-row branch through the same capped+deduped merge as an
            // existing-row update (against an empty existing list) so a caller
            // handing `upsert_for_origin` an oversized/duplicate-question
            // `meta.answers` on FIRST creation can't bypass `MAX_TOTAL_ANSWERS` the
            // way storing `meta.answers` verbatim did.
            answers: Self::merge_answers_by_question(Vec::new(), meta.answers.clone()),
            brief: meta.brief.clone(),
            job_description: clamped_jd.to_string(),
            notes: String::new(),
            next_action_at: None,
            next_action_notified_at: None,
            comp: String::new(),
            contact_name: String::new(),
            contact_email: String::new(),
            job_summary: meta.job_summary.clone(),
            recipient_name: String::new(),
            recipient_email: String::new(),
            salary_min: meta.salary_min,
            salary_max: meta.salary_max,
            salary_currency: meta.salary_currency.clone(),
        };
        // New row: its seed status event shares the same transaction.
        Self::write_row_conn(&tx, &app)?;
        Self::append_event_conn(&tx, &app.id, "", status.as_id(), "", now)?;
        tx.commit()?;
        Ok(app.id)
    }

    /// Manual create from the `/applications` page. Optional url; everything else
    /// from `meta`. Always `applied` (a hand-tracked pursuit already applied to).
    pub fn track_manual(
        &self,
        job_url: &str,
        board: &str,
        meta: &ApplicationMeta,
    ) -> AppResult<String> {
        self.upsert_for_origin(job_url, board, meta, ApplicationOrigin::Manual, Some(true))
    }

    /// Transition an Application's status, appending one history event and bumping
    /// `updated_at`. Sets `applied_at` the first time it leaves `saved`.
    pub fn set_status(&self, id: &str, to: ApplicationStatus, note: &str) -> AppResult<()> {
        // The READ joins the same transaction as the two writes. The row UPDATE
        // and its append-only status event must land together or not at all —
        // otherwise a crash between them leaves the status changed with no
        // history row — and reading through a separately-released lock let a
        // concurrent transition land in the gap, so `from_status` below recorded
        // a status this row no longer had.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        let existing = Self::row_by_id_conn(&tx, id)?
            .ok_or_else(|| AppError::Validation(format!("application not found: {id}")))?;
        let now = now_ms();
        let applied_at = if existing.applied_at.is_none() && !to.is_pre_apply() {
            Some(now)
        } else {
            existing.applied_at
        };
        tx.execute(
            "UPDATE applications SET status = ?2, applied_at = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, to.as_id(), applied_at.map(ts_to_db), ts_to_db(now)],
        )?;
        tx.execute(
            "INSERT INTO status_events (application_id, from_status, to_status, at, note)
             VALUES (?1,?2,?3,?4,?5)",
            params![id, existing.status.as_id(), to.as_id(), ts_to_db(now), note],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Atomic compare-and-set: transition an Application's status ONLY if its
    /// CURRENT status is exactly `from` — the read-check-write happens under
    /// ONE lock/transaction (`UPDATE ... WHERE id=? AND status=?`), unlike a
    /// caller doing its own `.get()` status check and then calling
    /// [`Self::set_status`] (which re-locks separately and writes
    /// unconditionally) — that pattern can lose a race between the check and
    /// the write. `pub(crate)`: the extension bridge's `status.update` guard is
    /// the first caller (`extension_bridge::status_update::resolve_status_update`).
    ///
    /// Returns `Ok(true)` iff exactly one row matched `from` and was
    /// transitioned (with its status event appended, same transaction — an
    /// event-insert failure propagates and rolls the whole transaction back,
    /// so the status flip and its history row always commit or roll back
    /// together); `Ok(false)` when zero rows matched (no such id, or its
    /// status had already moved off `from` since the caller last checked —
    /// never a partial write). Mirrors `set_status`'s field semantics for the
    /// matched row: `updated_at` always bumps; `applied_at` is
    /// first-applied-wins (only set when currently `NULL`) whenever `to` is
    /// not pre-apply — a `saved` row CAN already carry a prior `applied_at`
    /// from an earlier applied -> saved demotion via the stage picker, and
    /// that timestamp must survive a re-transition back to `applied`; the
    /// event's `note` defaults to `""` when `None`.
    pub(crate) fn transition_status_if(
        &self,
        id: &str,
        from: ApplicationStatus,
        to: ApplicationStatus,
        note: Option<&str>,
    ) -> AppResult<bool> {
        let now = now_ms();
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        let rows = if !to.is_pre_apply() {
            tx.execute(
                "UPDATE applications SET status = ?2, applied_at = COALESCE(applied_at, ?3), updated_at = ?4
                 WHERE id = ?1 AND status = ?5",
                params![id, to.as_id(), ts_to_db(now), ts_to_db(now), from.as_id()],
            )?
        } else {
            tx.execute(
                "UPDATE applications SET status = ?2, updated_at = ?3 WHERE id = ?1 AND status = ?4",
                params![id, to.as_id(), ts_to_db(now), from.as_id()],
            )?
        };
        if rows == 0 {
            tx.commit()?;
            return Ok(false);
        }
        Self::append_event_conn(&tx, id, from.as_id(), to.as_id(), note.unwrap_or(""), now)?;
        tx.commit()?;
        Ok(true)
    }

    /// Append newly-captured extension answers onto Application `id`'s answer
    /// list — an APPEND-only dedup merge, `pub(crate)` so the extension
    /// bridge's `answers.save` handler
    /// (`extension_bridge::answers_save::resolve_answers_save`) is the first
    /// caller. Deliberately independent of
    /// [`Self::merge_answers_by_question`] (`upsert_internal`'s meta-merge
    /// path, used by every in-app writer — `ai_generations_save`, manual
    /// edits, legacy-generation backfill): that path lets `incoming` win for
    /// a matching question, because those writers legitimately re-save
    /// EDITED text for a question the user already answered. THIS method
    /// must never do that — a stray/duplicate extension re-capture of the
    /// same page must never clobber an answer the user already reviewed —
    /// so here the EXISTING answer always wins and only genuinely new
    /// questions are appended.
    ///
    /// Dedup key: the NORMALIZED question text (trim + lowercase + collapse
    /// internal whitespace runs) compared against the CURRENT answers — an
    /// existing answer for a given question always wins and is NEVER
    /// overwritten, so a re-capture of the same page only ever adds
    /// genuinely new questions. The dedup set also accumulates across
    /// `incoming` itself, so two same-normalized entries in one call collapse
    /// to a single added answer rather than two. A blank (post-trim) question
    /// is dropped.
    ///
    /// The dedup READ and the WRITE happen in the SAME transaction (via
    /// [`Self::row_by_id_conn`], not the self-locking [`Self::get`] — calling
    /// `get` here would deadlock the non-reentrant `parking_lot::Mutex`), so
    /// there is no earlier separate read a concurrent caller could race
    /// against; only `answers` + `updated_at` are touched. Returns the count
    /// of NEWLY ADDED answers (`0` when every captured question was already
    /// present or blank).
    ///
    /// Capped at [`MAX_TOTAL_ANSWERS`] merged answers per application: once
    /// that many are stored, further incoming entries are dropped rather than
    /// appended (they count toward the caller's `skipped`, same as a dedup
    /// hit — never rejected outright).
    pub(crate) fn merge_answers(
        &self,
        id: &str,
        incoming: Vec<ApplicationAnswer>,
    ) -> AppResult<usize> {
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;

        let existing = Self::row_by_id_conn(&tx, id)?
            .ok_or_else(|| AppError::Validation(format!("application not found: {id}")))?;

        let mut seen: std::collections::HashSet<String> = existing
            .answers
            .iter()
            .map(|a| normalize_question(&a.question))
            .collect();

        let mut merged = existing.answers;
        let mut added = 0usize;
        for ans in incoming {
            if merged.len() >= MAX_TOTAL_ANSWERS {
                break; // per-application cap hit — remaining entries count as skipped
            }
            let key = normalize_question(&ans.question);
            if key.is_empty() || !seen.insert(key) {
                continue; // blank question, or an existing answer already wins
            }
            merged.push(ApplicationAnswer {
                id: make_answer_id(),
                question: ans.question,
                answer: ans.answer,
            });
            added += 1;
        }

        if added > 0 {
            // `?` (not `.unwrap_or_else(|_| "[]".into())`): a serialize failure
            // must abort the transaction (never committed, so the existing
            // `answers` column is untouched) rather than writing an empty `[]`
            // that would silently wipe every previously-stored answer.
            let answers_json = serde_json::to_string(&merged)?;
            tx.execute(
                "UPDATE applications SET answers = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, answers_json, ts_to_db(now_ms())],
            )?;
        }
        tx.commit()?;
        Ok(added)
    }

    /// Merge `incoming` onto `existing` by NORMALIZED question text —
    /// `upsert_internal`'s `answers` merge path, used on every in-app
    /// writer's re-upsert (`ai_generations_save`'s AI-generated answer set,
    /// `track_manual`, the legacy-generation backfill). Unlike
    /// [`Self::merge_answers`] (the extension's separate append-only
    /// capture path, where an EXISTING answer always wins), here `incoming`
    /// wins for a matching question: `ai_generations_save` re-saves the
    /// CURRENT full answer set on every call, including in-app edits to a
    /// question the user already answered, and "existing wins" would
    /// silently discard that edit. Existing answers for a question NOT
    /// present in `incoming` are preserved untouched — this is what fixes
    /// the previous wholesale-replace data-loss hazard, where a non-empty
    /// `meta.answers` simply became the whole stored list, dropping every
    /// answer another writer (e.g. the extension's `answers.save`) had
    /// appended in between. An empty `incoming` is a no-op. Same
    /// [`MAX_TOTAL_ANSWERS`] cap as `merge_answers`, but the cap only ever
    /// blocks a genuinely NEW question: a same-question replacement is a
    /// swap for an entry already removed from `merged` below, so it is
    /// always applied regardless of the cap — otherwise a legacy row that
    /// already sits at/over the cap (seeded before this cap existed) would
    /// have the matching existing answer removed to make room, then the cap
    /// check block the incoming replacement from ever being pushed back in,
    /// making the question vanish entirely instead of being rewritten.
    fn merge_answers_by_question(
        existing: Vec<ApplicationAnswer>,
        incoming: Vec<ApplicationAnswer>,
    ) -> Vec<ApplicationAnswer> {
        if incoming.is_empty() {
            return existing;
        }
        let existing_keys: std::collections::HashSet<String> = existing
            .iter()
            .map(|a| normalize_question(&a.question))
            .collect();
        let incoming_keys: std::collections::HashSet<String> = incoming
            .iter()
            .map(|a| normalize_question(&a.question))
            .filter(|k| !k.is_empty())
            .collect();
        // Existing answers survive as-is unless `incoming` carries a
        // (re)answer for the same question — those are dropped here and
        // replaced below.
        let mut merged: Vec<ApplicationAnswer> = existing
            .into_iter()
            .filter(|a| !incoming_keys.contains(&normalize_question(&a.question)))
            .collect();

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for ans in incoming {
            let key = normalize_question(&ans.question);
            if key.is_empty() || !seen.insert(key.clone()) {
                continue; // blank question, or a duplicate within this same incoming batch
            }
            // A same-question replacement always wins (see doc comment above);
            // only a brand-new question is subject to the cap. `continue`
            // (not `break`) so a later replacement in this same batch still
            // gets applied even after a run of new-question entries hit it.
            if !existing_keys.contains(&key) && merged.len() >= MAX_TOTAL_ANSWERS {
                continue; // per-application cap — this new-question entry is dropped
            }
            merged.push(ans);
        }
        merged
    }

    /// Patch the user-editable tracking fields. Each `None` leaves its field
    /// unchanged; bumps `updated_at` whenever called.
    ///
    /// **Contact unification:** `recipient_name`/`recipient_email` are deprecated
    /// aliases of `contact_name`/`contact_email` (see
    /// [`Application::recipient_name`]) — both inbound names patch the SAME
    /// canonical storage. When a caller sends both, the canonical one wins.
    ///
    /// **Reminders:** changing (or clearing) `next_action_at` drops the
    /// `next_action_notified_at` dedupe marker in the same transaction, so a
    /// rescheduled follow-up notifies again exactly once.
    #[allow(clippy::too_many_arguments)]
    pub fn update_fields(
        &self,
        id: &str,
        notes: Option<String>,
        next_action_at: Option<Option<u64>>,
        comp: Option<String>,
        contact_name: Option<String>,
        contact_email: Option<String>,
        job_description: Option<String>,
        job_summary: Option<String>,
        recipient_name: Option<String>,
        recipient_email: Option<String>,
    ) -> AppResult<()> {
        // ONE lock/transaction spans lookup + write + the marker clear
        // (`row_by_id_conn`, not self-locking `get`): the write rewrites EVERY
        // column, so releasing the lock between them let a commit be lost, and
        // the marker clear must land with the row or not at all.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        let existing = Self::row_by_id_conn(&tx, id)?
            .ok_or_else(|| AppError::Validation(format!("application not found: {id}")))?;
        let next_action_at = next_action_at.unwrap_or(existing.next_action_at);
        let reminder_rescheduled = next_action_at != existing.next_action_at;
        // Canonical pair wins over the deprecated alias when both are supplied;
        // otherwise whichever one was supplied patches it; absent in both leaves
        // the stored value alone.
        let contact_name = contact_name
            .or(recipient_name)
            .unwrap_or(existing.contact_name);
        let contact_email = contact_email
            .or(recipient_email)
            .unwrap_or(existing.contact_email);
        // A new (or cleared) due date is a NEW reminder — forget that the old one
        // was already announced so the scheduler can fire once for it.
        let next_action_notified_at = if reminder_rescheduled {
            None
        } else {
            existing.next_action_notified_at
        };
        let app = Application {
            notes: notes.unwrap_or(existing.notes),
            next_action_at,
            next_action_notified_at,
            comp: comp.unwrap_or(existing.comp),
            // Clamp Some(s) at the store boundary; None still preserves the stored JD
            // (the IPC arg is attacker-reachable and bypasses the renderer Zod cap).
            job_description: job_description
                .map(clamp_job_description)
                .unwrap_or(existing.job_description),
            job_summary: job_summary.unwrap_or(existing.job_summary),
            recipient_name: contact_name.clone(),
            recipient_email: contact_email.clone(),
            contact_name,
            contact_email,
            updated_at: now_ms(),
            ..existing
        };
        Self::write_row_conn(&tx, &app)?;
        tx.commit()?;
        Ok(())
    }

    /// Delete an Application and its status history. `keep_documents` is consumed
    /// at the command layer (it decides whether child generations are also
    /// deleted); this store owns only the Application + its events, so the flag is
    /// accepted for a uniform signature and does not change the row deletion here.
    pub fn delete(&self, id: &str, _keep_documents: bool) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM applications WHERE id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM status_events WHERE application_id = ?1",
            params![id],
        )?;
        Ok(())
    }

    // ── Internals ─────────────────────────────────────────────────────────────

    /// Connection-scoped row write, callable inside a transaction. `conn` may be a
    /// plain `&Connection` or a `&Transaction` (which derefs to `&Connection`).
    fn write_row_conn(conn: &Connection, app: &Application) -> AppResult<()> {
        let answers_json = serde_json::to_string(&app.answers).unwrap_or_else(|_| "[]".into());
        let job_summary = truncate_on_char_boundary(&app.job_summary, MAX_JOB_SUMMARY_BYTES);
        // The deprecated `recipient_name`/`recipient_email` columns are NOT in
        // this statement: the canonical pair is `contact_name`/`contact_email`
        // (migration `unify_application_contact`). Omitting them means a new row
        // gets their `DEFAULT ''` and an existing row keeps its pre-unification
        // value untouched — additive, never destructive.
        //
        // `next_action_notified_at` IS written, from the struct — so every
        // caller must carry the existing value forward. Rust enforces that
        // mechanically: `Application` is built by exhaustive struct literals, so
        // a new write path cannot compile without deciding what the marker is.
        conn.execute(
            "INSERT INTO applications
                (id, status, applied_at, created_at, updated_at, job_url, board,
                 company, title, candidate, answers, brief, notes, next_action_at,
                 comp, contact_name, contact_email, job_description, job_summary,
                 salary_min, salary_max, salary_currency, next_action_notified_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)
             ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                applied_at = excluded.applied_at,
                updated_at = excluded.updated_at,
                job_url = excluded.job_url,
                board = excluded.board,
                company = excluded.company,
                title = excluded.title,
                candidate = excluded.candidate,
                answers = excluded.answers,
                brief = excluded.brief,
                notes = excluded.notes,
                next_action_at = excluded.next_action_at,
                comp = excluded.comp,
                contact_name = excluded.contact_name,
                contact_email = excluded.contact_email,
                job_description = excluded.job_description,
                job_summary = excluded.job_summary,
                salary_min = excluded.salary_min,
                salary_max = excluded.salary_max,
                salary_currency = excluded.salary_currency,
                next_action_notified_at = excluded.next_action_notified_at",
            params![
                app.id,
                app.status.as_id(),
                app.applied_at.map(ts_to_db),
                ts_to_db(app.created_at),
                ts_to_db(app.updated_at),
                app.job_url,
                app.board,
                app.company,
                app.title,
                app.candidate,
                answers_json,
                app.brief,
                app.notes,
                app.next_action_at.map(ts_to_db),
                app.comp,
                app.contact_name,
                app.contact_email,
                app.job_description,
                job_summary,
                app.salary_min,
                app.salary_max,
                app.salary_currency,
                app.next_action_notified_at.map(ts_to_db),
            ],
        )?;
        Ok(())
    }

    /// Connection-scoped single-row read by id, callable inside an existing
    /// lock/transaction — unlike [`Self::get`] (which takes its OWN lock;
    /// calling `get` while already holding `self.conn.lock()` would deadlock
    /// the non-reentrant `parking_lot::Mutex`). Used by
    /// [`Self::merge_answers`] so its dedup read and its write land in the
    /// exact same transaction.
    fn row_by_id_conn(conn: &Connection, id: &str) -> AppResult<Option<Application>> {
        use rusqlite::OptionalExtension;
        let mut stmt = conn.prepare(&format!("{SELECT_COLS} WHERE id = ?1"))?;
        Ok(stmt.query_row(params![id], row_to_application).optional()?)
    }

    /// `find_by_job_url` sibling for [`Self::upsert_internal`] (self-locking here would deadlock).
    fn row_by_job_url_conn(conn: &Connection, normalized: &str) -> AppResult<Option<Application>> {
        use rusqlite::OptionalExtension;
        if normalized.is_empty() {
            return Ok(None);
        }
        let sql = format!("{SELECT_COLS} WHERE job_url = ?1 ORDER BY created_at DESC LIMIT 1");
        let mut stmt = conn.prepare(&sql)?;
        Ok(stmt
            .query_row(params![normalized], row_to_application)
            .optional()?)
    }

    /// Connection-scoped status-event append, callable inside a transaction.
    /// Propagates an insert failure (`?`) rather than swallowing it — every
    /// caller runs this inside the SAME transaction as its row write, so an
    /// event-insert error rolls the whole transaction back on drop instead of
    /// leaving a status flip with no history row.
    fn append_event_conn(
        conn: &Connection,
        id: &str,
        from: &str,
        to: &str,
        note: &str,
        at: u64,
    ) -> AppResult<()> {
        conn.execute(
            "INSERT INTO status_events (application_id, from_status, to_status, at, note)
             VALUES (?1,?2,?3,?4,?5)",
            params![id, from, to, ts_to_db(at), note],
        )?;
        Ok(())
    }
}

/// Server-side hard cap on a persisted job summary. The Zod `.max(50_000)` on the
/// IPC schema is UX-only — this is the real bound, applied in the store write path
/// so the IPC and any future import path are both protected.
// ponytail: byte-cap + char-boundary truncate; raise the const if summaries grow.
const MAX_JOB_SUMMARY_BYTES: usize = 50_000;

/// Column projection shared by `list`/`get`/`find_by_job_url` so order lives once.
///
/// The deprecated `recipient_name`/`recipient_email` COLUMNS are deliberately
/// absent: since `unify_application_contact` the canonical pair is
/// `contact_name`/`contact_email`, and the struct's alias fields are mirrored
/// from it in [`row_to_application`].
const SELECT_COLS: &str = "SELECT id, status, applied_at, created_at, updated_at, job_url, board,
            company, title, candidate, answers, brief, notes, next_action_at,
            comp, contact_name, contact_email, job_description, job_summary,
            salary_min, salary_max, salary_currency, next_action_notified_at
     FROM applications";

fn row_to_application(row: &rusqlite::Row) -> rusqlite::Result<Application> {
    let status_raw: String = row.get(1)?;
    let answers_json: String = row.get(10)?;
    // Canonical contact pair, read once and mirrored onto the deprecated
    // `recipient_*` alias fields below (see `Application::recipient_name`).
    let contact_name: String = row.get(15)?;
    let contact_email: String = row.get(16)?;
    Ok(Application {
        id: row.get(0)?,
        status: ApplicationStatus::from_id(&status_raw),
        applied_at: row.get::<_, Option<i64>>(2)?.map(ts_from_db),
        created_at: ts_from_db(row.get::<_, i64>(3)?),
        updated_at: ts_from_db(row.get::<_, i64>(4)?),
        job_url: row.get(5)?,
        board: row.get(6)?,
        company: row.get(7)?,
        title: row.get(8)?,
        candidate: row.get(9)?,
        answers: serde_json::from_str(&answers_json).unwrap_or_default(),
        brief: row.get(11)?,
        notes: row.get(12)?,
        next_action_at: row.get::<_, Option<i64>>(13)?.map(ts_from_db),
        next_action_notified_at: row.get::<_, Option<i64>>(22)?.map(ts_from_db),
        comp: row.get(14)?,
        job_description: row.get(17)?,
        job_summary: row.get(18)?,
        recipient_name: contact_name.clone(),
        recipient_email: contact_email.clone(),
        contact_name,
        contact_email,
        // NULL (unknown salary, e.g. a pre-migration row) → None, never 0.
        salary_min: row.get(19)?,
        salary_max: row.get(20)?,
        salary_currency: row.get(21)?,
    })
}

/// Truncate `s` to at most `max_bytes`, never splitting a UTF-8 char.
fn truncate_on_char_boundary(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

pub fn make_application_id() -> String {
    format!("app-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
}

/// Fresh id for an answer merged in by [`ApplicationStore::merge_answers`] —
/// same shape as [`make_application_id`].
fn make_answer_id() -> String {
    format!("ans-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
}

/// Normalize a question for dedup comparison in [`ApplicationStore::merge_answers`]
/// (and reused by `extension_bridge::answers_suggest`'s matcher): trim, lowercase,
/// collapse whitespace — "Why  this role?" and "why this role?" dedup to one key.
pub(crate) fn normalize_question(q: &str) -> String {
    q.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

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
            // Seed one event so an imported Application still carries a history row.
            Self::append_event_conn(
                &tx,
                &app.id,
                "",
                app.status.as_id(),
                "imported",
                app.updated_at,
            )?;
        }
        tx.commit()?;
        Ok(apps.len())
    }
}

#[cfg(test)]
mod test;
