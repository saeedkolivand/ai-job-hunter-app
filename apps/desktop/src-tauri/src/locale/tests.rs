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
    let ids: std::collections::HashSet<&str> = all.iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), all.len(), "two profiles share an id: {ids:?}");

    // The property, rather than a restated list: every profile `all()`
    // advertises must resolve back to ITSELF through `get`. That is the
    // contract the rest of the app depends on — `recommend::pick_locale`
    // forwards a profile's `id` verbatim and downstream code calls `get`
    // on it — and it is exactly what a missing `get` arm breaks, silently
    // and with every hardcoded-list test still green. Spain, Portugal and
    // Brazil each shipped that way until this test stopped naming names.
    for profile in &all {
        assert_eq!(
            LocaleProfile::get(profile.id).id,
            profile.id,
            "market {:?} is offered by `all()` but does not round-trip through \
             `get` — it falls back to the international default, taking the \
             US section order with it",
            profile.id
        );
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
/// order — so an Italian user's already-committed `EUROPASS_ORDER` was
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
