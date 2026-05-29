//! Pure model transforms: ATS linearization, locale-driven section reordering,
//! and auto-fix mutators.
//!
//! `reorder_sections` and `linearize` are real and tested now. The auto-fix
//! mutators are stubs (no-ops) with stable signatures — Phase 4 wires their
//! bodies into the validation pipeline so callers can be written against them today.

use super::document::{DocumentModel, SectionId};

/// Canonical single-column reading order used for ATS linearization. Sections
/// not listed keep their relative order and follow the listed ones.
const ATS_ORDER: &[SectionId] = &[
    SectionId::Summary,
    SectionId::Experience,
    SectionId::Skills,
    SectionId::Projects,
    SectionId::Education,
    SectionId::Certifications,
    SectionId::Languages,
    SectionId::Awards,
    SectionId::Publications,
];

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

/// Reorder the model into an ATS-safe, single-column reading order in place.
///
/// In ATS mode the visual layout collapses to one column (a theme/layout
/// concern); this guarantees the underlying section sequence reads sensibly
/// top-to-bottom regardless of where the theme would have placed each section.
pub fn linearize(model: &mut DocumentModel) {
    reorder_sections(model, ATS_ORDER);
}

/// Move a section heading orphaned at the bottom of a column next to its content.
///
/// Stub: implemented in Phase 4 (validation auto-fix). No-op today.
pub fn move_orphan_headers(_model: &mut DocumentModel) {}

/// Re-flow content that overflows the available page area.
///
/// Stub: implemented in Phase 4 (validation auto-fix). No-op today.
pub fn reflow_overflow(_model: &mut DocumentModel) {}

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
        linearize(&mut m);
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

    #[test]
    fn autofix_stubs_do_not_drop_content() {
        let mut m = model_with(&[SectionId::Summary, SectionId::Experience]);
        move_orphan_headers(&mut m);
        reflow_overflow(&mut m);
        assert_eq!(ids(&m), vec![SectionId::Summary, SectionId::Experience]);
    }
}
