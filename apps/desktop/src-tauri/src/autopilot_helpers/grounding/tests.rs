//! Requirement tiers (EN/DE/IT) and the language-level grounding of fit notes.

use super::*;

#[test]
fn tiers_english() {
    assert_eq!(
        classify_tier("Must have 5+ years of CRM experience"),
        Tier::Mandatory
    );
    assert_eq!(classify_tier("7+ years in B2B sales"), Tier::Mandatory);
    assert_eq!(classify_tier("Fluent Italian"), Tier::Mandatory);
    assert_eq!(classify_tier("C1 German is mandatory"), Tier::Mandatory);
    assert_eq!(
        classify_tier("Experience with Salesforce is a plus"),
        Tier::Preferred
    );
    assert_eq!(classify_tier("Nice to have: Google Ads"), Tier::Preferred);
    assert_eq!(classify_tier("A degree is not required"), Tier::Preferred);
    assert_eq!(
        classify_tier("You enjoy working in a team"),
        Tier::Unspecified
    );
}

#[test]
fn tiers_german() {
    assert_eq!(
        classify_tier("Fließend Deutsch in Wort und Schrift ist zwingend"),
        Tier::Mandatory
    );
    assert_eq!(
        classify_tier("Erforderlich: 3 Jahre Berufserfahrung"),
        Tier::Mandatory
    );
    assert_eq!(
        classify_tier("Voraussetzung ist ein abgeschlossenes Studium"),
        Tier::Mandatory
    );
    assert_eq!(
        classify_tier("Erfahrung mit SAP ist wünschenswert"),
        Tier::Preferred
    );
    assert_eq!(
        classify_tier("Englischkenntnisse sind von Vorteil"),
        Tier::Preferred
    );
    assert_eq!(
        classify_tier("Wir bieten flexible Arbeitszeiten"),
        Tier::Unspecified
    );
}

#[test]
fn tiers_italian() {
    assert_eq!(classify_tier("Inglese fluente"), Tier::Mandatory);
    assert_eq!(classify_tier("Italiano madrelingua"), Tier::Mandatory);
    assert_eq!(
        classify_tier("Almeno 3 anni di esperienza commerciale"),
        Tier::Mandatory
    );
    assert_eq!(
        classify_tier("Requisito indispensabile: laurea magistrale"),
        Tier::Mandatory
    );
    assert_eq!(
        classify_tier("Costituisce titolo preferenziale la conoscenza di SAP"),
        Tier::Preferred
    );
    assert_eq!(
        classify_tier("Offriamo un ambiente dinamico"),
        Tier::Unspecified
    );
}

#[test]
fn preferred_heading_demotes_the_lines_below_it() {
    let ad = "Requirements:\nFluent Italian\nNice to have:\nFluent German\nMeta Ads experience";
    let reqs = classify_requirements(ad);
    let tier_of = |t: &str| reqs.iter().find(|r| r.text == t).map(|r| r.tier);
    assert_eq!(tier_of("Fluent Italian"), Some(Tier::Mandatory));
    assert_eq!(tier_of("Fluent German"), Some(Tier::Preferred));
    assert_eq!(tier_of("Meta Ads experience"), Some(Tier::Preferred));
}

#[test]
fn language_levels_parse_cefr_and_fluency_words() {
    assert_eq!(
        evidenced_levels("Languages: English (native), Italian (A2-B1)")["Italian"],
        3
    );
    assert_eq!(evidenced_levels("Italian B1 and English C2")["English"], 6);
    assert_eq!(evidenced_levels("Deutsch: C1")["German"], 5);
    assert!(evidenced_levels("Rust, Kubernetes, 8 years of backend work").is_empty());
}

const A2_B1_RESUME: &str =
    "Maria Rossi\nSenior CRM manager.\nLanguages: English (C2), Italian (A2-B1)";

#[test]
fn a2_b1_italian_vs_fluent_requirement_is_a_mandatory_gap() {
    let report = mandatory_report(
        A2_B1_RESUME,
        "Requirements:\nFluent Italian (C1)\nNice to have: Meta Ads",
    );
    assert_eq!(
        report.gaps,
        vec!["Italian C1 required, résumé shows B1".to_string()]
    );
    assert!(report.to_verify.is_empty());
}

#[test]
fn fluent_italian_claim_is_removed_and_the_gap_appended() {
    let note = "You are a strong CRM manager. You are fluent in Italian and English. \
                Tip: lead with your retention results.";
    let out = ground_note(note, A2_B1_RESUME, "Fluent Italian required.");
    assert!(!out.contains("fluent in Italian"), "{out}");
    assert!(out.contains("strong CRM manager"));
    assert!(out.contains("Tip: lead with your retention results."));
    assert!(
        out.contains("Unmet must-have: Italian C1 required, résumé shows B1."),
        "{out}"
    );
}

#[test]
fn restating_the_ad_requirement_is_not_a_candidate_claim() {
    let out = ground_note("The posting requires fluent Italian.", A2_B1_RESUME, "");
    assert!(out.contains("requires fluent Italian"));
}

#[test]
fn evidenced_claims_survive() {
    let out = ground_note("You speak English at C2 level.", A2_B1_RESUME, "");
    assert!(out.contains("English at C2"));
}

#[test]
fn preferred_requirement_is_never_a_gap() {
    let ad = "Nice to have: fluent Italian\nItalian C2 is a plus";
    let report = mandatory_report(A2_B1_RESUME, ad);
    assert_eq!(report, MandatoryReport::default());
    assert_eq!(ground_note("Good fit.", A2_B1_RESUME, ad), "Good fit.");
}

#[test]
fn missing_evidence_is_unknown_not_a_gap() {
    let resume = "Senior CRM manager, 8 years in retail.";
    let ad = "Fluent Italian required\nAt least 7+ years of CRM experience";
    let report = mandatory_report(resume, ad);
    assert!(report.gaps.is_empty(), "{report:?}");
    assert_eq!(report.to_verify.len(), 2, "{report:?}");
    let out = ground_note("Solid CRM background.", resume, ad);
    assert!(out.contains("To verify (must-have): Italian C1; "), "{out}");
    assert!(!out.contains("Unmet"));
}

#[test]
fn unsupported_fluency_claim_with_no_evidence_is_dropped() {
    let out = ground_note(
        "You are fluent in Italian. Good CRM depth.",
        "CRM manager",
        "",
    );
    assert_eq!(out, "Good CRM depth.");
}

#[test]
fn italian_ad_native_requirement_vs_b1_candidate() {
    let report = mandatory_report(A2_B1_RESUME, "Italiano madrelingua\nInglese fluente");
    assert_eq!(
        report.gaps,
        vec!["Italian native required, résumé shows B1".to_string()]
    );
}

#[test]
fn language_demand_past_the_display_cap_is_still_analysed() {
    let ad = format!("Role details {} Fluent Italian required", "x ".repeat(100));
    let report = mandatory_report(A2_B1_RESUME, &ad);
    assert_eq!(
        report.gaps,
        vec!["Italian C1 required, résumé shows B1".to_string()]
    );
}

#[test]
fn years_check_runs_on_a_line_that_also_demands_a_language() {
    let ad = "Fluent English and at least 7+ years of CRM experience required";
    let report = mandatory_report("English C2", ad);
    assert!(report.gaps.is_empty(), "{report:?}");
    assert_eq!(report.to_verify, vec![ad.to_string()]);
}

#[test]
fn explicit_mandatory_level_below_c1_is_compared() {
    let report = mandatory_report("German A1", "German B2 required");
    assert_eq!(
        report.gaps,
        vec!["German B2 required, résumé shows A1".to_string()]
    );
}

#[test]
fn candidate_claim_containing_require_is_not_exempt() {
    let out = ground_note(
        "You meet the requirement for fluent Italian.",
        A2_B1_RESUME,
        "",
    );
    assert!(!out.contains("fluent Italian"), "{out}");
}

#[test]
fn finished_note_respects_the_cap_and_keeps_the_warnings() {
    let note = "Strong retention background across many roles. ".repeat(40);
    let out = finish_note(&note, A2_B1_RESUME, Some("Fluent Italian (C1) required."));
    assert!(out.chars().count() <= NOTE_CAP, "{}", out.chars().count());
    assert!(
        out.ends_with("Unmet must-have: Italian C1 required, résumé shows B1."),
        "{out}"
    );
}

#[test]
fn negated_fluency_is_not_evidence() {
    assert_eq!(evidenced_levels("Italian: B1 (not fluent)")["Italian"], 3);
}

#[test]
fn leading_level_is_shared_across_coordinated_languages() {
    let e = evidenced_levels("fluent in English and Italian");
    assert_eq!((e["English"], e["Italian"]), (5, 5));
    let e = evidenced_levels("Italian B1 and English C2");
    assert_eq!((e["Italian"], e["English"]), (3, 6));
}

#[test]
fn levels_do_not_bleed_across_sentences() {
    let e = evidenced_levels("Italian B1. English C2.");
    assert_eq!((e["Italian"], e["English"]), (3, 6));
}

#[test]
fn empty_or_garbled_inputs_are_harmless() {
    assert_eq!(ground_note("", "", ""), "");
    assert_eq!(mandatory_report("", ""), MandatoryReport::default());
    assert_eq!(ground_note("Fine.", "\u{0}\u{fffd}§§", "%%%% ;;;"), "Fine.");
}
