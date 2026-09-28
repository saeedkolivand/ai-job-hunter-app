//! Cross-source dedup (trust PR E, stage 1): collapse the same job
//! surfaced by two boards into one row. Split out of `mod.rs` (issue #1280).

use std::collections::HashMap;

use crate::scraping::types::JobPosting;

/// Collapse cross-source duplicate postings into one, keyed by the app-wide
/// [`canonical_job_key`](crate::scraping::boards::common::canonical_job_key). Boards run
/// independently, so a job surfaced by two of them lands in the concatenated
/// result as separate rows; this is the single cross-board dedup pass
/// (trust-program PR E, stage 1).
///
/// **Survivor policy — field-level upgrade, incumbent identity kept.** The
/// FIRST-seen posting for a key is the incumbent and stays in its original slot
/// (order-preserving, stable) — its `id`/`url`/`source` (board attribution) are
/// NEVER overwritten by a later duplicate, only two things are merged in from
/// each challenger:
///   - `description` — upgraded when the challenger's is longer (a fuller
///     description scores and reads better, and aggregator snippets are often
///     truncated — trust-audit root cause 6 — so a board returning full text
///     should win the description regardless of which came first);
///   - `extra` — unioned key-by-key: a non-empty (non-null, non-`""`) incumbent
///     value is kept as-is; only a key the incumbent lacks (or holds
///     null/empty for) is filled from the challenger.
///
/// This mirrors the within-batch upgrade step in `autopilot::merge_found_jobs`
/// and is deliberately NOT a whole-struct replace: a full swap would silently
/// discard every field only the incumbent had — concretely, Adzuna rows carry
/// `extra["salaryMin"/"salaryMax"/"salaryCurrency"]` (consumed by
/// usePostingActions/TailorFlow/ApplicationDetailPage) that most direct boards
/// don't, so a direct board winning purely on description length must not
/// delete the salary.
///
/// Per-board `BoardScrapeSummary.count`s are intentionally NOT adjusted here:
/// each describes what that board returned (as-fetched), so after this pass the
/// per-board counts may not sum to the deduped total — that difference IS the
/// cross-source overlap, logged by the caller.
///
/// Pure (no I/O) so the survivor policy is directly unit-testable.
pub(super) fn dedup_cross_source(postings: Vec<JobPosting>) -> Vec<JobPosting> {
    let mut index_of: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<JobPosting> = Vec::with_capacity(postings.len());
    for p in postings {
        let key = crate::scraping::boards::common::canonical_job_key(&p.url, &p.title, &p.company);
        match index_of.get(&key) {
            Some(&i) => {
                // Field-level upgrade only — incumbent id/url/source (board
                // attribution) are left untouched.
                if desc_len(&p) > desc_len(&out[i]) {
                    out[i].description = p.description;
                }
                // Same fill-without-clobbering pattern as the `extra` union below:
                // a dateless incumbent (e.g. a direct board with no publish-date
                // field) picks up the challenger's date rather than losing it
                // permanently once collapsed; an incumbent that already has one
                // keeps it.
                if out[i].posted_at.is_none() {
                    out[i].posted_at = p.posted_at;
                }
                merge_extra(&mut out[i].extra, p.extra);
            }
            None => {
                index_of.insert(key, out.len());
                out.push(p);
            }
        }
    }
    out
}

/// Byte length of a posting's description (`None`/absent → 0). The description-
/// upgrade tiebreak in [`dedup_cross_source`].
fn desc_len(p: &JobPosting) -> usize {
    p.description.as_deref().map(str::len).unwrap_or(0)
}

/// Union a challenger posting's `extra` map into the incumbent's, in place. A
/// key the incumbent already holds a non-empty value for (anything but
/// JSON `null` or `""`) is left untouched; a key the incumbent lacks, or holds
/// null/empty for, is filled from the challenger. Never removes an incumbent
/// key the challenger doesn't have.
///
/// Unions arbitrary extra keys; the TS mirror
/// (`features/jobs/lib/merge-postings.ts` `collapseDuplicate`) fills a FIXED
/// field list instead — any NEW key a board writes into `JobPosting.extra`
/// must be added to that TS fill-list too (lockstep pair).
fn merge_extra(
    incumbent: &mut HashMap<String, serde_json::Value>,
    challenger: HashMap<String, serde_json::Value>,
) {
    for (k, v) in challenger {
        let incumbent_is_empty = incumbent
            .get(&k)
            .map(|existing| existing.is_null() || existing.as_str() == Some(""))
            .unwrap_or(true);
        if incumbent_is_empty {
            incumbent.insert(k, v);
        }
    }
}
