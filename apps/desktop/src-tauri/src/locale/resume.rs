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
//! lowercase, unknown market falls back to the default) — deliberately NOT
//! `LocaleProfile::get`'s region-tag parsing, since only `de` currently
//! diverges from the default.

use crate::model::document::SectionId;

/// Skills-driven order (US/UK/default): a dedicated Skills block sits above
/// Experience as a concentrated keyword zone early in the page — 2026 ATS
/// guidance for skills-driven roles.
const DEFAULT_ORDER: &[SectionId] = &[
    SectionId::Summary,
    SectionId::Skills,
    SectionId::Experience,
    SectionId::Projects,
    SectionId::Education,
    SectionId::Certifications,
    SectionId::Languages,
    SectionId::Awards,
    SectionId::Publications,
];

/// German Lebenslauf order: Berufserfahrung → Ausbildung → Weiterbildung →
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
        _ => DEFAULT_ORDER,
    }
}

/// Render `section_order_for(market)` as a comma-separated list of canonical
/// English section names, for injecting into the draft prompt as a FIXED
/// instruction (the model is told the order rather than inventing one). Not a
/// localized heading — see `prompt_blocks::resume_conventions` for the small
/// subset of headings (summary/skills/experience/education) that ARE
/// localized for the "Structure:" line.
pub fn section_order_prompt_list(market: &str) -> String {
    section_order_for(market)
        .iter()
        .map(|id| format!("{id:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(order: &[SectionId], id: &SectionId) -> usize {
        order.iter().position(|s| s == id).expect("section present")
    }

    #[test]
    fn default_order_puts_skills_before_experience() {
        let order = section_order_for("us");
        assert!(
            position(order, &SectionId::Skills) < position(order, &SectionId::Experience),
            "skills-driven markets put a dedicated Skills block before Experience"
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
    fn unknown_market_falls_back_to_default() {
        assert_eq!(section_order_for("zz"), DEFAULT_ORDER);
        assert_eq!(section_order_for(""), DEFAULT_ORDER);
        assert_eq!(section_order_for("  De  "), DE_ORDER);
    }

    #[test]
    fn prompt_list_renders_canonical_names_in_order() {
        assert_eq!(
            section_order_prompt_list("us"),
            "Summary, Skills, Experience, Projects, Education, Certifications, \
             Languages, Awards, Publications"
        );
        assert_eq!(
            section_order_prompt_list("de"),
            "Summary, Experience, Education, Certifications, Skills, Languages, \
             Projects, Awards, Publications"
        );
    }
}
