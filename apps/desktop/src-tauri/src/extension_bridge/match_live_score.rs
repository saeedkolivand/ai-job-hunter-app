//! Pure scoring primitives shared by `match_live`'s "Check fit" verb AND the import-time
//! best-effort score fill (`match_live_import_score`) — split from `match_live.rs` (R8 relief).
//! See that module's own doc for the full design (keyword-only guarantee, consent gate, cache-key
//! parity with `handle_import`).

use serde_json::Value;
use tauri::AppHandle;

use super::match_live::MatchLiveOk;
use crate::documents::{DocumentRecord, DocumentStore};
use crate::scraping::types::JobPosting;

/// Cap on the gap keywords sent over the wire — a JD can carry far more misses
/// than are useful in a popup chip list.
const MAX_GAPS: usize = 8;

/// Resolve which résumé to score: the `is_default` row, else the
/// most-recently-created one, else `None` when the user has no résumé at all.
/// `docs` is expected to be [`DocumentStore::list`]'s output, which is already
/// ordered `created_at DESC` — so the first entry IS the most recent; this
/// function does not re-sort. Pure — no `AppHandle`, no I/O — directly
/// unit-testable against a synthetic `Vec<DocumentRecord>`.
pub(super) fn resolve_resume(docs: &[DocumentRecord]) -> Option<&DocumentRecord> {
    docs.iter().find(|d| d.is_default).or_else(|| docs.first())
}

/// Parse a captured Scan-mode DOM into a searchable job-text blob — the SAME
/// extraction `handle_import`'s Scan-mode branch uses
/// ([`crate::scraping::scrape_url::parse_from_html`], which honors the
/// `data-ajh-job-root` hint `content.ts` marks) plus
/// [`crate::documents::keywords::posting_text_blob`] for the title/
/// description/requirements join. `None` when nothing usable parsed (a
/// blocked page / unrecognized markup) — the caller surfaces a fixed refusal,
/// never a panic.
pub(super) fn parse_job_text(url: &str, html: &str) -> Option<String> {
    let posting = crate::scraping::scrape_url::parse_from_html(url, html)?;
    posting_job_text(&posting)
}

/// Build the same ATS text blob [`parse_job_text`] does, directly from an
/// already-parsed [`JobPosting`] — shared by [`score_import_posting`], which
/// has a `JobPosting` in hand from `handle_import` and never re-parses HTML.
pub(super) fn posting_job_text(posting: &JobPosting) -> Option<String> {
    crate::documents::keywords::posting_text_blob(
        &posting.title,
        posting.description.as_deref(),
        posting.requirements.as_deref(),
    )
}

/// Score `resume` against `job_text`, keyword-only (ALWAYS — see the module
/// doc), caching under the ad-hoc `job_id`. Shared by [`resolve_match_live`]
/// (the popup's "Check fit" click) AND [`score_import_posting`] (the
/// `import.result.matchScore` fill), so the keyword-only decision and the
/// underlying [`crate::commands::match_resume::score_adhoc_keyword_only`] call
/// live in exactly one place. That callee hardcodes keyword-only AND
/// never-translate internally (no flags to pass here) — see its doc.
pub(super) async fn score_keyword_only(
    app: &AppHandle,
    store: &DocumentStore,
    resume: &DocumentRecord,
    job_id: &str,
    job_text: String,
) -> Value {
    let resume_raw_keywords = crate::commands::match_resume::parse_resume_keywords(resume);
    let active = store.embedding_config();
    crate::commands::match_resume::score_adhoc_keyword_only(
        app,
        store,
        resume,
        resume_raw_keywords.as_deref(),
        &active,
        job_id,
        job_text,
    )
    .await
}

/// Build the ad-hoc result-cache key for a normalized job url. One scheme
/// shared by [`resolve_match_live`] and [`score_import_posting`] so a
/// "Check fit" click and an import on the SAME page hit the SAME
/// self-invalidating `match_scores` row instead of scoring twice. Prefixed so
/// it can never collide with a real `PostingsCache` posting id (which never
/// carries this prefix).
pub(super) fn adhoc_job_id(normalized_url: &str) -> String {
    format!("adhoc:{}", crate::documents::sha256_hex(normalized_url))
}

/// Canonicalize + normalize a raw url the SAME way `handle_import` derives its
/// `normalized` variable (`canonical_job_url` then
/// [`crate::applications::normalize_job_url`]) — so a "Check fit" click and an
/// import on the SAME page compute the IDENTICAL [`adhoc_job_id`] and hit the
/// SAME `match_scores` row, regardless of which raw url variant (www /
/// trailing slash / tracking query params) each side happened to observe. Used
/// ONLY to derive the cache key — [`parse_job_text`] still parses the DOM
/// against the raw `url` untouched, mirroring `handle_import`'s Scan-mode
/// branch (which never rewrites the DOM-parse url either). Pure — no I/O — so
/// the cache-key parity is directly unit-testable against representative url
/// variants without a scoring round-trip.
pub(super) fn canonicalized_normalized_url(url: &str) -> String {
    let canonical = crate::scraping::scrape_url::canonical_job_url(url);
    let effective = canonical.as_deref().unwrap_or(url);
    crate::applications::normalize_job_url(effective)
}

/// Extract the wire-ready [`MatchLiveOk`] fields out of `score_one`'s raw
/// `Value` result (`combined`/`ats`/`gaps`), clamping `gaps` to [`MAX_GAPS`].
/// Pure — no `AppHandle`, no I/O — so the clamp is directly unit-testable
/// against a synthetic `Value` without a scoring round-trip.
pub(super) fn build_match_ok(result: &Value, resume_name: String) -> MatchLiveOk {
    let combined = result
        .get("combined")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let ats = result.get("ats").and_then(Value::as_f64).unwrap_or(0.0);
    let gaps: Vec<String> = result
        .get("gaps")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|g| g.as_str().map(str::to_string))
                .take(MAX_GAPS)
                .collect()
        })
        .unwrap_or_default();

    MatchLiveOk {
        combined,
        ats,
        gaps,
        resume_name,
        // Filled in by the caller (`resolve_match_live`) — this extraction has no salary context.
        salary_posting: None,
        salary_expectation: None,
    }
}

#[cfg(test)]
mod tests;
