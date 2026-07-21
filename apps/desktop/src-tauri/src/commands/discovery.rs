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

/// Passively harvest ATS company slugs from a batch of `(url, company)` posting
/// pairs (parse-only, zero network) into the discovered-companies store under
/// `source`. Each posting's `company` becomes the display name when the URL
/// itself carries none (it never does today). Degrades with a `log::warn` rather
/// than failing on a missing store or a store error — harvesting is best-effort
/// enrichment, never a hard dependency of the ingest that triggered it (ADR-030
/// §c, per the dedup degrade-not-fail lesson). Lives in the shell layer (holds the
/// `AppHandle`) so the L1 store stays Tauri-free, mirroring how `recluster_
/// postings_cache` lives here rather than in the `dedup` store.
pub fn harvest_ats_refs<I>(app: &AppHandle, items: I, source: &str)
where
    I: IntoIterator<Item = (String, String)>,
{
    let refs: Vec<(String, String, Option<String>, String)> = items
        .into_iter()
        .filter_map(|(url, company)| posting_to_ref(&url, &company, source))
        .collect();
    if refs.is_empty() {
        return;
    }
    let Some(store) = app.try_state::<crate::discovered::DiscoveredCompanyStore>() else {
        return;
    };
    if let Err(e) = store.upsert_batch(&refs) {
        log::warn!("[discovered] harvest upsert failed ({e}); slugs not recorded this ingest");
    }
}

/// Pure per-posting mapping: `(url, company)` → the store's upsert tuple
/// `(ats, slug, display_name, source)`, or `None` when the URL is not a recognised
/// ATS posting. The display name is the posting's `company` ONLY when non-empty
/// after trimming (the URL itself never carries one today), so an empty/whitespace
/// company yields `None` — never an empty display name. The slug is ALWAYS from
/// `extract_ats_ref`, never a hand-written mapping. Pure (no `AppHandle`) so BOTH
/// [`harvest_ats_refs`] and its acceptance test exercise the SAME fallback (the
/// ADR-029 shared-seam lesson — a test that re-implements this branch tests
/// nothing).
fn posting_to_ref(
    url: &str,
    company: &str,
    source: &str,
) -> Option<(String, String, Option<String>, String)> {
    crate::scraping::ats_ref::extract_ats_ref(url).map(|r| {
        let display = r.display_name.or_else(|| {
            let c = company.trim();
            (!c.is_empty()).then(|| c.to_string())
        });
        (r.ats, r.slug, display, source.to_string())
    })
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
    use super::{clamp_bytes, posting_to_ref, MAX_QUERY_BYTES};

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

    /// ADR-030 §c/§d acceptance: a mixed batch of realistic aggregator/board posting
    /// URLs, run through the SAME production URL-shape authority the harvest call site
    /// uses (`extract_ats_ref`) into a real `DiscoveredCompanyStore`, makes the
    /// harvested slugs discoverable via `search` — the exact read the typeahead does.
    ///
    /// This crate has no `tauri::test` mock-app harness, so the test drives the two
    /// production seams `harvest_ats_refs` composes (extract → upsert) directly rather
    /// than the `AppHandle`-bound wrapper (whose only extra work is the store lookup).
    /// The URL→(ats, slug) mapping is NEVER re-derived — it flows through
    /// `extract_ats_ref` (per the ADR-029 shared-seam lesson).
    #[test]
    fn harvested_posting_urls_surface_their_slugs_in_search() {
        use crate::discovered::DiscoveredCompanyStore;

        let dir = tempfile::TempDir::new().unwrap();
        let store = DiscoveredCompanyStore::open(dir.path()).unwrap();

        // ATS apply/redirect URLs that leak a company slug, plus one non-ATS
        // aggregator URL that must contribute nothing to the store.
        let postings: &[(&str, &str)] = &[
            ("https://boards.greenhouse.io/stripe/jobs/123", "Stripe"),
            ("https://jobs.lever.co/spotify/abc", "Spotify"),
            ("https://jobs.ashbyhq.com/Linear/uuid", "Linear Inc"),
            ("https://acme.recruitee.com/o/backend", "Acme"),
            ("https://example.com/jobs/42", "Random Co"), // non-ATS → ignored
        ];

        // Drive the SAME pure mapping `harvest_ats_refs` uses (extract + display
        // fallback) — never a re-implemented mapping (ADR-029 shared-seam lesson).
        let refs: Vec<(String, String, Option<String>, String)> = postings
            .iter()
            .filter_map(|(url, company)| posting_to_ref(url, company, "scrape"))
            .collect();
        store.upsert_batch(&refs).unwrap();

        // The greenhouse slug is now offered by the typeahead search, name backfilled.
        let stripe = store.search("stripe", 10);
        assert_eq!(
            stripe.len(),
            1,
            "the harvested greenhouse slug is searchable"
        );
        assert_eq!(stripe[0].ats_kind, "greenhouse");
        assert_eq!(stripe[0].slug, "stripe");
        assert_eq!(stripe[0].display_name.as_deref(), Some("Stripe"));

        // Ashby's case-sensitive slug survives the whole URL → store → search chain.
        let linear = store.search("Linear", 10);
        assert_eq!(linear.len(), 1);
        assert_eq!(linear[0].ats_kind, "ashby");
        assert_eq!(
            linear[0].slug, "Linear",
            "ashby casing preserved end-to-end"
        );

        // Exactly the 4 ATS postings were harvested — the non-ATS URL added nothing.
        assert_eq!(
            store.search("", 50).len(),
            4,
            "only company-scoped ATS postings feed the typeahead"
        );
        assert!(store.search("example", 10).is_empty());
    }

    /// The display-name fallback in `posting_to_ref` (shared by `harvest_ats_refs`):
    /// a real company name is kept, but an EMPTY/whitespace-only company yields
    /// `None` — never an empty display name (the branch the acceptance test above
    /// can't reach). A non-ATS URL maps to `None` regardless of the company.
    #[test]
    fn posting_to_ref_display_name_falls_back_and_drops_empty_company() {
        // Real company name → becomes the display name.
        let (ats, slug, name, source) = posting_to_ref(
            "https://boards.greenhouse.io/stripe/jobs/1",
            "Stripe",
            "scrape",
        )
        .expect("greenhouse URL must map");
        assert_eq!((ats.as_str(), slug.as_str()), ("greenhouse", "stripe"));
        assert_eq!(name.as_deref(), Some("Stripe"));
        assert_eq!(source, "scrape");

        // Empty / whitespace-only company → display_name None (never "").
        for empty in ["", "   ", "\t\n"] {
            let (_, _, name, _) =
                posting_to_ref("https://jobs.lever.co/spotify/x", empty, "scrape")
                    .expect("lever URL must map");
            assert_eq!(
                name, None,
                "empty/whitespace company must yield no display name: {empty:?}"
            );
        }

        // Non-ATS URL → None regardless of the company.
        assert!(posting_to_ref("https://example.com/jobs/1", "Random Co", "scrape").is_none());
    }

    /// ADR-031 §c: a successful single-URL resolve (`scrape_resolve_url`) feeds the
    /// harvest seam with the RESOLVED posting's final/canonical `url` (what the
    /// command stores on the posting — an aggregator click-tracker resolves to the
    /// board's real posting url) + its company, so the slug surfaces in the
    /// typeahead like the scrape/autopilot/extension paths. `scrape_resolve_url` is
    /// `AppHandle`-bound (like `harvest_ats_refs`), so this drives the SAME pure
    /// `posting_to_ref` seam the command uses — no mock-app harness.
    #[test]
    fn single_resolve_harvests_the_resolved_postings_slug() {
        use crate::discovered::DiscoveredCompanyStore;

        let dir = tempfile::TempDir::new().unwrap();
        let store = DiscoveredCompanyStore::open(dir.path()).unwrap();

        // A resolved posting's canonical url + company (personio subdomain shape —
        // distinct from the multi-URL acceptance test above).
        let posting_url = "https://acme.jobs.personio.de/job/42";
        let company = "Acme GmbH";

        let ats_ref = posting_to_ref(posting_url, company, "scrape")
            .expect("a personio posting url must map to a ref");
        store.upsert_batch(&[ats_ref]).unwrap();

        let hits = store.search("acme", 10);
        assert_eq!(hits.len(), 1, "the resolved posting's slug is searchable");
        assert_eq!(hits[0].ats_kind, "personio");
        assert_eq!(hits[0].slug, "acme");
        assert_eq!(hits[0].display_name.as_deref(), Some("Acme GmbH"));
        assert_eq!(
            hits[0].source, "scrape",
            "a single-resolve harvest is source 'scrape'"
        );
    }
}
