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

/// US résumés lead with experience and skills; education trails.
const US_ORDER: &[SectionId] = &[
    SectionId::Summary,
    SectionId::Experience,
    SectionId::Skills,
    SectionId::Projects,
    SectionId::Education,
    SectionId::Certifications,
    SectionId::Awards,
    SectionId::Languages,
];

/// UK/EU CVs give education more prominence (right after experience).
const EDUCATION_FORWARD_ORDER: &[SectionId] = &[
    SectionId::Summary,
    SectionId::Experience,
    SectionId::Education,
    SectionId::Skills,
    SectionId::Languages,
    SectionId::Certifications,
    SectionId::Projects,
    SectionId::Awards,
];

impl LocaleProfile {
    /// Page geometry for this profile.
    pub fn page_geometry(&self) -> PageGeometry {
        self.page_size.geometry()
    }

    /// Resolve a profile by market id (case-insensitive; accepts country codes,
    /// `en-US`-style tags, and family names). Unknown ids fall back to the
    /// international default, so a new/unsupported market always works.
    pub fn get(id: &str) -> LocaleProfile {
        let key = id.trim().to_lowercase();
        // Match on the leading region/country token (`de-at` → `de`, `en_us` → `en`).
        let region = key
            .split(['-', '_'])
            .next_back()
            .filter(|s| s.len() == 2)
            .unwrap_or(key.as_str());
        match region {
            "us" => Self::us(),
            "uk" | "gb" => Self::uk(),
            "de" | "at" | "ch" | "dach" => Self::dach(),
            "fr" => Self::fr(),
            "nl" => Self::nl(),
            "eu" => Self::eu(),
            _ => Self::intl(),
        }
    }

    /// Every supported market profile (for the recommender and UI pickers).
    pub fn all() -> Vec<LocaleProfile> {
        vec![
            Self::us(),
            Self::uk(),
            Self::dach(),
            Self::fr(),
            Self::nl(),
            Self::eu(),
            Self::intl(),
        ]
    }

    /// English / international default: A4, no photo, no personal details.
    /// Retained as the `Default` and the backward-compatible `en` profile.
    pub fn en() -> LocaleProfile {
        Self::intl()
    }

    /// International default — the safe, photo-free, A4 baseline.
    pub fn intl() -> LocaleProfile {
        LocaleProfile {
            id: "en",
            page_size: PageSize::A4,
            date_style: DateStyle::MonthYear,
            photo: PhotoPolicy::Never,
            include_personal_details: false,
            default_section_order: DEFAULT_ORDER,
        }
    }

    /// United States — US Letter, no photo, no personal details.
    pub fn us() -> LocaleProfile {
        LocaleProfile {
            id: "us",
            page_size: PageSize::Letter,
            date_style: DateStyle::MonthYear,
            photo: PhotoPolicy::Never,
            include_personal_details: false,
            default_section_order: US_ORDER,
        }
    }

    /// United Kingdom — A4, no photo, education-forward.
    pub fn uk() -> LocaleProfile {
        LocaleProfile {
            id: "uk",
            page_size: PageSize::A4,
            date_style: DateStyle::MonthYear,
            photo: PhotoPolicy::Never,
            include_personal_details: false,
            default_section_order: EDUCATION_FORWARD_ORDER,
        }
    }

    /// DACH (DE/AT/CH) — A4, photo common, personal details customary.
    pub fn dach() -> LocaleProfile {
        LocaleProfile {
            id: "dach",
            page_size: PageSize::A4,
            date_style: DateStyle::NumericSlash,
            photo: PhotoPolicy::Common,
            include_personal_details: true,
            default_section_order: EDUCATION_FORWARD_ORDER,
        }
    }

    /// France — A4, photo optional, personal details customary.
    pub fn fr() -> LocaleProfile {
        LocaleProfile {
            id: "fr",
            page_size: PageSize::A4,
            date_style: DateStyle::NumericSlash,
            photo: PhotoPolicy::Optional,
            include_personal_details: true,
            default_section_order: EDUCATION_FORWARD_ORDER,
        }
    }

    /// Netherlands — A4, photo optional, no personal details.
    pub fn nl() -> LocaleProfile {
        LocaleProfile {
            id: "nl",
            page_size: PageSize::A4,
            date_style: DateStyle::MonthYear,
            photo: PhotoPolicy::Optional,
            include_personal_details: false,
            default_section_order: EDUCATION_FORWARD_ORDER,
        }
    }

    /// Generic EU — A4, photo optional, education-forward.
    pub fn eu() -> LocaleProfile {
        LocaleProfile {
            id: "eu",
            page_size: PageSize::A4,
            date_style: DateStyle::MonthYear,
            photo: PhotoPolicy::Optional,
            include_personal_details: false,
            default_section_order: EDUCATION_FORWARD_ORDER,
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

    #[test]
    fn us_is_letter_sized_without_photo() {
        let us = LocaleProfile::get("us");
        assert_eq!(us.page_size, PageSize::Letter);
        assert_eq!(us.photo, PhotoPolicy::Never);
        assert!(!us.include_personal_details);
    }

    #[test]
    fn dach_uses_photo_and_personal_details() {
        let de = LocaleProfile::get("de");
        assert_eq!(de.id, "dach");
        assert_eq!(de.page_size, PageSize::A4);
        assert_eq!(de.photo, PhotoPolicy::Common);
        assert!(de.include_personal_details);
        // AT and CH resolve to the same DACH profile.
        assert_eq!(LocaleProfile::get("at"), de);
        assert_eq!(LocaleProfile::get("ch"), de);
    }

    #[test]
    fn region_is_parsed_from_locale_tags_case_insensitively() {
        assert_eq!(LocaleProfile::get("en-US"), LocaleProfile::us());
        assert_eq!(LocaleProfile::get("de_AT"), LocaleProfile::dach());
        assert_eq!(LocaleProfile::get("GB"), LocaleProfile::uk());
    }

    #[test]
    fn all_markets_are_distinct_and_present() {
        let all = LocaleProfile::all();
        assert_eq!(all.len(), 7);
        let ids: std::collections::HashSet<&str> = all.iter().map(|p| p.id).collect();
        for id in ["us", "uk", "dach", "fr", "nl", "eu", "en"] {
            assert!(ids.contains(id), "missing market {id}");
        }
    }

    #[test]
    fn non_us_markets_are_a4() {
        for id in ["uk", "de", "fr", "nl", "eu", "intl"] {
            assert_eq!(
                LocaleProfile::get(id).page_size,
                PageSize::A4,
                "{id} should be A4"
            );
        }
    }
}
