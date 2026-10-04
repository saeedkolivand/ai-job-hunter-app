//! Pure model transforms: ATS linearization and locale-driven section reordering.

use super::document::{DocumentModel, SectionId};
use crate::locale::resume::section_order_for;

/// Reorder `model.sections` to follow `order`. Sections whose id isn't in
/// `order` keep their original relative order and are appended after the ordered
/// ones. Stable and non-dropping.
pub fn reorder_sections(model: &mut DocumentModel, order: &[SectionId]) {
    model.sections.sort_by_key(|s| {
        order
            .iter()
            .position(|id| id == &s.id)
            .unwrap_or(usize::MAX)
    });
}

/// Reorder the model into a market-aware, single-column reading order in place.
///
/// In ATS mode the visual layout collapses to one column (a theme/layout
/// concern); this guarantees the underlying section sequence reads sensibly
/// top-to-bottom regardless of where the theme would have placed each section.
/// The order itself is resolved from `market` by
/// [`crate::locale::resume::section_order_for`] — the SAME source
/// `pipeline::resume::prompts::draft_system` injects into the draft prompt,
/// so the model's section order and the exporter's order can never disagree.
pub fn linearize(model: &mut DocumentModel, market: &str) {
    reorder_sections(model, section_order_for(market));
}

#[cfg(test)]
mod tests;
