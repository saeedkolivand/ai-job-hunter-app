//! Pure posting-resolution helpers for `import.request` — split from `import_flow.rs` (R8
//! relief): persistence + usability/merge decisions with no `AppHandle`, directly unit-testable
//! without a Tauri app. See `import_flow`'s own module doc for the whole import flow.

use crate::applications::{ApplicationMeta, ApplicationOrigin, ApplicationStore};
use crate::error::AppResult;

/// Persist a parsed [`crate::scraping::types::JobPosting`] from an import as a
/// Saved Application and return `(application_id, status_id)`. This is the
/// *entire* persistence side effect of an import: it touches the
/// [`ApplicationStore`] only and has **no access to the `PostingsCache`**, so
/// an import can never enter the Jobs/discovery feed. Split out of
/// [`handle_import`] (which needs an `AppHandle` for event/notification
/// plumbing) so the import → Application contract is unit-testable without a
/// Tauri app — see `import_flow_resolve/tests/persist.rs`.
pub(super) fn persist_import_application(
    store: &ApplicationStore,
    normalized_url: &str,
    posting: &crate::scraping::types::JobPosting,
    applied: Option<bool>,
) -> AppResult<(String, String)> {
    let meta = ApplicationMeta {
        company: posting.company.clone(),
        title: posting.title.clone(),
        job_description: posting.description.clone().unwrap_or_default(),
        ..Default::default()
    };
    let id = store.upsert_for_origin(
        normalized_url,
        &posting.source,
        &meta,
        ApplicationOrigin::Saved,
        applied,
    )?;
    let status = store
        .get(&id)
        .map(|a| a.status.as_id().to_string())
        .unwrap_or_else(|| "saved".to_string());
    Ok((id, status))
}

/// A posting is usable for an import only if it carries a real title; an
/// empty-title parse means the extractor degraded (blocked fetch / unknown page).
pub(super) fn usable(p: &crate::scraping::types::JobPosting) -> bool {
    !p.title.trim().is_empty()
}

/// Fill `resolve`'s title/description from the extension's `[data-ajh-job-root]`
/// HINT ONLY — used by the SPA/list-view (canonical) import branch when the
/// resolve came back unusable or description-less (LinkedIn's anonymous-fetch
/// authwall is the common trigger).
///
/// Deliberately narrower than a full DOM/`parse_from_html` merge: a list-shell
/// page (LinkedIn search/collections) commonly carries its OWN SEO
/// `JobPosting` JSON-LD for an unrelated job (the first list result), and
/// `parse_from_html`'s precedence lets JSON-LD override the hint — so calling
/// it on the whole shell document risks silently importing the wrong job. The
/// caller extracts via [`crate::scraping::scrape_url::job_root_generic_html`]
/// instead, which reads ONLY the hinted subtree, never the document's JSON-LD
/// /`__NEXT_DATA__`/whole-page heuristics.
///
/// `resolve`'s non-empty title/description win; a field it left empty is
/// filled from the hint — never the other way around. `company`/`location`
/// are untouched (the hint doesn't extract them — they stay whatever `resolve`
/// produced, including its own host-based company fallback). Returns `None`
/// when `resolve` is `None` — there is no base posting's identity
/// (id/url/source/company) to attach the hint to, so the stub/partial path
/// covers that case instead of synthesizing a whole posting from a
/// list-shell's hint alone. Pure — no `AppHandle`/network — so it's directly
/// unit-testable.
pub(super) fn merge_resolve_with_hint(
    resolve: Option<crate::scraping::types::JobPosting>,
    hint_title: String,
    hint_description: Option<String>,
) -> Option<crate::scraping::types::JobPosting> {
    let mut base = resolve?;
    if base.title.trim().is_empty() && !hint_title.trim().is_empty() {
        base.title = hint_title;
    }
    if base
        .description
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        base.description = hint_description;
    }
    Some(base)
}

#[cfg(test)]
mod tests;
