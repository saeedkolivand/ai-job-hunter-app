//! Per-market canonical résumé section order.
//!
//! Distinct from [`super::letter`]'s cover-letter conventions: this governs
//! the résumé's own section sequence — the ONE order source both the ATS
//! exporter (`model::transform::linearize`) and the draft prompt
//! (`pipeline::resume::prompts::draft_system`) read, so the model's section
//! order and the exporter's order can never disagree. Previously the prompt
//! left this to a free-form LLM choice (re-rolled every generation) while the
//! exporter used a market-blind order that only applied in ATS mode.
//!
//! Same market-string convention as [`super::letter::conventions`] (trim +
//! lowercase, unknown market falls back to the default), but the German arm
//! accepts BOTH live market-id namespaces — `"de"`/`"at"`/`"ch"` as the letter
//! conventions and the generation pipeline key them, and `"dach"` as
//! `LocaleProfile` collapses them, which is what actually arrives on the
//! AI-Generate résumé export path.
//!
//! Italy has no such second namespace to alias: `LocaleProfile` (see
//! `locale::mod.rs`) has no Italy profile at all, so a `target_country`- or
//! job-ad-language-derived `recommend::pick_locale` call for an Italian user
//! currently falls through to the "en"/intl default rather than "it" — a
//! separate, larger gap outside this file's scope. The market string this
//! file actually reads on the AI-Generate résumé path is the pipeline's own
//! `market` field (`req.market`, IPC-supplied), keyed the same way
//! `letter::conventions` keys it, and BOTH the TS `COUNTRY_TO_MARKET` (`IT`)
//! and `LANGUAGE_TO_MARKET` (`it`) tables already agree on the single
//! canonical id `"it"` — unlike Germany's three-country split, there is no
//! second spelling to accept here.

use crate::model::document::SectionId;

/// Reverse-chronological order (US/UK/default): Experience leads, Skills
/// follows it. Skills-above-Experience is real advice, but for career-changers
/// and entry-level candidates — on the continuous-history résumé this app
/// mostly serves it reads as compensating for thin experience, and it departs
/// from the reverse-chronological baseline `resume-export-standards` anchors
/// on. Section position does not affect ATS extraction either way (parsers
/// bucket by heading text), so this is a convention call, not a parsing one.
const DEFAULT_ORDER: &[SectionId] = &[
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

/// German Lebenslauf order: Berufserfahrung → Ausbildung → Zertifikate →
/// Kenntnisse — skills run late, not as an early keyword block.
const DE_ORDER: &[SectionId] = &[
    SectionId::Summary,
    SectionId::Experience,
    SectionId::Education,
    SectionId::Certifications,
    SectionId::Skills,
    SectionId::Languages,
    SectionId::Projects,
    SectionId::Awards,
    SectionId::Publications,
];

/// Italian CV order. Like the German Lebenslauf, `Istruzione e formazione`
/// (Education) and any `Certificazioni` sit right after `Esperienza
/// professionale` — Italian hiring reads a titled qualification as core
/// structure, not something to bury under Skills/Projects the way the US
/// default does.
///
/// **Reviewable call, not a copy of [`DE_ORDER`]:** Languages runs BEFORE
/// Skills here — the one position this order deliberately does NOT mirror
/// the German one. Italy's Europass CV, still the reference format most
/// Italian hirers recognise, nests "Lingue straniere" (foreign-language
/// competence) as the FIRST subsection of "Competenze personali", ahead of
/// any general or digital-skills subsection — the opposite of the German
/// convention, where a language table is usually the LAST thing in the
/// skills block. If a native reviewer disagrees, swapping these two back to
/// the German order is a one-line change, not a rethink of the rest of the
/// list.
///
/// Projects and the Awards/Publications tail stay last, same reasoning as
/// every market in this file: neither is a section a traditional Italian CV
/// has by default, so an application with real content for them still gets
/// a heading, just not one that crowds out Experience/Education/
/// Certifications/Languages/Skills for it.
const IT_ORDER: &[SectionId] = &[
    SectionId::Summary,
    SectionId::Experience,
    SectionId::Education,
    SectionId::Certifications,
    SectionId::Languages,
    SectionId::Skills,
    SectionId::Projects,
    SectionId::Awards,
    SectionId::Publications,
];

/// Canonical single-column section order for a market id. Sections not
/// listed (e.g. a [`SectionId::Custom`] one) keep their relative order and
/// follow the listed ones — see `model::transform::reorder_sections`.
pub fn section_order_for(market: &str) -> &'static [SectionId] {
    match market.trim().to_lowercase().as_str() {
        // Two market-id namespaces reach this function and they disagree:
        // `locale::letter::conventions` and the generation pipeline key on
        // "de", while `LocaleProfile` collapses DE/AT/CH into the single id
        // "dach" — which `recommend::pick_locale` returns and the AI-Generate
        // résumé export path forwards verbatim. Accept BOTH, using the same
        // alias set `LocaleProfile::get` already uses, so a German user cannot
        // silently receive the default order depending on which surface set
        // the market.
        "de" | "at" | "ch" | "dach" => DE_ORDER,
        // No alias to accept here — see the module doc comment: unlike
        // Germany, Italy has only one live market-id spelling ("it") on
        // every namespace that actually reaches this function.
        "it" => IT_ORDER,
        _ => DEFAULT_ORDER,
    }
}

#[cfg(test)]
mod tests {
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

    /// Mirrors `german_market_aliases_all_resolve_to_the_de_order`: Italy has
    /// only ONE live spelling to accept (see the module doc comment), but the
    /// case/whitespace handling still needs covering, and the market must
    /// resolve to something OTHER than the silent US default.
    #[test]
    fn italian_market_resolves_to_the_it_order() {
        for market in ["it", "IT", "  it  "] {
            assert_eq!(
                section_order_for(market),
                IT_ORDER,
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
}
