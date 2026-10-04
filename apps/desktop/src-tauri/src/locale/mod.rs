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
            "es" => Self::es(),
            "pt" => Self::pt(),
            "br" => Self::br(),
            _ => Self::intl(),
        }
    }

    /// Returns `true` when `token` is a supported region/market code.
    fn is_known_region(token: &str) -> bool {
        matches!(
            token,
            "us" | "uk"
                | "gb"
                | "de"
                | "at"
                | "ch"
                | "fr"
                | "nl"
                | "eu"
                | "dach"
                | "it"
                | "es"
                | "pt"
                | "br"
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
            Self::es(),
            Self::pt(),
            Self::br(),
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
    /// baseline, and `locale::resume::EUROPASS_ORDER`'s doc comment independently
    /// grounds the Italian CV in the same Europass reference format that
    /// justifies `eu()`'s 3-page tolerance (German-style itemised history
    /// rather than a US-style 2-page summary).
    ///
    /// This is still its own constructor, not an `"it" => Self::eu()` alias,
    /// because the **id** must stay distinct: `recommend::pick_locale`
    /// forwards this id verbatim as the export `market` string, and
    /// `locale::resume::section_order_for` matches the literal `"it"` to
    /// select `EUROPASS_ORDER`. Aliasing to `eu()` (id `"eu"`) would resolve the
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

    /// Spain. A4 + photo, like the other southern-European markets, but capped
    /// at **2 pages**, not Italy's 3 — Spanish CV guidance is consistently
    /// "máximo dos páginas". [`Self::max_pages`] is advisory (it decides when
    /// the trim panel OFFERS suggestions; nothing blocks a longer export), so
    /// inheriting Italy's 3 would not break anything — it would simply stop
    /// the app from ever suggesting a trim to a Spanish user whose CV has run
    /// a page past what the market expects.
    ///
    /// Its own constructor rather than an alias, for the reason Italy's is:
    /// `recommend::pick_locale` forwards this `id` verbatim and
    /// `locale::resume::section_order_for` matches the literal string, so
    /// aliasing to `eu()` would hand back the id `"eu"` and silently restore
    /// the US section order.
    pub fn es() -> LocaleProfile {
        LocaleProfile {
            id: "es",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Optional,
            max_pages: 2,
        }
    }

    /// Portugal. Same shape as [`Self::es`] — A4, photo optional, 2 pages.
    pub fn pt() -> LocaleProfile {
        LocaleProfile {
            id: "pt",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Optional,
            max_pages: 2,
        }
    }

    /// Brazil — a separate market id in the TS `COUNTRY_TO_MARKET` table
    /// (`BR: 'br'`, not `'pt'`), so it needs its own profile or it falls
    /// through to the US-shaped default the way Spain and Portugal did.
    ///
    /// **Reviewable call, and the one place it deliberately differs from
    /// [`Self::pt`]: `photo: Never`.** Brazilian hiring guidance has moved
    /// against photos on CVs on anti-discrimination grounds, unlike Portugal
    /// where they remain unremarkable. Same language, same section order,
    /// different norm — which is exactly why it is a distinct profile rather
    /// than an alias. If a Brazilian reviewer disagrees, this is a one-line
    /// change.
    pub fn br() -> LocaleProfile {
        LocaleProfile {
            id: "br",
            page_size: PageSize::A4,
            photo: PhotoPolicy::Never,
            max_pages: 2,
        }
    }
}

impl Default for LocaleProfile {
    fn default() -> Self {
        Self::en()
    }
}

#[cfg(test)]
mod tests;
