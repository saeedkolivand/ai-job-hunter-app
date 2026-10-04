//! ATS slug-harvest seam (ADR-030 §c): turns a batch of `(url, company)` posting
//! pairs into [`DiscoveredCompanyStore`] rows, parse-only + zero network.
//!
//! AppHandle-free by design — the L3 command handlers resolve the store via
//! `try_state` and pass it in, so this URL→store seam stays an L1 sibling of
//! `scraping::ats_ref` (the slug authority) and is unit-testable without a mock-app
//! harness.

use super::DiscoveredCompanyStore;
use crate::observability::sanitize_reason;

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
        log::warn!(
            "[discovered] harvest upsert failed ({}); slugs not recorded this ingest",
            sanitize_reason(&e.to_string())
        );
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
mod tests;
