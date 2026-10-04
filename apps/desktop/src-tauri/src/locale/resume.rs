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
//! Italy has no such second namespace to alias. The market string this
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

/// Southern-European / Europass CV order — Italy, Spain, Portugal, Brazil.
/// Like the German Lebenslauf, Education and any Certifications sit right
/// after Experience: these markets read a titled qualification as core
/// structure, not something to bury under Skills/Projects the way the US
/// default does.
///
/// **Why these four and not every non-US market:** they are exactly the
/// markets whose section HEADINGS are already curated in `resume_conventions`
/// (`de en es fr it nl pt`, minus the ones with their own order). A market
/// with no curated headings gets English headings, and giving it a
/// non-English section ORDER would be half a localization — worse than none,
/// because the document then matches no market's expectations. So the order
/// axis deliberately tracks the heading axis.
///
/// **Reviewable call, not a copy of [`DE_ORDER`]:** Languages runs BEFORE
/// Skills here — the one position this order deliberately does NOT mirror
/// the German one. The Europass CV, still the reference format these markets
/// recognise, nests foreign-language competence ("Lingue straniere",
/// "Idiomas", "Línguas") as the FIRST subsection of personal competences,
/// ahead of any general or digital-skills subsection — the opposite of the
/// German convention, where a language table is usually the LAST thing in
/// the skills block. If a native reviewer disagrees, swapping these two back
/// to the German order is a one-line change, not a rethink of the rest of
/// the list.
///
/// Projects and the Awards/Publications tail stay last, same reasoning as
/// every market in this file: neither is a section a traditional Italian CV
/// has by default, so an application with real content for them still gets
/// a heading, just not one that crowds out Experience/Education/
/// Certifications/Languages/Skills for it.
const EUROPASS_ORDER: &[SectionId] = &[
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
        // Spain, Portugal and Brazil share Italy's shape here; they differ
        // from each other on page count and photo policy, which is
        // `LocaleProfile`'s axis, not this one.
        "it" | "es" | "pt" | "br" => EUROPASS_ORDER,
        _ => DEFAULT_ORDER,
    }
}

#[cfg(test)]
mod tests;
