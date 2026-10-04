//! The [`Application`] aggregate's types, and the size caps its writes enforce.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Nothing here touches SQLite: the status
//! vocabulary, the creation origins, the merge metadata, the wire shape, and the
//! byte caps that make the store the real trust boundary.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ai_generations::ApplicationAnswer;
use crate::db::now_ms;

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
pub(super) fn clamp_job_description(jd: String) -> String {
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

pub fn make_application_id() -> String {
    format!("app-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
}

#[cfg(test)]
mod tests;
