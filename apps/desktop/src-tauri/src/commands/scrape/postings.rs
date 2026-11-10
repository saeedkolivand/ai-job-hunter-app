//! The live postings cache: list, clear and the description write-back. Split out
//! of `commands/scrape.rs` for R8 (issue #1280); `scrape.rs` re-exports the
//! commands, so each keeps its `commands::scrape::<name>` path.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::autopilot::{ResolveWrite, ResolvedIdentity};
use crate::error::{AppError, AppResult};
use crate::postings::{attach_interactions, InteractionStore, PostingsCache};
use crate::scraping::scrape_url::{identity_verdict, IdentityVerdict, PostingIdentity};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeUpdateDescriptionRequest {
    /// Renamed from `id` (issue #1106): the board-synthetic `id`
    /// `PostingsCache` upserts by has no meaning off that one in-memory
    /// cache, and no Agent/MCP read command ever exposed it — every reader
    /// (`job`/`best-matches`/`autopilot_best_matches`, and the renderer's own
    /// `JobDetailPane`) already has the posting's `url`. See
    /// `scrape_update_description`'s own doc for the two stores this now
    /// addresses by url.
    pub url: String,
    pub description: String,
    /// Identity of the posting the description was resolved from (issue
    /// #1445). A stored row that no longer matches it is held for review, not
    /// written.
    pub resolved: ResolvedIdentity,
}

/// Outcome of [`scrape_update_description`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DescriptionWriteStatus {
    Updated,
    NoMatch,
    /// The url now leads to a different advert; nothing was written onto the
    /// drifted rows and they carry an identity review.
    IdentityDrift,
    /// A stored copy exists but nothing was written and nothing drifted
    /// (nothing comparable, or a drift the user already dismissed): the text
    /// could not be verified against the stored advert. `NoMatch` means only
    /// that no stored row or cache entry exists at all.
    Unverified,
}

/// Upper bound on a write-back description. A full JD is on the order of a few KB;
/// 256 KB is generous headroom while bounding a looping/XSS'd renderer from
/// ballooning a cached entry. Over-cap input is rejected, not silently truncated,
/// so a caller can tell the write didn't take effect as sent.
const MAX_DESCRIPTION_LEN: usize = 256 * 1024;

/// Write a freshly-resolved full description back into BOTH stores that can
/// carry a copy of this posting, addressed by `url` (issue #1106): the live
/// [`PostingsCache`] (session-lifetime, in-memory) AND every matching
/// `FoundJob` row across every persisted `Autopilot` record
/// (`AutopilotStore::update_found_job_descriptions`). `id` — this command's
/// old parameter — was a board-synthetic key with no meaning off
/// `PostingsCache`: no Agent/MCP read command ever exposed it, and even a
/// correct `id`-based lookup would still silently miss every posting
/// surfaced via `job`/`best-matches`/`autopilot_best_matches`, which read
/// `Autopilot.found_jobs` directly and never touch `PostingsCache` at all.
/// `url` is the one identity every surface already shares — see
/// `extension_bridge::agent_read`'s own module doc ("`url` is the
/// CROSS-RESOURCE KEY — not an id").
///
/// The detail pane resolves a fuller description on demand (see
/// [`scrape_resolve_url`]); without this, match scoring would continue
/// reading the truncated aggregator snippet from whichever store still held
/// it. The match-score cache is job-text-hash-keyed, so updating the
/// description invalidates cached scores for that job; on-demand scoring via
/// `useJobMatchScore` will recompute.
///
/// The verdict runs on `resolved`, an identity the CALLER asserts (the renderer,
/// or an agent): it protects honest flows from attaching another advert's text,
/// and never authorizes releasing a hold — a held row is only released by
/// `autopilot_dismiss_identity_review`.
///
/// Both stores are tried independently, but a posting whose identity drifted
/// (issue #1445) is never written — see [`write_status`] for the resulting
/// `updated`/`noMatch`/`identityDrift` rule.
///
/// Validate the write-back inputs, returning the NORMALIZED url on success
/// (reused as-is for both stores, never re-derived per store). Pure (no
/// `AppHandle`) so the error paths are unit-tested directly. Rejects an empty
/// or non-http(s) url (an explicit `http://`/`https://` prefix is REQUIRED —
/// `normalize_job_url` alone passes a schemeless bare token like the
/// pre-rename `id` shape straight through unchanged, which would otherwise
/// validate and then miss both stores as a silent, honest-looking
/// `data: false`), an empty/whitespace-only description (this command
/// CORRECTS a description, it does not clear one — an empty string here
/// would otherwise wipe every matching row across both stores), and an
/// over-cap description rather than silently truncating, so the caller can
/// tell the write didn't take effect as sent.
///
/// Canonicalizes via [`crate::scraping::scrape_url::canonical_job_url`]
/// BEFORE normalizing — the exact two-line pipeline
/// `extension_bridge::agent_read::job_resource` uses — so a board-specific
/// search/SPA-view url (e.g. LinkedIn's `?currentJobId=` search view) lands
/// on the SAME identity a `job`/`answers.save` read resolves it to, rather
/// than a normalized value neither store was ever keyed by (issue #1106
/// follow-up).
fn validate_update_description(url: &str, description: &str) -> AppResult<String> {
    let url = url.trim();
    if url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }
    if description.trim().is_empty() {
        return Err(AppError::Validation(
            "description must not be empty — this command corrects a description, it does not \
             clear one"
                .to_string(),
        ));
    }
    if description.len() > MAX_DESCRIPTION_LEN {
        return Err(AppError::Validation(format!(
            "description exceeds the {MAX_DESCRIPTION_LEN}-byte cap"
        )));
    }
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(AppError::Validation(
            "url must have an explicit http(s) scheme".to_string(),
        ));
    }
    let canonical = crate::scraping::scrape_url::canonical_job_url(url);
    let effective = canonical.as_deref().unwrap_or(url);
    let normalized = crate::applications::normalize_job_url(effective);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "url is not a valid http(s) URL".to_string(),
        ));
    }
    Ok(normalized)
}

/// Per-field byte cap on a caller-supplied [`ResolvedIdentity`]: these strings
/// are stored on a found job and echoed to agents, so they are bounded like any
/// other caller input.
const MAX_RESOLVED_FIELD_LEN: usize = 2 * 1024;

/// Bound a caller-supplied resolved identity: every field at most
/// [`MAX_RESOLVED_FIELD_LEN`] bytes and `url` a parseable http(s) url. Returns a
/// copy whose `url` is the PARSED form, so control characters never reach the
/// stored review (and from there an agent reply).
fn validate_resolved(resolved: &ResolvedIdentity) -> AppResult<ResolvedIdentity> {
    let fields = [
        ("title", resolved.title.as_str()),
        ("company", resolved.company.as_str()),
        ("url", resolved.url.as_str()),
        ("externalId", resolved.external_id.as_deref().unwrap_or("")),
    ];
    for (name, value) in fields {
        if value.len() > MAX_RESOLVED_FIELD_LEN {
            return Err(AppError::Validation(format!(
                "resolved.{name} exceeds the {MAX_RESOLVED_FIELD_LEN}-byte cap"
            )));
        }
    }
    let parsed = reqwest::Url::parse(resolved.url.trim())
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .ok_or_else(|| {
            AppError::Validation("resolved.url must be a valid http(s) url".to_string())
        })?;
    if parsed.as_str().len() > MAX_RESOLVED_FIELD_LEN {
        return Err(AppError::Validation(format!(
            "resolved.url exceeds the {MAX_RESOLVED_FIELD_LEN}-byte cap"
        )));
    }
    Ok(ResolvedIdentity {
        url: parsed.as_str().to_string(),
        ..resolved.clone()
    })
}

/// The one validation both write-back entry points run (`description: None` =
/// verdict-only): the url + description rules of [`validate_update_description`]
/// plus [`validate_resolved`]. Returns the normalized url and the sanitized
/// resolved identity.
fn validate_write_back(
    url: &str,
    description: Option<&str>,
    resolved: &ResolvedIdentity,
) -> AppResult<(String, ResolvedIdentity)> {
    let normalized = validate_update_description(url, description.unwrap_or("-"))?;
    Ok((normalized, validate_resolved(resolved)?))
}

/// Fold the per-store results into the one status the caller sees. A drifted
/// found-job row always wins. A drift in the live cache alone (the url now opens
/// a different posting) reports `IdentityDrift`, unless a row was genuinely
/// written, in which case that write is the more useful fact. Otherwise success
/// iff EITHER store matched a row; a copy that exists but could not be
/// verified or written is `Unverified`, never `NoMatch`.
fn write_status(
    cache_hit: bool,
    cache_drift: bool,
    cache_unverified: bool,
    rows: ResolveWrite,
) -> DescriptionWriteStatus {
    if rows.drifted > 0 || (cache_drift && rows.updated == 0) {
        DescriptionWriteStatus::IdentityDrift
    } else if cache_hit || rows.updated > 0 {
        DescriptionWriteStatus::Updated
    } else if rows.skipped > 0 || cache_unverified {
        DescriptionWriteStatus::Unverified
    } else {
        DescriptionWriteStatus::NoMatch
    }
}

/// Fold the live cache's per-entry verdicts into `(drifted, writable)`: drifted
/// when ANY entry is a different advert; writable only when EVERY entry is the
/// same advert (no entry at all is trivially writable — the write then reports
/// no match).
fn fold_cache_verdicts(verdicts: &[IdentityVerdict]) -> (bool, bool) {
    (
        verdicts
            .iter()
            .any(|v| matches!(v, IdentityVerdict::Drift(_))),
        verdicts.iter().all(|v| *v == IdentityVerdict::Same),
    )
}

/// How the live cache's entries for `normalized_url` compare with `resolved`
/// (issue #1445), folded by [`fold_cache_verdicts`].
fn cache_check(app: &AppHandle, normalized_url: &str, resolved: &ResolvedIdentity) -> (bool, bool) {
    let stored = app
        .state::<Mutex<PostingsCache>>()
        .lock()
        .stored_identities(normalized_url);
    let verdicts: Vec<IdentityVerdict> = stored
        .iter()
        .map(|(title, company, url)| {
            identity_verdict(
                PostingIdentity {
                    title,
                    company,
                    url,
                },
                resolved.as_posting(),
            )
        })
        .collect();
    fold_cache_verdicts(&verdicts)
}

/// Run only the identity verdict of `resolved` against every stored row at
/// `url`, flagging drifted rows (the LinkedIn enrichment's drift path). Returns
/// the number of rows held for review.
pub(crate) fn flag_identity_drift(
    app: &AppHandle,
    url: &str,
    resolved: &ResolvedIdentity,
) -> AppResult<u32> {
    let (normalized_url, resolved) = validate_write_back(url, None, resolved)?;
    Ok(crate::commands::autopilot::store(app)
        .lock()
        .update_found_job_descriptions(&normalized_url, None, &resolved)
        .drifted)
}

#[tauri::command]
pub fn scrape_update_description(
    app: AppHandle,
    req: ScrapeUpdateDescriptionRequest,
) -> AppResult<DescriptionWriteStatus> {
    let (normalized_url, resolved) =
        validate_write_back(&req.url, Some(&req.description), &req.resolved)?;
    let (cache_drift, cache_writable) = cache_check(&app, &normalized_url, &resolved);
    let rows = crate::commands::autopilot::store(&app)
        .lock()
        .update_found_job_descriptions(&normalized_url, Some(&req.description), &resolved);
    let cache_hit = cache_writable && {
        let cache = app.state::<Mutex<PostingsCache>>();
        cache
            .lock()
            .update_description(&normalized_url, &req.description)
    };
    // A cache entry that is neither drifted nor writable had nothing comparable.
    let cache_unverified = !cache_drift && !cache_writable;
    Ok(write_status(cache_hit, cache_drift, cache_unverified, rows))
}

#[tauri::command]
pub fn scrape_list_postings(app: AppHandle) -> Value {
    // Snapshot the interactions first and DROP that guard before locking the
    // postings cache, so the two mutexes are never held at once (no lock-order
    // deadlock). `list` takes `&mut` because it lazily hydrates from disk.
    let interactions = {
        let store = app.state::<Mutex<InteractionStore>>();
        let mut guard = store.lock();
        guard.list(None)
    };
    // Now join the interactions onto the live postings so the jobs list can show
    // viewed/applied/saved badges (the cache items carry no interactions).
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock();
    json!(attach_interactions(guard.get_all(), &interactions))
}

#[tauri::command]
pub fn scrape_clear_postings(app: AppHandle) -> Value {
    app.state::<Mutex<PostingsCache>>().lock().clear_all();
    json!(null)
}

#[cfg(test)]
mod tests;
