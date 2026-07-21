//! ATS slug-harvest seam (ADR-030 §c): turns a batch of `(url, company)` posting
//! pairs into [`DiscoveredCompanyStore`] rows, parse-only + zero network.
//!
//! AppHandle-free by design — the L3 command handlers resolve the store via
//! `try_state` and pass it in, so this URL→store seam stays an L1 sibling of
//! `scraping::ats_ref` (the slug authority) and is unit-testable without a mock-app
//! harness.

use super::DiscoveredCompanyStore;

/// Passively harvest ATS company slugs from a batch of `(url, company)` posting
/// pairs (parse-only, zero network) into `store` under `source`. Each posting's
/// `company` becomes the display name when the URL itself carries none (it never
/// does today). Degrades with a `log::warn` rather than failing on a store error —
/// harvesting is best-effort enrichment, never a hard dependency of the ingest that
/// triggered it (ADR-030 §c, per the dedup degrade-not-fail lesson).
///
/// AppHandle-free so the L3 command layer stays thin and this seam is directly
/// testable: each call site resolves the store with
/// `app.try_state::<DiscoveredCompanyStore>()` and forwards it here, so a missing
/// store (startup failure) is a no-op that stays at the shell boundary rather than
/// wiring Tauri into this domain module.
pub fn harvest_ats_refs<I>(store: &DiscoveredCompanyStore, items: I, source: &str)
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
    if let Err(e) = store.upsert_batch(&refs) {
        log::warn!("[discovered] harvest upsert failed ({e}); slugs not recorded this ingest");
    }
}

/// Pure per-posting mapping: `(url, company)` → the store's upsert tuple
/// `(ats, slug, display_name, source)`, or `None` when the URL is not a recognised
/// ATS posting. The display name is the posting's `company` ONLY when non-empty
/// after trimming (the URL itself never carries one today), so an empty/whitespace
/// company yields `None` — never an empty display name. The slug is ALWAYS from
/// `extract_ats_ref`, never a hand-written mapping. Pure (no store) so BOTH
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

#[cfg(test)]
mod tests {
    use super::{harvest_ats_refs, posting_to_ref};
    use crate::discovered::DiscoveredCompanyStore;

    /// ADR-031 §c: a single resolved posting (`scrape_resolve_url`) drives the now
    /// AppHandle-free `harvest_ats_refs` end-to-end with a real store, pinning the
    /// call-site CONTRACT: the `url` argument — NOT the `company` — determines
    /// `(ats, slug)`, and the `company` — NOT the URL — becomes the display name. A
    /// swapped `(company, url)` argument order fails here (`extract_ats_ref` can't
    /// parse a company name, so nothing is stored and the search comes up empty).
    ///
    /// COVERAGE GAP (honest, narrowed): this pins the harvest SEAM (url → ref →
    /// store → search) via the real production function. The only line left unpinned
    /// is the command fetching `app.try_state::<DiscoveredCompanyStore>()` and
    /// forwarding the resolved posting's `(url, company)` — a swapped tuple THERE is
    /// `AppHandle` + network-bound (`resolve()`), unreachable without a mock-app
    /// harness this crate deliberately doesn't build, so it stays guarded by the
    /// argument names + code review.
    #[test]
    fn harvest_maps_resolved_posting_url_to_slug_and_company_to_display() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = DiscoveredCompanyStore::open(dir.path()).unwrap();

        // A resolved posting's canonical url + company (personio subdomain shape), in
        // the `(url, company)` order every call site passes.
        harvest_ats_refs(
            &store,
            std::iter::once((
                "https://acme.jobs.personio.de/job/42".to_string(),
                "Acme GmbH".to_string(),
            )),
            "scrape",
        );

        let hits = store.search("acme", 10);
        assert_eq!(hits.len(), 1, "the resolved posting's slug is searchable");
        // The URL determines (ats, slug) — a swapped arg order would try to parse
        // "Acme GmbH" as a URL and store nothing.
        assert_eq!(hits[0].ats_kind, "personio");
        assert_eq!(hits[0].slug, "acme");
        // The company becomes the display name.
        assert_eq!(hits[0].display_name.as_deref(), Some("Acme GmbH"));
        assert_eq!(
            hits[0].source, "scrape",
            "a single-resolve harvest is source 'scrape'"
        );
    }

    /// ADR-030 §c/§d acceptance: a mixed batch of realistic aggregator/board posting
    /// URLs, run through the real `harvest_ats_refs` into a temp
    /// `DiscoveredCompanyStore`, makes the harvested slugs discoverable via `search`
    /// — the exact read the typeahead does. The URL→(ats, slug) mapping is NEVER
    /// re-derived; it flows through `extract_ats_ref` (ADR-029 shared-seam lesson).
    #[test]
    fn harvested_posting_urls_surface_their_slugs_in_search() {
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
        harvest_ats_refs(
            &store,
            postings
                .iter()
                .map(|(url, company)| ((*url).to_string(), (*company).to_string())),
            "scrape",
        );

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
    /// can't reach, since a dropped item just doesn't appear). A non-ATS URL maps to
    /// `None` regardless of the company.
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
}
