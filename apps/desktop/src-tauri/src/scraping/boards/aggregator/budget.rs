//! The two independent search budgets (`SearchBudget`) and the Apify spend
//! cap derived from them — split out of `mod.rs` (R8 module-size guard).

use crate::scraping::types::BoardSearchInput;

use super::apify::APIFY_MAX_ITEMS;

// ── Search budgets ────────────────────────────────────────────────────────────

/// The two INDEPENDENT budgets one aggregator search carries. They are separate
/// fields (not one number) because conflating them is a live cost bug: `amount`
/// is a sentinel ceiling for callers with no item-count intent, so spending
/// upstream calls against it silently multiplies every scheduled run's bill.
///
/// Passed as one struct rather than two parameters so the provider-orchestration
/// fns stay within the crate's 8-argument clippy ceiling (`clippy.toml`).
#[derive(Clone, Copy)]
pub(super) struct SearchBudget {
    /// OUTPUT cap — how many postings the caller will keep, and NOTHING else.
    /// It buys no upstream call on any tier (paid ones included): it is a
    /// "don't cap me" sentinel on the scheduled path, so spending against it is
    /// precisely the cost bug this type exists to prevent.
    pub(super) amount: usize,
    /// UPSTREAM SPEND target — see `BoardSearchInput::provider_amount`. The ONLY
    /// field that buys upstream calls. `None` (the default, and every scheduled
    /// run) keeps each metered provider at its cheapest single-request form and
    /// skips the paid LinkedIn tier entirely (see [`apify_cap`]).
    pub(super) provider_amount: Option<u32>,
}

impl SearchBudget {
    /// The ONLY production translation from a request to a spend budget, and the
    /// single line where the cost bug this type exists to prevent could reappear.
    /// Deliberately not inlined at the call site: hand-built `SearchBudget`s in
    /// tests cannot observe this hop, so a guard that constructs one directly
    /// would keep passing even if this mapping were rewritten to spend `amount`.
    /// `budget_from_autopilot_shaped_input_has_no_provider_spend` pins it.
    ///
    /// `amount` becomes the output cap verbatim; the upstream budget is passed
    /// through UNCHANGED (`None` stays `None`) — it is never derived from
    /// `amount` or `pages`, both of which are sentinels on one caller or the
    /// other (see `BoardSearchInput`).
    pub(super) fn from_input(input: &BoardSearchInput) -> Self {
        Self {
            amount: input.amount as usize,
            provider_amount: input.provider_amount,
        }
    }

    /// Test-only constructor. Production goes through [`Self::from_input`], so
    /// this cannot be the thing a mutation of the real mapping slips past.
    #[cfg(test)]
    pub(crate) fn new(amount: usize, provider_amount: Option<u32>) -> Self {
        Self {
            amount,
            provider_amount,
        }
    }

    /// Output cap only, no upstream spend target — the quota-neutral shape a
    /// scheduled run uses, and the default for tests that only exercise the
    /// fallback chain with fake providers.
    #[cfg(test)]
    pub(crate) fn items_only(amount: usize) -> Self {
        Self::new(amount, None)
    }

    /// MANUAL-search shape: one user-typed count that is both the output cap and
    /// the upstream spend target (exactly what `commands::scrape` builds). The
    /// shape any PAID-tier test needs — with `items_only` the aggregator buys
    /// nothing upstream, so an Apify assertion would pass vacuously.
    #[cfg(test)]
    pub(crate) fn manual(amount: usize) -> Self {
        Self::new(amount, Some(amount as u32))
    }
}

/// How many items the paid LinkedIn (Apify) tier may be asked for — `0` meaning
/// "do not call it at all". The single place the paid tier's spend is decided,
/// pulled out of [`search_with_providers`] so the cost contract is assertable
/// without a network call.
///
/// It rides `provider_amount`, NEVER `amount`:
/// * `provider_amount: None` → `0`. Apify bills per dataset result, so it is an
///   UPSTREAM SPEND, and `amount` is a "don't cap me" sentinel on the scheduled
///   path (`amount: 100`, no item-count intent). Gating a paid run on that
///   sentinel meant every scheduled run bought a full 50-item actor run forever —
///   the exact class of bug `SearchBudget` exists to prevent.
/// * `Some(spend)` → the still-UNMET part of that budget (`spend - primary_len`),
///   clamped to [`APIFY_MAX_ITEMS`]. A primary result that already meets the
///   budget yields `0`, so LinkedIn stays a fill for unmet capacity.
///
/// BEHAVIOR CHANGE (deliberate, PR #896 review): an Apify-enabled user's
/// SCHEDULED runs no longer buy LinkedIn results — only manual searches, which
/// are the ones carrying a real, user-typed spend target. The opt-in toggle
/// still gates whether Apify may run at all (`is_configured`); this decides
/// whether the current search has money to spend on it.
pub(super) fn apify_cap(budget: SearchBudget, primary_len: usize) -> u32 {
    let Some(spend) = budget.provider_amount else {
        return 0;
    };
    (spend as usize)
        .saturating_sub(primary_len)
        .min(APIFY_MAX_ITEMS as usize) as u32
}
