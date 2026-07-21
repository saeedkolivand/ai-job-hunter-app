//! Discovery IPC surface (ADR-030 §f): reads over the passively-harvested
//! [`crate::discovered::DiscoveredCompanyStore`].
//!
//! `discovery_search_companies` powers the ScrapeForm slug typeahead;
//! `discovery_set_starred` toggles a "watched company"; `discovery_watched`
//! lists the current stars. Every input is re-validated + clamped SERVER-SIDE —
//! the renderer's Zod is not a trust boundary.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

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

/// Typeahead search over discovered/seeded company slugs + display names.
/// Returns `[]` when the store is unavailable (startup failure) rather than
/// erroring — an empty typeahead degrades gracefully.
#[tauri::command]
pub fn discovery_search_companies(app: AppHandle, req: DiscoverySearchRequest) -> Value {
    let Some(store) = app.try_state::<crate::discovered::DiscoveredCompanyStore>() else {
        return json!([]);
    };
    let query = clamp_bytes(&req.query, MAX_QUERY_BYTES);
    json!(store.search(&query, SEARCH_LIMIT))
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
mod tests {
    use super::{clamp_bytes, MAX_QUERY_BYTES};

    #[test]
    fn clamp_trims_and_byte_caps_on_char_boundary() {
        assert_eq!(clamp_bytes("  hello  ", MAX_QUERY_BYTES), "hello");
        let euros = "€".repeat(100); // 300 bytes > cap
        let out = clamp_bytes(&euros, MAX_QUERY_BYTES);
        assert!(out.len() <= MAX_QUERY_BYTES, "query byte-clamped");
        assert!(
            out.is_char_boundary(out.len()),
            "clamp must cut on a char boundary (valid UTF-8)"
        );
    }

    /// The registry predicate `discovery_set_starred` gates on: only a registered
    /// company-scoped board id may be starred, so a compromised renderer can't
    /// materialize garbage rows for a non-ATS or unknown id.
    #[test]
    fn only_company_scoped_boards_are_watchable() {
        let watchable =
            |ats: &str| crate::scraping::boards::get(ats).is_some_and(|s| s.requires_company());
        assert!(watchable("greenhouse"), "greenhouse is company-scoped");
        assert!(watchable("ashby"), "ashby is company-scoped");
        assert!(!watchable("linkedin"), "linkedin is not company-scoped");
        assert!(!watchable("aggregator"), "aggregator is not company-scoped");
        assert!(!watchable("not-a-real-board"), "unknown id is rejected");
    }
}
