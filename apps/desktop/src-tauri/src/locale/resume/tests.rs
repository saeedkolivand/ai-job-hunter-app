use super::*;

fn position(order: &[SectionId], id: &SectionId) -> usize {
    order.iter().position(|s| s == id).expect("section present")
}

#[test]
fn default_order_is_reverse_chronological_experience_first() {
    let order = section_order_for("us");
    assert!(
        position(order, &SectionId::Experience) < position(order, &SectionId::Skills),
        "the default market leads with Experience (reverse-chronological)"
    );
    assert!(
        position(order, &SectionId::Skills) < position(order, &SectionId::Education),
        "Skills still precedes Education on the default order"
    );
}

#[test]
fn de_order_puts_skills_after_certifications() {
    // Case-insensitive, like `letter::conventions`.
    let order = section_order_for("DE");
    assert!(
        position(order, &SectionId::Certifications) < position(order, &SectionId::Skills),
        "the German Lebenslauf runs skills late, after certifications"
    );
}

/// Regression: `LocaleProfile` collapses DE/AT/CH into the id "dach", and
/// `recommend::pick_locale` returns that value, which the AI-Generate
/// résumé export path forwards to `linearize` verbatim. Matching only the
/// literal "de" silently gave those users the default (US) order.
#[test]
fn german_market_aliases_all_resolve_to_the_de_order() {
    for market in ["de", "at", "ch", "dach", "DACH", "  dach  "] {
        assert_eq!(
            section_order_for(market),
            DE_ORDER,
            "market {market:?} must resolve to the Lebenslauf order"
        );
    }
}

#[test]
fn it_order_runs_education_and_certifications_right_after_experience() {
    let order = section_order_for("it");
    // "right AFTER Experience" is half this test's name and was the half
    // it did not assert: every other assertion here is `… < Skills`, so
    // an order burying Experience below Education — the exact opposite of
    // what `EUROPASS_ORDER`'s own doc claims — stayed green.
    assert!(
        position(order, &SectionId::Experience) < position(order, &SectionId::Education),
        "Experience must lead the Italian order; Education follows it, \
         the same shape as the German Lebenslauf"
    );
    assert!(
        position(order, &SectionId::Education) < position(order, &SectionId::Skills),
        "Education must not be buried under Skills on the Italian order"
    );
    assert!(
        position(order, &SectionId::Certifications) < position(order, &SectionId::Skills),
        "Certifications must not be buried under Skills on the Italian order"
    );
    // The reviewable call this order makes on purpose: Languages before
    // Skills, the one spot it does NOT mirror `DE_ORDER`.
    assert!(
        position(order, &SectionId::Languages) < position(order, &SectionId::Skills),
        "Italian Languages must precede Skills — Europass nests language \
         competence as the FIRST skills subsection, opposite of the German order"
    );
}

/// Spain, Portugal and Brazil share Italy's Europass shape. They shipped
/// resolving to the US-shaped default instead: the TS `COUNTRY_TO_MARKET`
/// and `LANGUAGE_TO_MARKET` tables have emitted `es`/`pt`/`br` since
/// before Italy was fixed, and nothing on the Rust side had an arm for
/// them, so a Spanish CV got Spanish HEADINGS in American ORDER.
///
/// Asserted against `DEFAULT_ORDER` as well as the positive case, because
/// the positive assertion alone would pass if every market were quietly
/// pointed at one shared list.
///
/// Mutation check: removed `"es" | "pt" | "br"` from the arm — RAN, went
/// red on all three, restored.
#[test]
fn iberian_and_lusophone_markets_resolve_to_the_europass_order() {
    for market in ["es", "ES", "  pt  ", "br"] {
        assert_eq!(
            section_order_for(market),
            EUROPASS_ORDER,
            "market {market:?} must resolve to the Europass order"
        );
        assert_ne!(
            section_order_for(market),
            DEFAULT_ORDER,
            "market {market:?} must not silently fall back to the US-shaped default"
        );
    }
}

/// Mirrors `german_market_aliases_all_resolve_to_the_de_order`: Italy has
/// only ONE live spelling to accept (see the module doc comment), but the
/// case/whitespace handling still needs covering, and the market must
/// resolve to something OTHER than the silent US default.
#[test]
fn italian_market_resolves_to_the_it_order() {
    for market in ["it", "IT", "  it  "] {
        assert_eq!(
            section_order_for(market),
            EUROPASS_ORDER,
            "market {market:?} must resolve to the Italian order"
        );
    }
    assert_ne!(
        section_order_for("it"),
        DEFAULT_ORDER,
        "an Italian market must not silently fall back to the US-shaped default"
    );
}

#[test]
fn unknown_market_falls_back_to_default() {
    assert_eq!(section_order_for("zz"), DEFAULT_ORDER);
    assert_eq!(section_order_for(""), DEFAULT_ORDER);
    assert_eq!(section_order_for("  De  "), DE_ORDER);
}

/// The producer side's chosen vocabulary must not collide with the
/// recognizer's own heading buckets (`documents::evidence::classify_section`)
/// — a collision would file a re-parsed/regenerated section under the
/// WRONG bucket. Verified by actually calling the classifier, not by
/// eyeballing its substring lists.
///
/// The two REJECTED German alternatives are asserted here too, as the
/// negative case that justifies picking the words used above them:
/// "Sprachkenntnisse" (contains "kenntnis") reads as Skills, and
/// "Weiterbildung" (contains "bildung") reads as Education, colliding
/// with "Ausbildung".
#[test]
fn localized_headers_do_not_collide_with_the_recognizers_buckets() {
    use crate::documents::evidence::{classify_section, SectionKind};

    // German — the traps this branch's fix specifically avoids.
    assert_eq!(classify_section("Zertifikate"), SectionKind::Other);
    assert_eq!(classify_section("Sprachen"), SectionKind::Other);
    assert_eq!(classify_section("Sprachkenntnisse"), SectionKind::Skills);
    assert_eq!(classify_section("Weiterbildung"), SectionKind::Education);

    // Italian.
    assert_eq!(classify_section("Progetti"), SectionKind::Projects);
    assert_eq!(classify_section("Certificazioni"), SectionKind::Other);
    assert_eq!(classify_section("Lingue"), SectionKind::Other);
    assert_eq!(classify_section("Riconoscimenti"), SectionKind::Other);
    assert_eq!(classify_section("Pubblicazioni"), SectionKind::Other);
}
