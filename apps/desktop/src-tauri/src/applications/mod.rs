//! Application — the status-bearing aggregate root for a job pursuit.
//!
//! Per ADR `docs/adr/0001-application-aggregate-split.md`, an **Application** is
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
use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::{AppError, AppResult};

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

/// Clamp a job description to at most `MAX_JOB_DESCRIPTION_BYTES` bytes, cutting on
/// a UTF-8 char boundary so the stored text is always valid UTF-8. Truncate (never
/// reject): an over-cap import is clamped, not dropped.
fn clamp_job_description(mut jd: String) -> String {
    if jd.len() <= MAX_JOB_DESCRIPTION_BYTES {
        return jd;
    }
    let mut end = MAX_JOB_DESCRIPTION_BYTES;
    while end > 0 && !jd.is_char_boundary(end) {
        end -= 1;
    }
    jd.truncate(end);
    jd
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
    #[serde(default)]
    pub comp: String,
    #[serde(default)]
    pub contact_name: String,
    #[serde(default)]
    pub contact_email: String,
    #[serde(default)]
    pub job_summary: String,
    /// Employer-side contact name for a direct "apply by email" approach.
    /// Distinct from the applicant's own `contact_name` on the Application.
    #[serde(default)]
    pub recipient_name: String,
    /// Employer-side contact email for a direct "apply by email" approach.
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

pub struct ApplicationStore {
    conn: Mutex<Connection>,
}

impl ApplicationStore {
    const MIGRATIONS: &'static [Migration] = &[
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
    ];

    /// Open `applications.db`, run migrations, then run the one-time backfill from
    /// the sibling `ai_generations.db` (idempotent — safe on every boot).
    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("applications.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        // The ai_generations FK column + the backfill touch the SEPARATE
        // ai_generations.db, so they can't be ordinary applications.db migrations.
        // Run them here, guarded so a re-run is a no-op.
        if let Err(e) = store.backfill_from_generations(data_dir) {
            // Non-fatal: a backfill failure must never block app boot. Worst case
            // the table is empty and the user re-derives via the normal flow.
            log::warn!("[applications] backfill skipped (non-fatal): {e}");
        }
        Ok(store)
    }

    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM applications", []).ok();
        conn.execute("DELETE FROM status_events", []).ok();
    }

    /// Forward-safe, idempotent backfill from the pre-split `ai_generations.db`:
    ///
    /// 1. `ALTER TABLE ai_generations ADD COLUMN application_id` (+ index) — guarded
    ///    by [`crate::db::column_exists`] so a re-run is a no-op.
    /// 2. For each generation, ensure exactly one `Application(status=applied,
    ///    applied_at=created_at)` keyed by `normalize(job_url)` (empty url → its own
    ///    uuid so link-less generations each get a distinct Application), link the
    ///    gen's `application_id`, and seed one status event.
    ///
    /// Re-running is safe: a gen that already carries a non-empty `application_id`
    /// is skipped, and Applications are looked up by normalized url before insert,
    /// so no duplicates accrue.
    fn backfill_from_generations(&self, data_dir: &Path) -> AppResult<()> {
        let gen_path = data_dir.join("ai_generations.db");
        if !gen_path.exists() {
            return Ok(()); // fresh install — nothing to migrate
        }
        let gen_conn = crate::db::open(&gen_path)?;

        // Step 1: add the FK column + index if absent (idempotent guard).
        if !crate::db::column_exists(&gen_conn, "ai_generations", "application_id") {
            gen_conn.execute_batch(
                "ALTER TABLE ai_generations ADD COLUMN application_id TEXT;
                 CREATE INDEX IF NOT EXISTS idx_ai_generations_application_id
                     ON ai_generations(application_id);",
            )?;
        }

        struct GenRow {
            id: String,
            created_at: u64,
            candidate: String,
            title: String,
            company: String,
            job_url: String,
            board: String,
            answers: Vec<ApplicationAnswer>,
            brief: String,
            application_id: Option<String>,
        }

        let mut stmt = gen_conn.prepare(
            "SELECT id, created_at, candidate_name, job_title, company_name,
                    job_url, board, application_answers, company_brief, application_id
             FROM ai_generations
             ORDER BY created_at ASC",
        )?;
        let rows: Vec<GenRow> = stmt
            .query_map([], |row| {
                let answers_json: String = row.get(7)?;
                let app_id: Option<String> = row.get(9).unwrap_or(None);
                Ok(GenRow {
                    id: row.get(0)?,
                    created_at: ts_from_db(row.get::<_, i64>(1)?),
                    candidate: row.get(2)?,
                    title: row.get(3)?,
                    company: row.get(4)?,
                    job_url: row.get(5)?,
                    board: row.get(6)?,
                    answers: serde_json::from_str(&answers_json).unwrap_or_default(),
                    brief: row.get(8)?,
                    application_id: app_id.filter(|s| !s.is_empty()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        for g in rows {
            if g.application_id.is_some() {
                continue; // already linked — idempotent skip
            }
            let normalized = normalize_job_url(&g.job_url);
            let meta = ApplicationMeta {
                company: g.company,
                title: g.title,
                candidate: g.candidate,
                brief: g.brief,
                job_description: String::new(), // ponytail: legacy generations carry no JD column
                answers: g.answers,
                job_summary: String::new(),
                salary_min: None,
                salary_max: None,
                salary_currency: None,
            };
            // Empty url → a fresh per-gen Application (no shared key to merge on);
            // non-empty url → merge into that url's Application if one exists.
            let app_id = self.upsert_internal(
                &normalized,
                &g.board,
                &meta,
                &meta.job_description,
                Some(g.created_at),
            )?;
            gen_conn.execute(
                "UPDATE ai_generations SET application_id = ?2 WHERE id = ?1",
                params![g.id, app_id],
            )?;
        }
        Ok(())
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

        if let Some(existing) = self.find_by_job_url(normalized) {
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
                comp: existing.comp.clone(),
                contact_name: existing.contact_name.clone(),
                contact_email: existing.contact_email.clone(),
                job_summary: pick(&meta.job_summary, &existing.job_summary),
                recipient_name: existing.recipient_name.clone(),
                recipient_email: existing.recipient_email.clone(),
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
            // Row write + the (conditional) status event in ONE transaction so an
            // upsert that changes status can never persist the row without its
            // history event. `.transaction()` needs `&mut Connection`.
            let mut guard = self.conn.lock();
            let tx = guard.transaction()?;
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
            answers: meta.answers.clone(),
            brief: meta.brief.clone(),
            job_description: clamped_jd.to_string(),
            notes: String::new(),
            next_action_at: None,
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
        // New Application: the row + its seed status event in ONE transaction.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
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
        let existing = self
            .get(id)
            .ok_or_else(|| AppError::Validation(format!("application not found: {id}")))?;
        let now = now_ms();
        let applied_at = if existing.applied_at.is_none() && !to.is_pre_apply() {
            Some(now)
        } else {
            existing.applied_at
        };
        // The row UPDATE and its append-only status event must land together or
        // not at all — otherwise a crash between them leaves the status changed
        // with no history row (or vice-versa). One transaction; `.transaction()`
        // needs `&mut Connection`, so call on `&mut *guard`.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
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
    /// [`MAX_TOTAL_ANSWERS`] cap as `merge_answers`; entries beyond the cap
    /// are dropped.
    fn merge_answers_by_question(
        existing: Vec<ApplicationAnswer>,
        incoming: Vec<ApplicationAnswer>,
    ) -> Vec<ApplicationAnswer> {
        if incoming.is_empty() {
            return existing;
        }
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
            if merged.len() >= MAX_TOTAL_ANSWERS {
                break; // per-application cap — remaining incoming entries are dropped
            }
            let key = normalize_question(&ans.question);
            if key.is_empty() || !seen.insert(key) {
                continue; // blank question, or a duplicate within this same incoming batch
            }
            merged.push(ans);
        }
        merged
    }

    /// Patch the user-editable tracking fields. Each `None` leaves its field
    /// unchanged; bumps `updated_at` whenever called.
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
        let existing = self
            .get(id)
            .ok_or_else(|| AppError::Validation(format!("application not found: {id}")))?;
        let app = Application {
            notes: notes.unwrap_or(existing.notes),
            next_action_at: next_action_at.unwrap_or(existing.next_action_at),
            comp: comp.unwrap_or(existing.comp),
            contact_name: contact_name.unwrap_or(existing.contact_name),
            contact_email: contact_email.unwrap_or(existing.contact_email),
            // Clamp Some(s) at the store boundary; None still preserves the stored JD
            // (the IPC arg is attacker-reachable and bypasses the renderer Zod cap).
            job_description: job_description
                .map(clamp_job_description)
                .unwrap_or(existing.job_description),
            job_summary: job_summary.unwrap_or(existing.job_summary),
            recipient_name: recipient_name.unwrap_or(existing.recipient_name),
            recipient_email: recipient_email.unwrap_or(existing.recipient_email),
            updated_at: now_ms(),
            ..existing
        };
        self.write_row(&app)
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

    fn write_row(&self, app: &Application) -> AppResult<()> {
        let conn = self.conn.lock();
        Self::write_row_conn(&conn, app)
    }

    /// Connection-scoped row write, callable inside a transaction. `conn` may be a
    /// plain `&Connection` or a `&Transaction` (which derefs to `&Connection`).
    fn write_row_conn(conn: &Connection, app: &Application) -> AppResult<()> {
        let answers_json = serde_json::to_string(&app.answers).unwrap_or_else(|_| "[]".into());
        let job_summary = truncate_on_char_boundary(&app.job_summary, MAX_JOB_SUMMARY_BYTES);
        conn.execute(
            "INSERT INTO applications
                (id, status, applied_at, created_at, updated_at, job_url, board,
                 company, title, candidate, answers, brief, notes, next_action_at,
                 comp, contact_name, contact_email, job_description, job_summary,
                 recipient_name, recipient_email, salary_min, salary_max, salary_currency)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24)
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
                recipient_name = excluded.recipient_name,
                recipient_email = excluded.recipient_email,
                salary_min = excluded.salary_min,
                salary_max = excluded.salary_max,
                salary_currency = excluded.salary_currency",
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
                app.recipient_name,
                app.recipient_email,
                app.salary_min,
                app.salary_max,
                app.salary_currency,
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
const SELECT_COLS: &str = "SELECT id, status, applied_at, created_at, updated_at, job_url, board,
            company, title, candidate, answers, brief, notes, next_action_at,
            comp, contact_name, contact_email, job_description, job_summary,
            recipient_name, recipient_email, salary_min, salary_max, salary_currency
     FROM applications";

fn row_to_application(row: &rusqlite::Row) -> rusqlite::Result<Application> {
    let status_raw: String = row.get(1)?;
    let answers_json: String = row.get(10)?;
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
        comp: row.get(14)?,
        contact_name: row.get(15)?,
        contact_email: row.get(16)?,
        job_description: row.get(17)?,
        job_summary: row.get(18)?,
        recipient_name: row.get(19)?,
        recipient_email: row.get(20)?,
        // NULL (unknown salary, e.g. a pre-migration row) → None, never 0.
        salary_min: row.get(21)?,
        salary_max: row.get(22)?,
        salary_currency: row.get(23)?,
    })
}

/// Extract an explicit URL scheme (the `scheme:` prefix per RFC 3986§3.1) if
/// one is present, lowercased. A scheme is `ALPHA *( ALPHA / DIGIT / "+" / "-" /
/// "." )` immediately followed by `:`, and it MUST appear before any `/`, `?`, or
/// `#` — so `javascript:alert(1)` and `data:text/html,…` are schemes, but a
/// scheme-less `host/path?x=a:b` (colon in the path/query) is not. Used to reject
/// dangerous schemes; returns `None` for scheme-less input.
fn explicit_scheme(input: &str) -> Option<String> {
    // Only the authority-less head, before the first path/query/fragment delimiter,
    // can carry a scheme. This keeps a `:` inside a path or query from looking like one.
    let head = input.split(['/', '?', '#']).next().unwrap_or(input);
    let (candidate, _) = head.split_once(':')?;
    if candidate.is_empty() {
        return None;
    }
    let mut chars = candidate.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    Some(candidate.to_ascii_lowercase())
}

/// Normalize a job URL into a stable dedup key: lowercase host, strip a leading
/// `www.`, drop the fragment (`#…`), retain only per-host *identifying* query
/// params (e.g. Indeed `jk`) while dropping every other query param (utm_*, ref,
/// tracking), and trim a trailing `/`. The scheme is preserved (lowercased). Empty
/// input returns empty.
///
/// Security chokepoint: an input carrying an explicit scheme other than
/// `http`/`https` (e.g. `javascript:`, `data:`, `file:`, `vbscript:`, `blob:`) is
/// neutralized to an empty string — i.e. "no url" — so an import-borne or
/// manually-entered payload can never be stored as an openable link. Scheme-less
/// input and `http(s)` keep their exact prior normalization.
///
/// No existing centralized URL normalizer was found in `net`/`scraping` (only
/// host-only helpers like `contact_profile::host_of`), so this is the single
/// owner for Application url identity.
pub fn normalize_job_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Reject dangerous explicit schemes at the single backend chokepoint. Only
    // `http`/`https` may round-trip; any other explicit scheme yields "no url".
    if let Some(scheme) = explicit_scheme(trimmed) {
        if scheme != "http" && scheme != "https" {
            return String::new();
        }
    }
    let lower = trimmed.to_lowercase();
    let (scheme, rest) = match lower.split_once("://") {
        Some((s, r)) => (Some(s.to_string()), r.to_string()),
        None => (None, lower.clone()),
    };
    // Drop the fragment (`#…`) unconditionally, then split the query off the path so
    // per-host identifying params can be selectively retained below.
    let no_frag = rest.split('#').next().unwrap_or(&rest);
    let (path_part, query) = match no_frag.split_once('?') {
        Some((p, q)) => (p, q),
        None => (no_frag, ""),
    };
    let (host, path) = match path_part.split_once('/') {
        Some((h, p)) => (h.to_string(), Some(p.to_string())),
        None => (path_part.to_string(), None),
    };
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    // Keep ONLY the host's identifying query params (utm_*, ref, … are dropped);
    // hosts with no allowlist entry drop the whole query, exactly as before.
    let retained_query = retain_identifying_params(&host, query);
    let mut out = String::new();
    if let Some(s) = scheme {
        out.push_str(&s);
        out.push_str("://");
    }
    out.push_str(&host);
    if let Some(p) = path {
        let p = p.trim_end_matches('/');
        if !p.is_empty() {
            out.push('/');
            out.push_str(p);
        }
    }
    if !retained_query.is_empty() {
        out.push('?');
        out.push_str(&retained_query);
    }
    out
}

/// Per-host allowlist of *identifying* query params that must survive normalization
/// (every other query param — utm_*, ref, tracking — is dropped, and hosts absent
/// here drop the entire query). Keep in sync with the canonical URL builders in
/// `scraping::scrape_url` that place a job id in the query string: currently only
/// Indeed (`/viewjob?jk=<id>`). LinkedIn et al. put the id in the PATH, so they need
/// no entry here and normalize exactly as before.
fn identifying_query_params(host: &str) -> &'static [&'static str] {
    if host == "indeed.com" || host.ends_with(".indeed.com") {
        &["jk"]
    } else {
        &[]
    }
}

/// Rebuild the query string keeping only `identifying_query_params(host)`, emitted
/// in the allowlist's own fixed order so the input param ordering can never change
/// the dedup key. A param with an empty value is skipped. Returns "" when nothing is
/// retained (the common, path-based case).
fn retain_identifying_params(host: &str, query: &str) -> String {
    let allow = identifying_query_params(host);
    if allow.is_empty() || query.is_empty() {
        return String::new();
    }
    allow
        .iter()
        .filter_map(|key| {
            query.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                (k == *key && !v.is_empty()).then(|| format!("{key}={v}"))
            })
        })
        .collect::<Vec<_>>()
        .join("&")
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

/// Normalize a question for dedup comparison in
/// [`ApplicationStore::merge_answers`]: trim, lowercase, and collapse
/// internal whitespace runs to a single space — so "Why  this role?" and
/// "why this role?" (a different capture pass / incidental whitespace) dedup
/// to the same key.
fn normalize_question(q: &str) -> String {
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
        let apps: Vec<Application> = items
            .iter()
            .map(|item| serde_json::from_value(item.clone()).map_err(AppError::from))
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
