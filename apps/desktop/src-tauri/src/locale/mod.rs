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
pub mod resume;

// Cross-module callers (`export::typst_engine::letter`,
// `validate::content::letter`) reach this through the module root per the
// architecture rules' L1 public-API contract, not the leaf `letter` module.
pub use letter::is_template_placeholder;

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
    /// Customary maximum **length** of a résumé in this market, in pages.
    ///
    /// NOT to be confused with [`Self::page_size`], which is the physical paper
    /// (A4 vs US Letter). This is "how long may the document be", not "how big
    /// is the sheet".
    ///
    /// These are hiring-convention norms, not a specification — no standards
    /// body publishes them. Anglophone markets cap at 2; DACH and the generic
    /// EU/Europass style tolerate 3 because a German `Lebenslauf` conventionally
    /// lists every position with dates rather than summarising. Advisory only:
    /// the trim panel uses it to decide when to *offer* suggestions, and nothing
    /// blocks an export that runs longer.
    pub max_pages: u8,
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
            "it" => Self::it(),
            _ => Self::intl(),
        }
    }

    /// Returns `true` when `token` is a supported region/market code.
    fn is_known_region(token: &str) -> bool {
        matches!(
            token,
            "us" | "uk" | "gb" | "de" | "at" | "ch" | "fr" | "nl" | "eu" | "dach" | "it"
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
            Self::it(),
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
            max_pages: 2,
        }
    }

    /// United States — US Letter, no photo.
    pub fn us() -> LocaleProfile {
        LocaleProfile {
            id: "us",
            page_size: PageSize::Letter,
            photo: PhotoPolicy::Never,
            max_pages: 2,
        }
    }

    /// United Kingdom — A4, no photo.
    pub fn uk() -> LocaleProfile {
        LocaleProfile {
            id: "uk",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Never,
            max_pages: 2,
        }
    }

    /// DACH (DE/AT/CH) — A4, photo common.
    pub fn dach() -> LocaleProfile {
        LocaleProfile {
            id: "dach",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Common,
            max_pages: 3,
        }
    }

    /// France — A4, photo optional.
    pub fn fr() -> LocaleProfile {
        LocaleProfile {
            id: "fr",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Optional,
            max_pages: 2,
        }
    }

    /// Netherlands — A4, photo optional.
    pub fn nl() -> LocaleProfile {
        LocaleProfile {
            id: "nl",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Optional,
            max_pages: 2,
        }
    }

    /// Generic EU — A4, photo optional.
    pub fn eu() -> LocaleProfile {
        LocaleProfile {
            id: "eu",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Optional,
            max_pages: 3,
        }
    }

    /// Italy — A4, photo optional, Europass-length tolerance.
    ///
    /// Every field here numerically matches [`Self::eu()`] — Italy has no
    /// distinct paper size or photo convention beyond the generic-EU/Europass
    /// baseline, and `locale::resume::IT_ORDER`'s doc comment independently
    /// grounds the Italian CV in the same Europass reference format that
    /// justifies `eu()`'s 3-page tolerance (German-style itemised history
    /// rather than a US-style 2-page summary).
    ///
    /// This is still its own constructor, not an `"it" => Self::eu()` alias,
    /// because the **id** must stay distinct: `recommend::pick_locale`
    /// forwards this id verbatim as the export `market` string, and
    /// `locale::resume::section_order_for` matches the literal `"it"` to
    /// select `IT_ORDER`. Aliasing to `eu()` (id `"eu"`) would resolve the
    /// page/photo conventions correctly but silently hand an Italian user the
    /// DEFAULT section order again — the exact bug this profile exists to fix.
    pub fn it() -> LocaleProfile {
        LocaleProfile {
            id: "it",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Optional,
            max_pages: 3,
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
        assert_eq!(all.len(), 8);
        let ids: std::collections::HashSet<&str> = all.iter().map(|p| p.id).collect();
        for id in ["us", "uk", "dach", "fr", "nl", "eu", "it", "en"] {
            assert!(ids.contains(id), "missing market {id}");
        }
    }

    #[test]
    fn non_us_markets_are_a4() {
        for id in ["uk", "de", "fr", "nl", "eu", "it", "intl"] {
            assert_eq!(
                LocaleProfile::get(id).page_size,
                PageSize::A4,
                "{id} should be A4"
            );
        }
    }

    #[test]
    fn max_pages_follows_the_market_not_the_paper_size() {
        // Anglophone + FR/NL cap at 2; DACH, generic-EU, and IT tolerate 3.
        for id in ["us", "uk", "fr", "nl", "en", "zz"] {
            assert_eq!(LocaleProfile::get(id).max_pages, 2, "{id} should cap at 2");
        }
        for id in ["de", "at", "ch", "eu", "it"] {
            assert_eq!(LocaleProfile::get(id).max_pages, 3, "{id} should allow 3");
        }
        // Resolves through locale tags, like every other profile field.
        assert_eq!(LocaleProfile::get("de-AT").max_pages, 3);
        assert_eq!(LocaleProfile::get("en-US").max_pages, 2);
        // US is Letter but still caps at 2 — length is not paper size.
        assert_eq!(LocaleProfile::get("us").page_size, PageSize::Letter);
        assert_eq!(LocaleProfile::get("us").max_pages, 2);
    }

    /// The gap this change closes: before this, `LocaleProfile::get("it")`
    /// fell through to `intl()` (id "en", photo Never, 2 pages), which made
    /// `recommend::pick_locale` forward "en" downstream — the id
    /// `locale::resume::section_order_for` reads to select a market's section
    /// order — so an Italian user's already-committed `IT_ORDER` was
    /// unreachable on the auto-recommendation path.
    #[test]
    fn italy_resolves_to_a_dedicated_profile_not_the_english_default() {
        let it = LocaleProfile::get("it");
        assert_eq!(
            it.id, "it",
            "must not silently collapse to \"en\" or \"eu\""
        );
        assert_ne!(it.id, "en");
        assert_eq!(it.page_size, PageSize::A4);
        assert_eq!(it.photo, PhotoPolicy::Optional);
        assert_eq!(it.max_pages, 3);
        // Case/whitespace/locale-tag handling, like every other market.
        assert_eq!(LocaleProfile::get("IT"), it);
        assert_eq!(LocaleProfile::get("  it  "), it);
        assert_eq!(LocaleProfile::get("it-IT"), it);
    }
}
