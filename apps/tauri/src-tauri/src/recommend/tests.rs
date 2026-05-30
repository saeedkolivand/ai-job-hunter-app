use super::*;

fn signals(title: &str, seniority: &str, reqs: &[&str]) -> RecommendSignals {
    RecommendSignals {
        job_title: Some(title.to_string()),
        candidate_seniority: Some(seniority.to_string()),
        top_requirements: reqs.iter().map(|s| s.to_string()).collect(),
        resume_language: None,
        job_ad_language: None,
        target_country: None,
    }
}

#[test]
fn software_role_gets_modern() {
    let r = recommend(&signals("Frontend Engineer", "mid", &["React", "TypeScript"]));
    assert_eq!(r.template_id, TemplateId::Modern);
    assert!(!r.ats_suggested);
}

#[test]
fn systems_role_gets_mono_technical() {
    let r = recommend(&signals("Embedded Software Engineer", "mid", &["C++", "firmware"]));
    assert_eq!(r.template_id, TemplateId::MonoTechnical);
}

#[test]
fn conservative_field_gets_classic_and_suggests_ats() {
    let r = recommend(&signals("Compliance Auditor", "senior", &["finance", "SOX"]));
    assert_eq!(r.template_id, TemplateId::Classic);
    assert!(r.ats_suggested, "conservative fields should suggest ATS mode");
}

#[test]
fn academia_gets_academic() {
    let r = recommend(&signals("Postdoctoral Researcher", "mid", &["PhD", "publications"]));
    assert_eq!(r.template_id, TemplateId::Academic);
}

#[test]
fn design_role_gets_two_column() {
    let r = recommend(&signals("Product Designer", "mid", &["Figma", "UX"]));
    assert_eq!(r.template_id, TemplateId::TwoColumn);
}

#[test]
fn executive_overrides_to_refined_executive() {
    let r = recommend(&signals("VP of Engineering", "executive", &["leadership", "strategy"]));
    assert_eq!(r.template_id, TemplateId::RefinedExecutive);
}

#[test]
fn conservative_executive_stays_classic() {
    // A conservative field outranks seniority — a finance exec still reads best
    // as a clean single column.
    let r = recommend(&signals("Chief Financial Officer", "executive", &["finance", "audit"]));
    assert_eq!(r.template_id, TemplateId::Classic);
    assert!(r.ats_suggested);
}

#[test]
fn explicit_ats_mention_suggests_ats() {
    let r = recommend(&signals("Software Engineer", "mid", &["must pass ATS screening"]));
    assert!(r.ats_suggested);
}

#[test]
fn locale_follows_job_ad_language() {
    let mut s = signals("Softwareentwickler", "mid", &["Java"]);
    s.job_ad_language = Some("de".to_string());
    let r = recommend(&s);
    assert_eq!(r.locale, "dach");
    assert!(r.rationale.contains("dach"));
}

#[test]
fn locale_defaults_to_international_english() {
    let r = recommend(&signals("Software Engineer", "mid", &["Go"]));
    assert_eq!(r.locale, "en");
}

#[test]
fn target_country_wins_over_language() {
    // A US posting written in English → US (Letter), not the international default.
    let mut s = signals("Software Engineer", "mid", &["Go"]);
    s.job_ad_language = Some("en".to_string());
    s.target_country = Some("us".to_string());
    assert_eq!(recommend(&s).locale, "us");

    // `gb` resolves to the UK market through the locale registry.
    s.target_country = Some("gb".to_string());
    assert_eq!(recommend(&s).locale, "uk");
}

#[test]
fn region_subtag_resolves_us_vs_uk() {
    let mut s = signals("Software Engineer", "mid", &["Go"]);
    s.job_ad_language = Some("en-US".to_string());
    assert_eq!(recommend(&s).locale, "us");

    s.job_ad_language = Some("en-GB".to_string());
    assert_eq!(recommend(&s).locale, "uk");
}

#[test]
fn rationale_is_always_present() {
    let r = recommend(&RecommendSignals::default());
    assert!(!r.rationale.is_empty());
    assert_eq!(r.template_id, TemplateId::Modern); // empty → general default
}
