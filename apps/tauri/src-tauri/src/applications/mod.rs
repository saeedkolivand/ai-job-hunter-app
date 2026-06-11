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
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ai_generations::ApplicationAnswer;
use crate::data_store::DataStore;
use crate::db::{run_migrations, Migration};
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
    pub answers: Vec<ApplicationAnswer>,
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
    pub notes: String,
    /// A user-set reminder timestamp (ms) for the next thing to do. `None` = unset.
    pub next_action_at: Option<u64>,
    #[serde(default)]
    pub comp: String,
    #[serde(default)]
    pub contact_name: String,
    #[serde(default)]
    pub contact_email: String,
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
    ];

    /// Open `applications.db`, run migrations, then run the one-time backfill from
    /// the sibling `ai_generations.db` (idempotent — safe on every boot).
    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("applications.db");
        let conn = Connection::open(&path)?;
        run_migrations(&conn, Self::MIGRATIONS)?;
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
        let gen_conn = Connection::open(&gen_path)?;

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
                    created_at: row.get::<_, i64>(1)? as u64,
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
                answers: g.answers,
            };
            // Empty url → a fresh per-gen Application (no shared key to merge on);
            // non-empty url → merge into that url's Application if one exists.
            let app_id = self.upsert_internal(&normalized, &g.board, &meta, Some(g.created_at))?;
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
                    at: row.get::<_, i64>(3)? as u64,
                    note: row.get(4)?,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    /// Most-recent Application for a normalized url, if any — the row a per-job
    /// upsert merges into so one job keeps a single aggregate.
    fn find_by_job_url(&self, normalized: &str) -> Option<Application> {
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
        self.upsert_internal(&normalized, board, meta, applied_at)
    }

    /// Core upsert: `normalized` is already normalized; `applied_at` Some marks the
    /// Application `applied`, None keeps it `saved`. Merges into an existing row
    /// when the (non-empty) url already has an Application; only ever advances OUT
    /// of `saved`, never demotes an applied+ row.
    fn upsert_internal(
        &self,
        normalized: &str,
        board: &str,
        meta: &ApplicationMeta,
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
            let answers = if meta.answers.is_empty() {
                existing.answers.clone()
            } else {
                meta.answers.clone()
            };
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
                notes: existing.notes.clone(),
                next_action_at: existing.next_action_at,
                comp: existing.comp.clone(),
                contact_name: existing.contact_name.clone(),
                contact_email: existing.contact_email.clone(),
            };
            self.write_row(&app)?;
            if status != existing.status {
                self.append_event(&app.id, existing.status.as_id(), status.as_id(), "", now);
            }
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
            notes: String::new(),
            next_action_at: None,
            comp: String::new(),
            contact_name: String::new(),
            contact_email: String::new(),
        };
        self.write_row(&app)?;
        self.append_event(&app.id, "", status.as_id(), "", now);
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
        {
            let conn = self.conn.lock();
            conn.execute(
                "UPDATE applications SET status = ?2, applied_at = ?3, updated_at = ?4 WHERE id = ?1",
                params![id, to.as_id(), applied_at.map(|v| v as i64), now as i64],
            )?;
        }
        self.append_event(id, existing.status.as_id(), to.as_id(), note, now);
        Ok(())
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
        let answers_json = serde_json::to_string(&app.answers).unwrap_or_else(|_| "[]".into());
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO applications
                (id, status, applied_at, created_at, updated_at, job_url, board,
                 company, title, candidate, answers, brief, notes, next_action_at,
                 comp, contact_name, contact_email)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
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
                contact_email = excluded.contact_email",
            params![
                app.id,
                app.status.as_id(),
                app.applied_at.map(|v| v as i64),
                app.created_at as i64,
                app.updated_at as i64,
                app.job_url,
                app.board,
                app.company,
                app.title,
                app.candidate,
                answers_json,
                app.brief,
                app.notes,
                app.next_action_at.map(|v| v as i64),
                app.comp,
                app.contact_name,
                app.contact_email,
            ],
        )?;
        Ok(())
    }

    fn append_event(&self, id: &str, from: &str, to: &str, note: &str, at: u64) {
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT INTO status_events (application_id, from_status, to_status, at, note)
             VALUES (?1,?2,?3,?4,?5)",
            params![id, from, to, at as i64, note],
        );
    }
}

/// Column projection shared by `list`/`get`/`find_by_job_url` so order lives once.
const SELECT_COLS: &str = "SELECT id, status, applied_at, created_at, updated_at, job_url, board,
            company, title, candidate, answers, brief, notes, next_action_at,
            comp, contact_name, contact_email
     FROM applications";

fn row_to_application(row: &rusqlite::Row) -> rusqlite::Result<Application> {
    let status_raw: String = row.get(1)?;
    let answers_json: String = row.get(10)?;
    Ok(Application {
        id: row.get(0)?,
        status: ApplicationStatus::from_id(&status_raw),
        applied_at: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
        created_at: row.get::<_, i64>(3)? as u64,
        updated_at: row.get::<_, i64>(4)? as u64,
        job_url: row.get(5)?,
        board: row.get(6)?,
        company: row.get(7)?,
        title: row.get(8)?,
        candidate: row.get(9)?,
        answers: serde_json::from_str(&answers_json).unwrap_or_default(),
        brief: row.get(11)?,
        notes: row.get(12)?,
        next_action_at: row.get::<_, Option<i64>>(13)?.map(|v| v as u64),
        comp: row.get(14)?,
        contact_name: row.get(15)?,
        contact_email: row.get(16)?,
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
/// `www.`, drop the query (`?…`) and fragment (`#…`), and trim a trailing `/`.
/// The scheme is preserved (lowercased). Empty input returns empty.
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
    let no_qf = rest.split(['?', '#']).next().unwrap_or(&rest).to_string();
    let (host, path) = match no_qf.split_once('/') {
        Some((h, p)) => (h.to_string(), Some(p.to_string())),
        None => (no_qf.clone(), None),
    };
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
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
    out
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn make_application_id() -> String {
    format!("app-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
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
        self.clear_all();
        let mut count = 0;
        for item in items {
            let app: Application = serde_json::from_value(item.clone())?;
            self.write_row(&app)?;
            // Seed one event so an imported Application still carries a history row.
            self.append_event(&app.id, "", app.status.as_id(), "imported", app.updated_at);
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod test;
