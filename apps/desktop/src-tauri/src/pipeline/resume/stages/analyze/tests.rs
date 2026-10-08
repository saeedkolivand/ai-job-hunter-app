use super::{apply_keyword_fallback, FALLBACK_MUST_HAVES};
use crate::pipeline::resume::types::JobAnalysis;

/// A real public Lever posting (Spotify, Android Engineer), whitespace-squeezed.
const AD: &str = include_str!("lever_android_ad.txt");

fn degraded() -> JobAnalysis {
    JobAnalysis {
        role_title: "Android Engineer".into(),
        responsibilities: vec!["Own app performance".into()],
        ..JobAnalysis::default()
    }
}

#[test]
fn a_degraded_analysis_gets_real_skills_not_boilerplate_from_a_real_ad() {
    let mut analysis = degraded();
    assert!(apply_keyword_fallback(&mut analysis, AD, "Spotify"));
    let got = &analysis.must_have;
    assert!(
        !got.is_empty() && got.len() <= FALLBACK_MUST_HAVES,
        "{got:?}"
    );
    for skill in ["kotlin", "sql"] {
        assert!(got.iter().any(|t| t == skill), "{skill} missing: {got:?}");
    }
    for junk in [
        "spotify",
        "https",
        "team",
        "teams",
        "experience",
        "android",
        "engineer",
    ] {
        assert!(!got.iter().any(|t| t == junk), "{junk} leaked: {got:?}");
    }
    assert!(!analysis.below_floor(AD));
}

#[test]
fn urls_are_dropped_and_skill_shaped_terms_rank_first() {
    let mut analysis = JobAnalysis::default();
    let ad = "See [our site](https://www.example.com/careers). Plenty of platform platform \
              platform work, plus SQL and Node.js.";
    assert!(apply_keyword_fallback(&mut analysis, ad, "Example"));
    assert!(!analysis
        .must_have
        .iter()
        .any(|t| t.contains("http") || t == "example"));
    assert_eq!(analysis.must_have[0], "sql");
}

#[test]
fn existing_must_haves_are_never_overwritten() {
    let mut analysis = JobAnalysis {
        must_have: vec!["Kotlin".into()],
        ..JobAnalysis::default()
    };
    assert!(!apply_keyword_fallback(&mut analysis, AD, "Spotify"));
    assert_eq!(analysis.must_have, ["Kotlin"]);
}

#[test]
fn a_garbled_ad_with_no_keywords_reports_no_fallback() {
    let mut analysis = JobAnalysis::default();
    assert!(!apply_keyword_fallback(
        &mut analysis,
        "!!! ??? ... --- 12 34",
        "Acme"
    ));
    assert!(analysis.must_have.is_empty());
}

/// A real public German Greenhouse posting (Doctolib), where every noun is
/// capitalised: capitalisation must not promote "Erfahrung" and friends.
#[test]
fn a_real_german_ad_ranks_skills_not_capitalised_nouns() {
    let ad = include_str!("greenhouse_de_ad.txt");
    let mut analysis = JobAnalysis::default();
    assert!(apply_keyword_fallback(&mut analysis, ad, "Doctolib"));
    let got = &analysis.must_have;
    let mut sorted = got.clone();
    sorted.sort();
    // Exactly the ad's skills: any capitalised German noun would add to this.
    assert_eq!(sorted, ["javascript", "python", "sql"], "{got:?}");
    for noun in ["erfahrung", "kenntnisse", "aufgaben", "team", "doctolib"] {
        assert!(!got.iter().any(|t| t == noun), "{noun} leaked: {got:?}");
    }
}
