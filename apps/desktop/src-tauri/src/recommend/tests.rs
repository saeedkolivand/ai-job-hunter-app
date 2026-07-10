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

/// The ten live template IDs — the recommender must never return anything outside
/// this set.
const LIVE_TEMPLATES: [TemplateId; 10] = [
    TemplateId::Classic,
    TemplateId::SwissMinimal,
    TemplateId::Academic,
    TemplateId::Atelier,
    TemplateId::Meridian,
    TemplateId::Throughline,
    TemplateId::Portrait,
    TemplateId::Lebenslauf,
    TemplateId::Cadence,
    TemplateId::Regent,
];

#[test]
fn recommender_never_returns_a_deleted_id() {
    // Exhaustive sweep over representative job signals.  Every result must be a
    // member of the ten live templates — no deleted id (Modern et al.) may slip through.
    let cases: &[(&str, &str, &[&str])] = &[
        ("Frontend Engineer", "mid", &["React", "TypeScript"]),
        ("Embedded Software Engineer", "mid", &["C++", "firmware"]),
        ("Compliance Auditor", "senior", &["finance", "SOX"]),
        ("Postdoctoral Researcher", "mid", &["PhD", "publications"]),
        ("Product Designer", "mid", &["Figma", "UX"]),
        (
            "VP of Engineering",
            "executive",
            &["leadership", "strategy"],
        ),
        (
            "Chief Financial Officer",
            "executive",
            &["finance", "audit"],
        ),
        ("Software Engineer", "mid", &["Go", "Kubernetes"]),
        ("Art Director", "lead", &["brand", "motion design"]),
        ("Data Scientist", "mid", &["machine learning", "Python"]),
        // Blank / default signals
        ("", "", &[]),
    ];

    for (title, seniority, reqs) in cases {
        let r = recommend(&signals(title, seniority, reqs));
        assert!(
            LIVE_TEMPLATES.contains(&r.template_id),
            "recommend({title:?}, {seniority:?}) returned deleted/unknown id {:?}",
            r.template_id
        );
    }
}

#[test]
fn software_role_gets_classic() {
    // Modern was deleted; software roles now map to the Classic single column.
    let r = recommend(&signals(
        "Frontend Engineer",
        "mid",
        &["React", "TypeScript"],
    ));
    assert_eq!(r.template_id, TemplateId::Classic);
    assert!(!r.ats_suggested);
}

#[test]
fn systems_role_gets_classic() {
    // MonoTechnical then Modern were deleted; systems roles now map to Classic.
    let r = recommend(&signals(
        "Embedded Software Engineer",
        "mid",
        &["C++", "firmware"],
    ));
    assert_eq!(r.template_id, TemplateId::Classic);
}

#[test]
fn conservative_field_gets_classic_and_suggests_ats() {
    let r = recommend(&signals(
        "Compliance Auditor",
        "senior",
        &["finance", "SOX"],
    ));
    assert_eq!(r.template_id, TemplateId::Classic);
    assert!(
        r.ats_suggested,
        "conservative fields should suggest ATS mode"
    );
}

#[test]
fn academia_gets_academic() {
    let r = recommend(&signals(
        "Postdoctoral Researcher",
        "mid",
        &["PhD", "publications"],
    ));
    assert_eq!(r.template_id, TemplateId::Academic);
}

#[test]
fn design_role_gets_atelier() {
    // TwoColumn was deleted; design roles now map to Atelier.
    let r = recommend(&signals("Product Designer", "mid", &["Figma", "UX"]));
    assert_eq!(r.template_id, TemplateId::Atelier);
}

#[test]
fn executive_gets_meridian() {
    // RefinedExecutive was deleted; senior/exec roles now map to Meridian.
    let r = recommend(&signals(
        "VP of Engineering",
        "executive",
        &["leadership", "strategy"],
    ));
    assert_eq!(r.template_id, TemplateId::Meridian);
}

#[test]
fn conservative_executive_stays_classic() {
    // A conservative field outranks seniority — a finance exec still reads best
    // as a clean single column.
    let r = recommend(&signals(
        "Chief Financial Officer",
        "executive",
        &["finance", "audit"],
    ));
    assert_eq!(r.template_id, TemplateId::Classic);
    assert!(r.ats_suggested);
}

#[test]
fn explicit_ats_mention_suggests_ats() {
    let r = recommend(&signals(
        "Software Engineer",
        "mid",
        &["must pass ATS screening"],
    ));
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
    assert_eq!(r.template_id, TemplateId::Classic); // empty → general default
}

// --- M1: score-all-fields-and-take-the-max classification ---

/// The motivating case: a title with one finance keyword but two software
/// keywords must classify as Software, not Conservative. Both currently resolve
/// to Classic, so the distinguishing signal is `ats_suggested`: Conservative
/// would auto-suggest ATS mode, Software must not. The old first-match-wins
/// ordering tested Conservative before Software and would have mis-suggested ATS.
#[test]
fn finance_software_role_classifies_as_software_not_conservative() {
    let r = recommend(&signals(
        "Financial Software Engineer",
        "mid",
        &["backend", "APIs"],
    ));
    assert_eq!(
        r.template_id,
        TemplateId::Classic,
        "software / conservative both map to Classic"
    );
    assert!(
        !r.ats_suggested,
        "must not be classified Conservative, so no automatic ATS suggestion"
    );
}

/// Word-boundary matching: "tax" must not match inside "syntax", which would have
/// mis-classified a pure software role as Conservative under the old substring
/// `h.contains("tax ")` / `h.contains(k)` logic.
#[test]
fn substring_does_not_trigger_false_conservative() {
    let r = recommend(&signals(
        "Backend Engineer",
        "mid",
        &["syntax trees", "compilers"],
    ));
    assert_eq!(r.template_id, TemplateId::Classic);
    // The real discriminator: a false Conservative match would auto-suggest ATS.
    assert!(
        !r.ats_suggested,
        "'syntax'/'tax' must not be read as a Conservative (finance/tax) signal"
    );
}

/// "ats" is matched on a word boundary, including when it is the very first
/// token (the old code needed a special `starts_with("ats")` branch for that).
#[test]
fn ats_keyword_matched_on_word_boundary() {
    let mut at_start = signals("ATS compliant resume screening", "mid", &[]);
    at_start.top_requirements = vec![];
    assert!(
        recommend(&at_start).ats_suggested,
        "leading 'ATS' token must trigger ATS suggestion"
    );

    // A word merely containing the letters "ats" must NOT trigger it.
    let r = recommend(&signals("Stats Analyst", "mid", &["statistics"]));
    assert!(
        !r.ats_suggested,
        "'stats'/'statistics' must not be read as an ATS mention"
    );
}

/// A clear single-field role still classifies correctly under the max-score
/// rule (regression guard that scoring all fields didn't change the easy cases).
#[test]
fn pure_design_role_still_design() {
    let r = recommend(&signals("UX/UI Designer", "mid", &["Figma"]));
    assert_eq!(r.template_id, TemplateId::Atelier);
}
