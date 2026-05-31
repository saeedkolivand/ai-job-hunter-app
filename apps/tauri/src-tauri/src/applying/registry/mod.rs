//! Applier registry — looks up the right `Applier` by board id.

use super::boards::{
    glassdoor::GlassdoorApplier, greenhouse::GreenhouseApplier, indeed::IndeedApplier,
    linkedin::LinkedInApplier, workday::WorkdayApplier, xing::XingApplier,
};
use super::types::Applier;

pub struct ApplierRegistry;

/// Every applier, registered exactly once (catalog display order). Both dispatch
/// ([`ApplierRegistry::get`]) and the catalog derive from this list via the
/// `Applier` trait — no parallel match or hardcoded array.
static APPLIERS: &[&dyn Applier] = &[
    &LinkedInApplier,
    &IndeedApplier,
    &GreenhouseApplier,
    &WorkdayApplier,
    &XingApplier,
    &GlassdoorApplier,
];

impl ApplierRegistry {
    pub fn catalog() -> Vec<(&'static str, &'static str)> {
        APPLIERS
            .iter()
            .map(|a| (a.board_id(), a.display_name()))
            .collect()
    }

    pub fn get(board_id: &str) -> Option<&'static dyn Applier> {
        APPLIERS.iter().copied().find(|a| a.board_id() == board_id)
    }
}

#[cfg(test)]
mod test;
