//! Locale profiles — per-market document conventions: page size, date style,
//! photo/PII policy, and default section order.
//!
//! Page size + date style feed both backends; section order + photo/PII feed the
//! AI prompt and model build. Phase 1 ships the `en` default (A4, photos Never,
//! no personal details) plus the types; the full registry (US, UK, DE/AT/CH, FR,
//! NL, generic-EU/INTL) is populated in Phase 7.
//!
//! Privacy: photo / personal details are **user-supplied only** — never inferred
//! or auto-added.
#![allow(dead_code)]

use crate::model::document::SectionId;

/// Physical page size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    A4,
    Letter,
}

/// Page dimensions in millimetres, derived from a [`PageSize`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    pub width_mm: f32,
    pub height_mm: f32,
}

impl PageSize {
    /// Physical dimensions for this page size, in mm.
    pub fn geometry(self) -> PageGeometry {
        match self {
            PageSize::A4 => PageGeometry {
                width_mm: 210.0,
                height_mm: 297.0,
            },
            PageSize::Letter => PageGeometry {
                width_mm: 215.9,
                height_mm: 279.4,
            },
        }
    }
}

/// How date ranges are written in the target market.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle {
    /// "January 2021 – March 2023"
    MonthYear,
    /// "01/2021 – 03/2023"
    NumericSlash,
    /// "2021-01 – 2023-03"
    IsoMonth,
}

/// Whether a photo is customary on a CV in this market.
/// User-supplied only — never inferred or auto-added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoPolicy {
    Never,
    Optional,
    Common,
}

/// Per-market document conventions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleProfile {
    /// Market id ("en", "de", "us", …).
    pub id: &'static str,
    pub page_size: PageSize,
    pub date_style: DateStyle,
    pub photo: PhotoPolicy,
    /// Whether personal details (DOB, nationality, marital status) are customary
    /// in this market. User-supplied only.
    pub include_personal_details: bool,
    /// Canonical section ordering for this market (consumed by
    /// `transform::reorder_sections`).
    pub default_section_order: &'static [SectionId],
}

/// Default single-column section order shared by the English/international profile.
const DEFAULT_ORDER: &[SectionId] = &[
    SectionId::Summary,
    SectionId::Experience,
    SectionId::Skills,
    SectionId::Projects,
    SectionId::Education,
    SectionId::Certifications,
    SectionId::Languages,
    SectionId::Awards,
];

impl LocaleProfile {
    /// Page geometry for this profile.
    pub fn page_geometry(&self) -> PageGeometry {
        self.page_size.geometry()
    }

    /// Resolve a profile by market id. Phase 1 only ships the `en` default;
    /// unknown ids fall back to it. Phase 7 populates the other markets.
    pub fn get(id: &str) -> LocaleProfile {
        match id {
            "en" => Self::en(),
            _ => Self::en(),
        }
    }

    /// English / international default: A4, no photo, no personal details.
    pub fn en() -> LocaleProfile {
        LocaleProfile {
            id: "en",
            page_size: PageSize::A4,
            date_style: DateStyle::MonthYear,
            photo: PhotoPolicy::Never,
            include_personal_details: false,
            default_section_order: DEFAULT_ORDER,
        }
    }
}

impl Default for LocaleProfile {
    fn default() -> Self {
        Self::en()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_and_letter_have_expected_dimensions() {
        assert_eq!(
            PageSize::A4.geometry(),
            PageGeometry {
                width_mm: 210.0,
                height_mm: 297.0
            }
        );
        assert_eq!(
            PageSize::Letter.geometry(),
            PageGeometry {
                width_mm: 215.9,
                height_mm: 279.4
            }
        );
    }

    #[test]
    fn default_profile_is_en_a4_never() {
        let p = LocaleProfile::default();
        assert_eq!(p, LocaleProfile::en());
        assert_eq!(p.id, "en");
        assert_eq!(p.page_size, PageSize::A4);
        assert_eq!(p.photo, PhotoPolicy::Never);
        assert!(!p.include_personal_details);
        assert_eq!(
            p.page_geometry(),
            PageGeometry {
                width_mm: 210.0,
                height_mm: 297.0
            }
        );
    }

    #[test]
    fn unknown_market_falls_back_to_en() {
        assert_eq!(LocaleProfile::get("zz"), LocaleProfile::en());
        assert_eq!(LocaleProfile::get("en"), LocaleProfile::en());
    }

    #[test]
    fn default_order_starts_with_summary_then_experience() {
        let order = LocaleProfile::en().default_section_order;
        assert_eq!(order.first(), Some(&SectionId::Summary));
        assert_eq!(order.get(1), Some(&SectionId::Experience));
    }
}
