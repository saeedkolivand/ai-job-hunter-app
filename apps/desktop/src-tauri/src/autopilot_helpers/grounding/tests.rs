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
fn fluent_italian_claim_is_removed_and_the_gap_is_not_appended() {
    let note = "You are a strong CRM manager. You are fluent in Italian and English. \
                Tip: lead with your retention results.";
    let out = finish_note(note, A2_B1_RESUME);
    assert!(!out.contains("fluent in Italian"), "{out}");
    assert!(out.contains("strong CRM manager"));
    assert!(out.contains("Tip: lead with your retention results."));
    assert!(
        !out.contains("Unmet"),
        "gap text lives in the structured field: {out}"
    );
}

#[test]
fn restating_the_ad_requirement_is_not_a_candidate_claim() {
    let out = finish_note("The posting requires fluent Italian.", A2_B1_RESUME);
    assert!(out.contains("requires fluent Italian"));
}

#[test]
fn evidenced_claims_survive() {
    let out = finish_note("You speak English at C2 level.", A2_B1_RESUME);
    assert!(out.contains("English at C2"));
}

#[test]
fn preferred_requirement_is_never_a_gap() {
    let ad = "Nice to have: fluent Italian\nItalian C2 is a plus";
    let report = mandatory_report(A2_B1_RESUME, ad);
    assert!(
        report.gaps.is_empty() && report.to_verify.is_empty(),
        "{report:?}"
    );
    assert!(report.preferred_unmet);
    assert_eq!(report.category(A2_B1_RESUME), FitCategory::Stretch);
}

#[test]
fn missing_evidence_is_unknown_not_a_gap() {
    let resume = "Senior CRM manager, 8 years in retail.";
    let ad = "Fluent Italian required\nAt least 7+ years of CRM experience";
    let report = mandatory_report(resume, ad);
    assert!(report.gaps.is_empty(), "{report:?}");
    assert_eq!(report.to_verify.len(), 2, "{report:?}");
    assert_eq!(report.to_verify[0], "Italian C1");
    assert_eq!(report.category(resume), FitCategory::Unknown);
}

#[test]
fn unsupported_fluency_claim_with_no_evidence_is_dropped() {
    let out = finish_note("You are fluent in Italian. Good CRM depth.", "CRM manager");
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
fn unevaluable_mandatory_requirement_is_to_verify_not_a_supported_fit() {
    let ad = "Work authorization required for this role";
    let report = mandatory_report("CRM manager", ad);
    assert_eq!(report.to_verify, vec![ad.to_string()]);
    assert_eq!(report.category("CRM manager"), FitCategory::Unknown);
    let ok = mandatory_report("English C2", "English C1 required");
    assert!(ok.to_verify.is_empty() && ok.category("cv") == FitCategory::SupportedFit);
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
    let out = finish_note("You meet the requirement for fluent Italian.", A2_B1_RESUME);
    assert!(!out.contains("fluent Italian"), "{out}");
}

#[test]
fn finished_note_respects_the_cap() {
    let note = "Strong retention background across many roles. ".repeat(40);
    let out = finish_note(&note, A2_B1_RESUME);
    assert!(out.chars().count() <= NOTE_CAP, "{}", out.chars().count());
}

fn job_with(description: Option<&str>) -> FoundJob {
    FoundJob {
        description: description.map(str::to_string),
        ..crate::autopilot::tests::support::found_job_full(
            "https://acme.example/1",
            "CRM",
            "Acme",
            0,
        )
    }
}

#[test]
fn apply_note_fills_structured_fields_and_keeps_the_prose_clean() {
    let mut job = job_with(Some("Fluent Italian (C1) required."));
    let stored = apply_note(
        &mut job,
        "You are fluent in Italian. Lead with retention results.",
        A2_B1_RESUME,
    );
    assert!(stored);
    assert_eq!(
        job.assistant_notes.as_deref(),
        Some("Lead with retention results.")
    );
    assert_eq!(
        job.mandatory_gaps,
        vec!["Italian C1 required, résumé shows B1".to_string()]
    );
    assert_eq!(job.fit_category, Some(FitCategory::ExplicitGap));
    assert_eq!(
        job.notes_basis.as_deref(),
        Some(notes_basis(A2_B1_RESUME, job.description.as_deref()).as_str())
    );
}

#[test]
fn apply_note_leaves_the_job_alone_when_nothing_grounded_remains() {
    let mut job = job_with(Some("Fluent Italian required."));
    assert!(!apply_note(
        &mut job,
        "You are fluent in Italian.",
        A2_B1_RESUME
    ));
    assert!(job.assistant_notes.is_none() && job.fit_category.is_none());
}

#[test]
fn fit_category_rule_table() {
    let rep = |gaps: &[&str], verify: &[&str], pref: bool, reqs: bool| MandatoryReport {
        gaps: gaps.iter().map(|s| s.to_string()).collect(),
        to_verify: verify.iter().map(|s| s.to_string()).collect(),
        preferred_unmet: pref,
        has_requirements: reqs,
    };
    use FitCategory::*;
    let cases = [
        (rep(&["g"], &["v"], true, true), "cv", ExplicitGap),
        (rep(&["g"], &[], false, false), "", ExplicitGap),
        (rep(&[], &["v"], true, true), "cv", Unknown),
        (rep(&[], &[], false, true), "", Unknown),
        (rep(&[], &[], false, false), "cv", Unknown),
        (rep(&[], &[], true, true), "cv", Stretch),
        (rep(&[], &[], false, true), "cv", SupportedFit),
    ];
    for (report, resume, want) in cases {
        assert_eq!(report.category(resume), want, "{report:?} / {resume:?}");
    }
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
    assert_eq!(finish_note("", ""), "");
    assert_eq!(mandatory_report("", ""), MandatoryReport::default());
    assert_eq!(finish_note("Fine.", "\u{0}\u{fffd}§§"), "Fine.");
    assert_eq!(
        mandatory_report("cv", "%%%% ;;;").category("cv"),
        FitCategory::Unknown
    );
}
