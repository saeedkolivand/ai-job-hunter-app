//! Discovery IPC surface (ADR-030 §f): reads over the passively-harvested
//! [`crate::discovered::DiscoveredCompanyStore`].
//!
//! `discovery_search_companies` powers the ScrapeForm slug typeahead;
//! `discovery_set_starred` toggles a "watched company"; `discovery_watched`
//! lists the current stars. Every input is re-validated + clamped SERVER-SIDE —
//! the renderer's Zod is not a trust boundary.

use std::collections::HashSet;

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::discovered::DiscoveredCompany;

// Generated from the Zod schemas by `pnpm gen:ipc`.
pub use crate::ipc_contracts::discovery::{DiscoverySearchRequest, DiscoveryStarRequest};

/// Server-side byte cap on the search query (defense-in-depth vs. a caller that
/// bypasses the Zod `.max(100)` bound — CWE-770).
const MAX_QUERY_BYTES: usize = 100;

/// Clamp `s` to at most `max` bytes on a UTF-8 char boundary (same discipline as
/// `dedup`/`discovered`).
fn clamp_bytes(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// How many typeahead rows to return per search. The store also clamps this.
const SEARCH_LIMIT: u32 = 50;

/// Typeahead search over discovered/seeded company slugs + display names,
/// topped up with the vendored community slug directory (ADR-030 §b) when the
/// organic/starred rows don't already fill the page. Vendored rows never
/// outrank a real DB row for the same `(atsKind, slug)` — a duplicate is
/// dropped in favor of the DB row, which carries real seen-count/starred
/// state. Returns `[]` when the store is unavailable (startup failure) rather
/// than erroring — an empty typeahead degrades gracefully.
#[tauri::command]
pub fn discovery_search_companies(app: AppHandle, req: DiscoverySearchRequest) -> Value {
    let Some(store) = app.try_state::<crate::discovered::DiscoveredCompanyStore>() else {
        return json!([]);
    };
    let query = clamp_bytes(&req.query, MAX_QUERY_BYTES);
    let db_results = store.search(&query, SEARCH_LIMIT);
    json!(fill_with_vendor_results(
        db_results,
        &query,
        crate::discovered::vendored::search
    ))
}

/// Top up `db_results` with vendored rows up to [`SEARCH_LIMIT`], via
/// `search_vendor` (production: [`crate::discovered::vendored::search`];
/// swappable in tests so this over-fetch/dedup interaction doesn't need a
/// `DiscoveredCompanyStore`/`AppHandle`).
///
/// Over-fetches before deduping: a vendor row can only collide with one of
/// the `db_results.len()` DB rows, so asking `search_vendor` for
/// `remaining + db_results.len()` candidates guarantees `remaining` survive
/// even in the worst case where every collision lands at the front of the
/// (alphabetical) vendor pool. Asking for exactly `remaining` and deduping
/// after — the previous behavior — could under-fill the page by exactly as
/// many rows as collided.
fn fill_with_vendor_results(
    db_results: Vec<DiscoveredCompany>,
    query: &str,
    search_vendor: impl Fn(&str, usize) -> Vec<DiscoveredCompany>,
) -> Vec<DiscoveredCompany> {
    let remaining = (SEARCH_LIMIT as usize).saturating_sub(db_results.len());
    if remaining == 0 {
        return db_results;
    }
    let pool = remaining.saturating_add(db_results.len());
    let vendor_results = search_vendor(query, pool);
    merge_vendor_results(db_results, vendor_results, remaining)
}

/// Append `vendor` rows after `db` rows, dropping any vendor row that
/// duplicates a `(atsKind, slug)` already present in `db` — a real DB row
/// always wins because it carries actual seen-count/starred state, where a
/// vendor row is always `seen_count=0, starred=false`. Case-insensitive on
/// the key (Ashby preserves slug casing, so `Linear` from the DB and
/// `linear` from the vendor directory are the same company). The
/// `remaining`-row cap is applied AFTER dedup (not by the caller pre-sizing
/// `vendor`), so a duplicate doesn't consume page room a further unique
/// match could have filled; `db` is already capped at [`SEARCH_LIMIT`] by
/// the store, so the result never exceeds `SEARCH_LIMIT`.
fn merge_vendor_results(
    db: Vec<DiscoveredCompany>,
    vendor: Vec<DiscoveredCompany>,
    remaining: usize,
) -> Vec<DiscoveredCompany> {
    let seen: HashSet<(String, String)> = db
        .iter()
        .map(|c| (c.ats_kind.to_ascii_lowercase(), c.slug.to_ascii_lowercase()))
        .collect();
    let mut results = db;
    results.extend(
        vendor
            .into_iter()
            .filter(|c| {
                !seen.contains(&(c.ats_kind.to_ascii_lowercase(), c.slug.to_ascii_lowercase()))
            })
            .take(remaining),
    );
    results
}

/// Star / unstar a company. RESOLVES an `{ error }` union on failure (the hook
/// narrows + throws) — mirrors `dedup_mark_not_duplicate`.
#[tauri::command]
pub fn discovery_set_starred(app: AppHandle, req: DiscoveryStarRequest) -> Value {
    let Some(store) = app.try_state::<crate::discovered::DiscoveredCompanyStore>() else {
        return json!({ "error": "discovered store unavailable" });
    };
    // The store re-clamps + treats empty ats/slug as a no-op; validate here too so
    // an out-of-bounds caller can't drive a junk write (renderer Zod isn't a boundary).
    let ats = req.ats_kind.trim();
    let slug = req.slug.trim();
    if ats.is_empty() || slug.is_empty() {
        return json!({ "error": "atsKind and slug are required" });
    }
    // Reject an `atsKind` that isn't a registered company-scoped board id, so a
    // compromised renderer can't materialize garbage seed rows. Only company-scoped
    // ATS boards can be "watched" — that's the only set the autopilot resolver fans
    // out to. Keyed on the registry (`requires_company()`), not a hardcoded list.
    let is_company_board = crate::scraping::boards::get(ats)
        .map(|s| s.requires_company())
        .unwrap_or(false);
    if !is_company_board {
        return json!({ "error": "atsKind is not a company-scoped board" });
    }
    match store.set_starred(ats, slug, req.starred) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Every watched (starred) company, as full rows for the renderer — via the
/// store's dedicated starred-row query (no search-cap coupling). The autopilot
/// resolver uses the lighter `store.watched()` `(ats, slug)` pairs directly.
#[tauri::command]
pub fn discovery_watched(app: AppHandle) -> Value {
    let Some(store) = app.try_state::<crate::discovered::DiscoveredCompanyStore>() else {
        return json!([]);
    };
    json!(store.watched_companies())
}

#[cfg(test)]
mod tests;
