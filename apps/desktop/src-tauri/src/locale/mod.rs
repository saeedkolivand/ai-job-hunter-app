//! Locale profiles — per-market document conventions: page size and photo policy.
//!
//! Page size feeds both PDF and DOCX backends (A4 vs US Letter).  Photo policy
//! and privacy rules (photo/PII) are surfaced to the AI prompt and UI.
//! Phase 1 ships the `en` default (A4, photos Never) plus the types; the full
//! registry (US, UK, DE/AT/CH, FR, NL, generic-EU/INTL) is populated in Phase 7.
//!
//! Privacy: photo is **user-supplied only** — never inferred or auto-added.
#![allow(dead_code)]

pub mod letter;

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
    pub photo: PhotoPolicy,
}

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
        // Try both the leading token and the trailing token — whichever matches a
        // known region wins.  This handles both `en-US` (leading `en` → intl but
        // trailing `us` → US Letter) and `de-AT` (leading `de` → DACH) correctly.
        // Prefer the leading token when both match (family-first: `de-AT` → `de`).
        let mut parts = key.split(['-', '_']).filter(|s| s.len() == 2);
        let first = parts.next().unwrap_or(key.as_str());
        let last = parts.next_back().unwrap_or(first);
        let region = if Self::is_known_region(first) {
            first
        } else if Self::is_known_region(last) {
            last
        } else {
            key.as_str()
        };
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

    /// Returns `true` when `token` is a supported region/market code.
    fn is_known_region(token: &str) -> bool {
        matches!(
            token,
            "us" | "uk" | "gb" | "de" | "at" | "ch" | "fr" | "nl" | "eu" | "dach"
        )
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
            photo: PhotoPolicy::Never,
        }
    }

    /// United States — US Letter, no photo.
    pub fn us() -> LocaleProfile {
        LocaleProfile {
            id: "us",
            page_size: PageSize::Letter,
            photo: PhotoPolicy::Never,
        }
    }

    /// United Kingdom — A4, no photo.
    pub fn uk() -> LocaleProfile {
        LocaleProfile {
            id: "uk",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Never,
        }
    }

    /// DACH (DE/AT/CH) — A4, photo common.
    pub fn dach() -> LocaleProfile {
        LocaleProfile {
            id: "dach",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Common,
        }
    }

    /// France — A4, photo optional.
    pub fn fr() -> LocaleProfile {
        LocaleProfile {
            id: "fr",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Optional,
        }
    }

    /// Netherlands — A4, photo optional.
    pub fn nl() -> LocaleProfile {
        LocaleProfile {
            id: "nl",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Optional,
        }
    }

    /// Generic EU — A4, photo optional.
    pub fn eu() -> LocaleProfile {
        LocaleProfile {
            id: "eu",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Optional,
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
    fn us_is_letter_sized_without_photo() {
        let us = LocaleProfile::get("us");
        assert_eq!(us.page_size, PageSize::Letter);
        assert_eq!(us.photo, PhotoPolicy::Never);
    }

    #[test]
    fn dach_uses_photo_common() {
        let de = LocaleProfile::get("de");
        assert_eq!(de.id, "dach");
        assert_eq!(de.page_size, PageSize::A4);
        assert_eq!(de.photo, PhotoPolicy::Common);
        // AT and CH resolve to the same DACH profile.
        assert_eq!(LocaleProfile::get("at"), de);
        assert_eq!(LocaleProfile::get("ch"), de);
    }

    #[test]
    fn region_is_parsed_from_locale_tags_case_insensitively() {
        // Trailing region token (en-US → US → Letter).
        assert_eq!(LocaleProfile::get("en-US"), LocaleProfile::us());
        // Leading language token (de_AT → de → DACH; at also matches but de comes first).
        assert_eq!(LocaleProfile::get("de_AT"), LocaleProfile::dach());
        // Single 2-char code (GB → uk).
        assert_eq!(LocaleProfile::get("GB"), LocaleProfile::uk());
    }

    #[test]
    fn leading_region_wins_when_trailing_is_unknown() {
        // "fr-CA" — trailing "ca" is not a known region, leading "fr" is → France.
        assert_eq!(LocaleProfile::get("fr-CA"), LocaleProfile::fr());
        // "nl-BE" — trailing "be" is not a known region, leading "nl" is → Netherlands.
        assert_eq!(LocaleProfile::get("nl-BE"), LocaleProfile::nl());
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
