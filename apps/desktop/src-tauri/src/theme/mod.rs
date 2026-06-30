//! Theme — the single source of truth for per-template styling and the
//! structural rules the canonical layout engine needs (section placement, link
//! styling).
//!
//! Phase 1 introduces `theme` as the canonical *seam* alongside the existing
//! renderers: it exposes the template registry via [`template`] and adds the
//! model-level decisions (`placement_for`, `link_style`) that the new layout
//! path will consume. The `Template` data itself still lives in
//! `export::templates` (which the legacy PDF/DOCX renderers import directly); it
//! migrates physically into this module when the backends move onto
//! `DocumentModel` in Phase 2, so nothing here changes current behavior.
#![allow(dead_code)]

use crate::export::templates::Template;
use crate::export::types::TemplateId;
use crate::model::document::{Placement, SectionId};

/// Canonical accessor for a template's style data. Thin wrapper over
/// [`Template::get`] so callers depend on `theme`, not the legacy module path.
pub fn template(id: TemplateId) -> Template {
    Template::get(id)
}

/// Column placement for a section in a two-column layout — the single source of
/// truth for sidebar classification (the per-template `sidebar_sections` list and
/// its legacy printpdf renderer are gone).
///
/// Sidebar-leaning sections (skills / education / languages / certifications) go
/// to the sidebar; everything else flows in the main column. Contact details
/// live in the document header (handled by the layout engine), so they are not a
/// [`SectionId`] here.
pub fn placement_for(id: &SectionId) -> Placement {
    match id {
        SectionId::Skills
        | SectionId::Education
        | SectionId::Languages
        | SectionId::Certifications => Placement::Sidebar,
        _ => Placement::Main,
    }
}

/// Returns `true` when a template uses a two-column layout.
///
/// `Atelier` (Phase 1b) and `Portrait` (Phase 3b-i) are the two live two-column
/// templates.  In ATS mode they collapse to a single linear column.
pub fn is_two_column(id: TemplateId) -> bool {
    matches!(id, TemplateId::Atelier | TemplateId::Portrait)
}

/// How hyperlinks render for a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkStyle {
    /// Draw links in the template accent color (vs. plain body-text color).
    pub use_accent: bool,
    /// Underline links.
    pub underline: bool,
}

/// Link styling per template: accent color + underline by default. The ATS
/// Classic template keeps links in body color with no underline, for maximum
/// parser/printer safety.
pub fn link_style(id: TemplateId) -> LinkStyle {
    match id {
        TemplateId::Classic => LinkStyle {
            use_accent: false,
            underline: false,
        },
        _ => LinkStyle {
            use_accent: true,
            underline: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_accessor_matches_registry() {
        assert_eq!(template(TemplateId::Classic).id, TemplateId::Classic);
        assert_eq!(template(TemplateId::Atelier).id, TemplateId::Atelier);
    }

    #[test]
    fn sidebar_sections_go_to_the_sidebar() {
        for id in [
            SectionId::Skills,
            SectionId::Education,
            SectionId::Languages,
            SectionId::Certifications,
        ] {
            assert_eq!(
                placement_for(&id),
                Placement::Sidebar,
                "{id:?} should be sidebar"
            );
        }
    }

    #[test]
    fn main_sections_stay_in_the_main_column() {
        for id in [
            SectionId::Summary,
            SectionId::Experience,
            SectionId::Projects,
        ] {
            assert_eq!(placement_for(&id), Placement::Main, "{id:?} should be main");
        }
        assert_eq!(
            placement_for(&SectionId::Custom("Patents".into())),
            Placement::Main
        );
    }

    #[test]
    fn two_column_only_for_two_column_templates() {
        assert!(is_two_column(TemplateId::Atelier), "Atelier is two-column");
        assert!(
            is_two_column(TemplateId::Portrait),
            "Portrait is two-column"
        );
        assert!(!is_two_column(TemplateId::Classic));
        assert!(!is_two_column(TemplateId::Modern));
        assert!(
            !is_two_column(TemplateId::Lebenslauf),
            "Lebenslauf is single-column"
        );
    }

    #[test]
    fn classic_links_are_plain_others_accented() {
        assert_eq!(
            link_style(TemplateId::Classic),
            LinkStyle {
                use_accent: false,
                underline: false
            }
        );
        let modern = link_style(TemplateId::Modern);
        assert!(modern.use_accent && modern.underline);
    }
}
