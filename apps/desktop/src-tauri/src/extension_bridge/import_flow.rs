//! `import.request` → `import.result` — the extension's "Save this job" import
//! flow. Parses the posting (Scan mode from the extension's captured DOM, else
//! URL mode via the resolver), upserts the Applications aggregate from it
//! (Application only — an import is a pursuit, not a discovery, so it never
//! enters the postings cache / Jobs feed), and best-effort fills the reply's
//! `matchScore`. Split out of `mod.rs` to keep that module under the R8 hard
//! LOC cap (`tests/architecture.rs`) — the same relocation as
//! `status_update.rs`/`match_live.rs`; behavior-identical, no logic changes.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use super::{auth, match_live, msg};
use crate::applications::{
    normalize_job_url, ApplicationMeta, ApplicationOrigin, ApplicationStore,
};
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, APPLICATIONS_CHANGED};

/// The successful import outcome: the created/merged application id, its status,
/// and the parsed title/company (so the popup can name the imported job).
pub(super) struct ImportOk {
    pub(super) application_id: String,
    pub(super) status: String,
    pub(super) title: String,
    pub(super) company: String,
    /// True when nothing usable parsed and a stub was persisted (empty title) —
    /// the extension surfaces this so the user knows to complete the row.
    pub(super) partial: bool,
    /// Best-effort keyword-only score (0–100) — see
    /// [`match_live::score_import_posting_bounded`]. `None` on any failure OR
    /// a timeout; the import above has already succeeded regardless.
    pub(super) match_score: Option<f64>,
}

/// Build a canonical `import.result` envelope (success or error). The error's
/// `to_string()` becomes the `error` field the extension surfaces. Also reused
/// by `mod.rs`'s `advance_authenticated` fallback to shape the "unknown
/// message type" error reply (see that call site) — the envelope shape is the
/// same either way.
pub(super) fn result_reply(req_id: &str, outcome: AppResult<ImportOk>) -> String {
    let payload = match outcome {
        Ok(ok) => {
            let mut obj = serde_json::Map::new();
            obj.insert("applicationId".to_string(), json!(ok.application_id));
            obj.insert("status".to_string(), json!(ok.status));
            obj.insert("title".to_string(), json!(ok.title));
            obj.insert("company".to_string(), json!(ok.company));
            obj.insert("partial".to_string(), json!(ok.partial));
            // Omit the key entirely when absent (mirrors `profile.result`'s
            // `extraLinks` discipline), not `null`.
            if let Some(score) = ok.match_score {
                obj.insert("matchScore".to_string(), json!(score));
            }
            Value::Object(obj)
        }
        Err(e) => json!({ "error": e.to_string() }),
    };
    json!({
        "type": msg::IMPORT_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Persist a parsed [`crate::scraping::types::JobPosting`] from an import as a
/// Saved Application and return `(application_id, status_id)`. This is the
/// *entire* persistence side effect of an import: it touches the
/// [`ApplicationStore`] only and has **no access to the `PostingsCache`**, so
/// an import can never enter the Jobs/discovery feed. Split out of
/// [`handle_import`] (which needs an `AppHandle` for event/notification
/// plumbing) so the import → Application contract is unit-testable without a
/// Tauri app — see `import_tests.rs`.
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

/// Core import: parse the posting (Scan mode from provided HTML, else URL mode
/// via the resolver), upsert the Applications aggregate from it (Application
/// only — not the postings cache), emit the change event, and return the
/// application id + status.
pub(super) async fn handle_import(app: &AppHandle, payload: Value) -> AppResult<ImportOk> {
    let url = payload
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let html = payload
        .get("html")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let applied = payload.get("applied").and_then(|v| v.as_bool());

    if url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }

    // Centralized SPA/list-view normalization: if the user imported from a board's
    // search/SPA view (selected job id in a query param), rewrite to the canonical
    // single-job URL so BOTH import modes resolve the SELECTED job, not the list
    // shell. `None` → already a direct page / unknown host (use the URL as-is).
    let canonical = crate::scraping::scrape_url::canonical_job_url(&url);
    let effective_url = canonical.as_deref().unwrap_or(url.as_str());

    // URL / SSRF safety on whatever we will actually fetch + store (the canonical
    // link when rewritten, else the original). Normalize (http(s) only) then guard
    // the host against loopback/private/link-local/`*.local`.
    let normalized = normalize_job_url(effective_url);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "url is not a valid http(s) URL".to_string(),
        ));
    }
    if !auth::is_safe_import_url(effective_url) {
        return Err(AppError::Validation(
            "url host is not allowed (private/loopback)".to_string(),
        ));
    }

    // Shared rate + concurrency budget with the scrape_resolve_url IPC command.
    // resolve() now follows up to 2 redirect hops per call (up to 3 outbound
    // fetches), so every resolve() call must hold a limiter slot — the same
    // "scrape_url" key and constants the command uses. `try_state` gracefully
    // handles the (startup-failure) case where Limiter was never managed.
    let limiter = app
        .try_state::<std::sync::Arc<crate::limits::Limiter>>()
        .map(|s| s.inner().clone());
    let acquire_slot = || -> AppResult<Option<crate::limits::ConcurrencyGuard>> {
        match &limiter {
            Some(l) => l
                .acquire(
                    "scrape_url",
                    crate::limits::SCRAPE_RATE_MAX,
                    crate::limits::SCRAPE_CONCURRENCY_MAX,
                )
                .map(Some),
            None => Ok(None), // Limiter not managed (startup failure) — allow through
        }
    };

    // At most one network fetch. For a SPA/list view (canonical rewrite), the
    // captured DOM's `[data-ajh-job-root]` hint is the SAME selected job's
    // detail pane — not the list shell (LinkedIn search/collections views
    // render the full JD client-side into it; see content.ts's pane-first
    // `JOB_NODE_CANDIDATES`) — so when the canonical resolve comes back
    // unusable or missing a description (its anonymous fetch commonly hits an
    // authwall), fill the gap from that hint. Deliberately scoped to the hint
    // ONLY (`job_root_generic_html`, never the whole-document
    // `parse_from_html`) — a list shell commonly carries its own SEO JSON-LD
    // for an UNRELATED job, and `parse_from_html`'s precedence would let that
    // JSON-LD override the hint (see `merge_resolve_with_hint`'s doc). No
    // usable hint on the shell → the stub/partial path below covers it. For a
    // direct page (no canonical rewrite) the DOM is parsed directly (its own
    // JSON-LD, if any, describes THAT page); URL mode with no DOM falls back
    // to a server fetch.
    let mut posting: Option<crate::scraping::types::JobPosting> =
        if let Some(c) = canonical.as_deref() {
            let _guard = acquire_slot()?;
            let resolved = crate::scraping::scrape_url::resolve(c).await?; // SPA/list view → selected job's canonical URL
            let resolved_needs_hint_fallback = !resolved.as_ref().is_some_and(usable)
                || resolved
                    .as_ref()
                    .and_then(|p| p.description.as_deref())
                    .is_none_or(|d| d.trim().is_empty());
            let hint = if resolved_needs_hint_fallback {
                html.as_deref()
                    .and_then(crate::scraping::scrape_url::job_root_generic_html)
            } else {
                None
            };
            match hint {
                Some((hint_title, hint_description)) => {
                    merge_resolve_with_hint(resolved, hint_title, hint_description)
                }
                None => resolved,
            }
        } else if let Some(h) = html.as_deref() {
            crate::scraping::scrape_url::parse_from_html(&url, h) // direct page → captured authenticated DOM
        } else {
            let _guard = acquire_slot()?;
            crate::scraping::scrape_url::resolve(effective_url).await? // URL mode, no DOM → server fetch
        };

    // Single server-fetch fallback: only the direct-page DOM path that came up unusable
    // (a board API may resolve the same URL where the DOM parse missed). Skipped for the
    // canonical and URL-mode branches because they already fetched `effective_url`.
    if !posting.as_ref().is_some_and(usable) && canonical.is_none() && html.is_some() {
        let _guard = acquire_slot()?;
        if let Some(p) = crate::scraping::scrape_url::resolve(effective_url).await? {
            if usable(&p) || posting.is_none() {
                posting = Some(p);
            }
        }
    }

    // Never lose an import click: if nothing usable parsed, persist a stub the user
    // can complete later (title empty → flagged partial), instead of erroring out.
    let (posting, partial) = if posting.as_ref().is_some_and(usable) {
        (posting.unwrap(), false)
    } else {
        let host = reqwest::Url::parse(effective_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default();
        let stub = crate::scraping::types::JobPosting {
            id: format!("url:{effective_url}"),
            external_id: None,
            title: String::new(),
            company: host,
            location: None,
            url: effective_url.to_string(),
            source: "url".to_string(),
            description: None,
            requirements: None,
            posted_at: None,
            captured_at: chrono::Utc::now().timestamp_millis(),
            extra: std::collections::HashMap::new(),
        };
        (stub, true)
    };

    // Passively harvest the ATS company slug from the imported posting's URL
    // (parse-only, zero network) — ADR-030 §c, source='extension'. Best-effort:
    // resolve the store at this shell boundary (missing store → no-op) and forward it
    // to the seam, which degrades on an upsert error; never blocks the import.
    if let Some(store) = app.try_state::<crate::discovered::DiscoveredCompanyStore>() {
        crate::discovered::harvest_ats_refs(
            store.inner(),
            std::iter::once((posting.url.clone(), posting.company.clone())),
            "extension",
        );
    }

    // An import is a deliberate pursuit, NOT a discovery: it creates only the
    // status-bearing Application below. It is intentionally NOT added to the
    // in-memory postings cache (the Jobs/discovery feed via
    // `commands::scrape::scrape_list_postings`), so an imported job shows up
    // under Applications only — never in the Jobs page. The Application carries
    // the title/company the detail-page tailoring needs; the JD is re-resolved
    // there if required.

    // Upsert the status-bearing Application (Saved origin → `saved` unless the
    // request flags it applied). Merges onto any existing row for this URL.
    let store = app
        .try_state::<ApplicationStore>()
        .ok_or_else(|| AppError::Config("applications store unavailable".to_string()))?;
    let (id, status) = persist_import_application(store.inner(), &normalized, &posting, applied)?;

    // A partial stub has an empty title — fall back to the company (host) so the
    // event payload and toast still name something the user recognizes.
    let title_is_blank = posting.title.trim().is_empty();
    let display_name = if title_is_blank {
        posting.company.clone()
    } else {
        posting.title.clone()
    };
    let body = if title_is_blank {
        posting.company.clone()
    } else {
        format!("{} · {}", posting.title, posting.company)
    };

    // Tell the renderer to refresh (Applications + Jobs views) and surface a
    // live toast. Carry the title/company/status so the toast can name the job
    // without a refetch race.
    emit_event(
        app,
        APPLICATIONS_CHANGED,
        json!({
            "applicationId": id.clone(),
            "title": display_name.clone(),
            "company": posting.company.clone(),
            "status": status.clone(),
        }),
    );

    // Also drop a Notification Center record. Best-effort and additive — the
    // lists still refresh via the `applications:changed` emit above; this only
    // adds the inbox entry + a focused-window toast, with an OS banner only when
    // the window is unfocused (the import UX intent). Route → the Applications
    // view, highlighting the just-imported row.
    let mut search = serde_json::Map::new();
    search.insert("highlight".to_string(), Value::String(id.clone()));
    crate::commands::notifications::push_and_notify(
        app,
        crate::notifications::NewNotification {
            kind: "import.result".to_string(),
            title: format!("Imported {display_name}"),
            body,
            route: Some(crate::notifications::NotificationRoute {
                to: "/applications".to_string(),
                search: Some(search),
            }),
        },
        crate::commands::notifications::OsBanner::WhenUnfocused,
    );

    // Best-effort, TIME-BOUNDED keyword-only score for `matchScore` — the
    // Application above already persisted, so a failure OR a timeout only
    // omits the field. See `match_live::score_import_posting_bounded`'s doc.
    //
    // Threat-model note: this score is ungated (unlike `match.live`, no
    // autofill opt-in check), so it is technically a coarse
    // résumé-membership signal — one bit per import of "did this posting
    // score well against my résumé". Accepted because every probe that
    // produces it MUST first persist the visible `Application` row above and
    // fire the toast/OS notification above — probing is loud by
    // construction, not silent; that forced visibility (not the score's
    // coarseness) is the rationale. See `match_live`'s module doc for the
    // full note.
    let match_score = match_live::score_import_posting_bounded(app, &posting, &normalized).await;

    Ok(ImportOk {
        application_id: id,
        status,
        title: posting.title,
        company: posting.company,
        match_score,
        partial,
    })
}
