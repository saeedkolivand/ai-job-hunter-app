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
/// Default: sidebar-leaning sections (skills / education / languages /
/// certifications) go to the sidebar; everything else flows in the main column.
/// Contact details live in the document header (handled by the layout engine), so
/// they are not a [`SectionId`] here.
///
/// Per-template overrides pull a specific section back into the main column:
/// **Aria** keeps Education in the main column; **Saffron** keeps Certifications
/// in the main column. Atelier / Portrait use the default table unchanged.
pub fn placement_for(template_id: TemplateId, id: &SectionId) -> Placement {
    match (template_id, id) {
        // Aria: Education reads in the main column (design choice).
        (TemplateId::Aria, SectionId::Education) => Placement::Main,
        // Saffron: Certifications read in the main column.
        (TemplateId::Saffron, SectionId::Certifications) => Placement::Main,
        // Default sidebar-leaning set for every other (template, section) pair.
        (
            _,
            SectionId::Skills
            | SectionId::Education
            | SectionId::Languages
            | SectionId::Certifications,
        ) => Placement::Sidebar,
        _ => Placement::Main,
    }
}

/// Returns `true` when a template uses a two-column layout.
///
/// `Atelier` (Phase 1b), `Portrait` (Phase 3b-i), and `Aria` / `Saffron` (PR4)
/// are the live two-column templates.  In ATS mode they collapse to a single
/// linear column.
pub fn is_two_column(id: TemplateId) -> bool {
    matches!(
        id,
        TemplateId::Atelier | TemplateId::Portrait | TemplateId::Aria | TemplateId::Saffron
    )
}

/// Returns `true` when a template carries a **droppable** filled header band —
/// the roster both renderers key their header band off, so PDF and DOCX cannot
/// disagree about it. The structural counterpart to [`is_two_column`]: a
/// per-template layout fact the backends ask about instead of each re-deriving
/// it from its own `TemplateId` match.
///
/// `Awesome` is the only member. `awesome.typ` paints the band into
/// `page.background`; `model_docx::add_header` approximates it with
/// paragraph-level shading (`docx-rs` has no page-background primitive).
///
/// **`Meridian` is a deliberate non-member even though `meridian.typ` does draw
/// a full-width 38 mm accent band.** The band here has to be *droppable*: both
/// renderers drop it in ATS mode, and the ATS-mode toggle only surfaces for
/// **design-tier** templates (`TemplateTier`). Meridian is ATS-tier, so a DOCX
/// band on it could never be turned off — it would permanently shade the header
/// of a template whose whole promise is that it is plain. Its PDF band is a
/// pre-existing PDF-only divergence; widening this predicate to "the PDF paints
/// a band" would change Meridian's DOCX output and break that promise.
/// `header_band_templates_are_all_design_tier` pins the rule.
///
/// WHETHER to draw the band (the `ats_mode` gate) stays with the caller — that
/// is a per-export mode decision, not a template property.
pub fn has_header_band(id: TemplateId) -> bool {
    matches!(id, TemplateId::Awesome)
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
mod tests;
