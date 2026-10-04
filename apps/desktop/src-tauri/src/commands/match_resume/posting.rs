//! Resolving a cached posting for scoring: its JD text blob, the facts the
//! hard-constraint pass compares against, and the identity fields a save needs.
//! Split out of `commands/match_resume.rs` for R8 (issue #1280);
//! `match_resume.rs` re-exports the crate-visible items, so each keeps its
//! `commands::match_resume::<name>` path.

use parking_lot::Mutex;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::constraints;
use crate::postings::PostingsCache;

/// Build a searchable text blob for a single cached posting JSON value (title +
/// description + requirements). Pure — no lock — so it can be reused for both the
/// single-job and batch lookups. Returns None if the posting has no usable text.
pub(super) fn posting_to_text(posting: &Value) -> Option<String> {
    let title = posting.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let description = posting.get("description").and_then(|v| v.as_str());
    // `requirements` is an array of strings; collect to a Vec the shared helper
    // can borrow as a slice.
    let requirements: Option<Vec<String>> = posting
        .get("requirements")
        .and_then(|v| v.as_array())
        .map(|reqs| {
            reqs.iter()
                .filter_map(|r| r.as_str().map(|s| s.to_string()))
                .collect()
        });
    crate::documents::keywords::posting_text_blob(title, description, requirements.as_deref())
}

/// Build a searchable text blob for a cached job posting (title + description +
/// requirements). Returns None if the posting isn't in the live cache.
///
/// `pub(crate)` so the agent tools reuse the same posting → text resolution instead
/// of re-deriving it.
pub(crate) fn job_text_for(app: &AppHandle, job_id: &str) -> Option<String> {
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock();
    let posting = guard
        .get_all()
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(job_id))?;
    posting_to_text(posting)
}

/// Everything [`match_resume`] needs from the live posting cache, under ONE lock
/// and ONE linear scan: the JD blob the scoring kernel consumes, and the
/// posting-side facts the hard-constraint pass compares against.
///
/// Split out from [`job_text_for`] (whose other callers want only the text)
/// rather than letting `constraints` take the lock again for itself. The
/// duplicate scan was not free in the place it ran: the constraint verdict is
/// deliberately recomputed even on a `match_scores` cache HIT — see the call
/// site — so on the Jobs page, the path where the score costs nothing, a second
/// full scan of the cache would have run on every single call.
pub(super) fn job_text_and_posting_facts(
    app: &AppHandle,
    job_id: &str,
) -> (Option<String>, constraints::PostingFacts) {
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock();
    resolve_posting(
        guard
            .get_all()
            .iter()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(job_id)),
    )
}

/// The pure half of [`job_text_and_posting_facts`]: what a resolved (or missing)
/// cached posting yields for each consumer.
///
/// Split out so the hand-off itself is testable without an `AppHandle`.
/// Replacing the facts here with a default is invisible to every other test —
/// the constraint pass would silently report on an empty posting forever — which
/// is exactly the shape that already bit this feature once at the command's tail
/// expression. `posting_facts_hand_off_carries_the_real_posting` pins it.
pub(super) fn resolve_posting(
    posting: Option<&Value>,
) -> (Option<String>, constraints::PostingFacts) {
    match posting {
        Some(p) => (posting_to_text(p), constraints::posting_facts_from_value(p)),
        // No such posting: `score_one` returns its job-not-found error, and
        // `attach` passes that through untouched, so these facts are never read.
        None => (None, constraints::PostingFacts::default()),
    }
}

/// Identity fields of a cached posting: the company/title/url/board an
/// application aggregate needs when a document is saved for it. All loaded
/// server-side by id (mirrors [`job_text_for`]'s single-lock lookup) — the model
/// never supplies these, so a prompt-injected posting can't spoof the target of a
/// save. Returns `None` when the posting isn't in the live cache.
#[derive(Debug, Clone, Default)]
pub(crate) struct JobPostingMeta {
    pub company: String,
    pub title: String,
    pub url: String,
    pub board: String,
}

/// `pub(crate)` so `commands::resume_pipeline` resolves the same posting
/// identity the rest of the app uses, instead of re-deriving it.
pub(crate) fn job_meta_for(app: &AppHandle, job_id: &str) -> Option<JobPostingMeta> {
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock();
    let posting = guard
        .get_all()
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(job_id))?;
    let field = |k: &str| {
        posting
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    Some(JobPostingMeta {
        company: field("company"),
        title: field("title"),
        url: field("url"),
        // `JobPosting` serializes the originating board under `source`.
        board: field("source"),
    })
}
