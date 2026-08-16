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
mod tests {
    use super::*;
    use crate::export::types::DocumentType;
    use crate::model::document::{Block, Section};

    fn section(id: SectionId) -> Section {
        Section {
            heading: format!("{id:?}"),
            id,
            blocks: vec![Block::Paragraph(Vec::new())],
        }
    }

    fn model_with(ids: &[SectionId]) -> DocumentModel {
        let mut m = DocumentModel::new(DocumentType::Resume);
        m.sections = ids.iter().cloned().map(section).collect();
        m
    }

    fn ids(model: &DocumentModel) -> Vec<SectionId> {
        model.sections.iter().map(|s| s.id.clone()).collect()
    }

    #[test]
    fn linearize_orders_sections_for_ats_reading() {
        let mut m = model_with(&[
            SectionId::Skills,
            SectionId::Education,
            SectionId::Experience,
            SectionId::Summary,
        ]);
        // Default market: reverse-chronological, Experience before Skills.
        linearize(&mut m, "us");
        assert_eq!(
            ids(&m),
            vec![
                SectionId::Summary,
                SectionId::Experience,
                SectionId::Skills,
                SectionId::Education,
            ]
        );
    }

    /// Two markets really do disagree on the order, AND a section `linearize`
    /// has never heard of (`Custom`) still survives and lands last — under
    /// BOTH orders, not just the default one.
    #[test]
    fn linearize_is_market_aware_and_never_drops_a_custom_section() {
        // Both orders lead with Experience, so Skills-vs-Education is what
        // actually discriminates them: default runs Skills then Education,
        // DE runs Education (Ausbildung) then Skills (Kenntnisse).
        let ordered = |market: &str| {
            let mut m = model_with(&[
                SectionId::Custom("Speaking".into()),
                SectionId::Education,
                SectionId::Skills,
                SectionId::Experience,
                SectionId::Summary,
            ]);
            linearize(&mut m, market);
            ids(&m)
        };

        assert_eq!(
            ordered("us"),
            vec![
                SectionId::Summary,
                SectionId::Experience,
                SectionId::Skills,
                SectionId::Education,
                SectionId::Custom("Speaking".into()),
            ],
            "default market: skills before education"
        );
        assert_eq!(
            ordered("de"),
            vec![
                SectionId::Summary,
                SectionId::Experience,
                SectionId::Education,
                SectionId::Skills,
                SectionId::Custom("Speaking".into()),
            ],
            "DE market: education before skills"
        );
    }

    #[test]
    fn reorder_appends_unlisted_sections_in_original_order() {
        let mut m = model_with(&[
            SectionId::Custom("Speaking".into()),
            SectionId::Experience,
            SectionId::Custom("Patents".into()),
            SectionId::Summary,
        ]);
        reorder_sections(&mut m, &[SectionId::Summary, SectionId::Experience]);
        assert_eq!(
            ids(&m),
            vec![
                SectionId::Summary,
                SectionId::Experience,
                SectionId::Custom("Speaking".into()),
                SectionId::Custom("Patents".into()),
            ]
        );
    }
}
