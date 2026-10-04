//! The renderer's `SECTION_LEXICON`, swept through `classify_section`, and the headings whose
//! classification is pinned against a known misfile.

use super::*;

/// Every heading term the renderer's `SECTION_LEXICON` knows, swept
/// through this classifier.
///
/// The two sides answer the same question from different data — the TS
/// lexicon drives `detectSections`, this drives `extract_evidence` — and
/// nothing compared them until a sweep found German "Akademischer
/// Werdegang" classifying as **Experience**, because it contains
/// `werdegang`. That is the expensive direction: prose under an Experience
/// heading reaches `extract_evidence`'s role arm, so degree entries became
/// work bullets under roles the candidate never held.
///
/// What this pins, in order of what each costs:
///
/// 1. **No term may land in another section's bucket.** This catches the
///    class of bug above and has zero exceptions.
/// 2. **The known misses are enumerated, not tolerated in bulk.** A term
///    falling to `Other` is a coverage gap, not a misfile — the
///    section-specific checks simply do not run. Listing all 40 turns a
///    silent gap into a reviewed inventory: adding a lexicon term without
///    teaching this classifier fails here, and FIXING one fails here too,
///    so the list can only shrink deliberately.
///
/// The list is dominated by Summary and Skills synonyms in fr/es/it/nl/pt
/// (`objectif`, `conoscenze`, `samenvatting`). Closing them is additive
/// but not free: every entry in the heading consts is a SUBSTRING test
/// against real headings, and `expertise`, `portfolio` and `studi` have
/// second readings (Italian "studio" contains `studi`), so they need the
/// word-bounded treatment rather than a bare push. Left as a bounded,
/// visible task rather than a silent one.
const KNOWN_MISSES: &[&str] = &[
    "à propos",
    "abilità",
    "aptitudes",
    "competenties",
    "conhecimentos",
    "conocimientos",
    "conoscenze",
    "diplômes",
    "doelstelling",
    "educação",
    "educación",
    "éducation",
    "educazione",
    "ervaring",
    "escolaridade",
    // Back on the list after a review proved the bare stem cannot be
    // taught safely: only the WORK-QUALIFIED plurals ("esperienze
    // professionali/lavorative") are in `EXPERIENCE_HEADINGS`, so the bare
    // word is an honest miss rather than a misfile.
    "esperienze",
    "estudios",
    "études",
    "expertise",
    "kennis",
    "loopbaan",
    "obiettivo",
    "objectif",
    "objetivo",
    "onderwijs",
    "over mij",
    "portfolio",
    "qualifications",
    "résumé",
    "resumen",
    "resumo",
    "riassunto",
    "samenvatting",
    "savoir-faire",
    "sobre mí",
    "sobre mim",
    "sommario",
    "studi",
    "studie",
    "über mich",
];

/// `SectionKind` has no `Languages` variant, so a language heading has no
/// bucket of its own. Skills is the closest true answer and matches what
/// the résumé conventions already rely on — "Sprachkenntnisse" was
/// rejected as a German producer heading for exactly this reason.
/// Allowlisted explicitly rather than swept under the wrong-bucket rule,
/// so adding a `Languages` variant later has to come past this comment.
const LANGUAGES_CLASSIFY_AS_SKILLS: &[&str] = &["language skills", "sprachkenntnisse"];

fn expected(section: &str) -> Option<SectionKind> {
    match section {
        "Summary" => Some(SectionKind::Summary),
        "Experience" => Some(SectionKind::Experience),
        "Education" => Some(SectionKind::Education),
        "Skills" => Some(SectionKind::Skills),
        "Projects" => Some(SectionKind::Projects),
        _ => None,
    }
}

#[test]
fn every_renderer_lexicon_term_classifies_consistently() {
    #[derive(serde::Deserialize)]
    struct Case {
        section: String,
        term: String,
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/section-lexicon.json");
    let raw = std::fs::read_to_string(&path).expect(
        "read section-lexicon parity fixture              (packages/prompts/src/fixtures/section-lexicon.json)",
    );
    let cases: Vec<Case> =
        serde_json::from_str(&raw).expect("parse section-lexicon parity fixture");
    assert!(!cases.is_empty(), "the lexicon fixture must not be empty");

    let mut unexpected_misses = Vec::new();
    let mut fixed_misses = Vec::new();

    for c in &cases {
        let kind = classify_section(&c.term);
        let known_miss = KNOWN_MISSES.contains(&c.term.as_str());

        match expected(&c.section) {
            Some(want) if kind == want => {
                if known_miss {
                    fixed_misses.push(c.term.as_str());
                }
            }
            Some(_) if kind == SectionKind::Other => {
                if !known_miss {
                    unexpected_misses.push(c.term.as_str());
                }
            }
            Some(want) => panic!(
                "{:?} term {:?} classifies as {kind:?}, not {want:?} — it landed in                      ANOTHER section's bucket, which MISFILES its content rather than                      merely skipping it",
                c.section, c.term
            ),
            None => assert!(
                kind == SectionKind::Other
                    || LANGUAGES_CLASSIFY_AS_SKILLS.contains(&c.term.as_str()),
                "{:?} term {:?} has no bucket of its own but classified as {kind:?}",
                c.section,
                c.term
            ),
        }
    }

    assert!(
        unexpected_misses.is_empty(),
        "these lexicon terms classify as Other and are not in KNOWN_MISSES — either              teach the classifier or add them there with a reason: {unexpected_misses:?}"
    );
    // Third direction, and the one the first draft left open: an entry
    // that names no lexicon term at all. Without this, KNOWN_MISSES could
    // be padded with anything and still "shrink deliberately" — a review
    // proved it by adding `"zzz-not-a-lexicon-term"` and watching the
    // suite stay green.
    let lexicon: std::collections::HashSet<&str> = cases.iter().map(|c| c.term.as_str()).collect();
    let dead: Vec<&str> = KNOWN_MISSES
        .iter()
        .chain(LANGUAGES_CLASSIFY_AS_SKILLS.iter())
        .copied()
        .filter(|t| !lexicon.contains(t))
        .collect();
    assert!(
        dead.is_empty(),
        "these entries name no lexicon term, so they exempt nothing and only              make the list look longer than the real gap: {dead:?}"
    );

    assert!(
        fixed_misses.is_empty(),
        "these terms now classify correctly but are still listed in KNOWN_MISSES —              remove them, so the list can only shrink deliberately: {fixed_misses:?}"
    );
}

/// The precedence exception stated as behaviour rather than list
/// membership: each of these carries an EXPERIENCE substring and must
/// still reach Education.
#[test]
fn education_headings_carrying_an_experience_stem_reach_education() {
    // TWO lists on purpose, because either alone passes for the wrong
    // reason. The loop below drives off the const, so a phrase ADDED
    // without thought is still asserted — but that is self-referential:
    // deleting an entry deletes its own assertion, and a mutation run
    // proved exactly that (removing `wissenschaftlicher werdegang` left
    // this test green). So the membership check comes first and is
    // written out by hand; it is the half that catches a deletion.
    assert_eq!(
        sections::EDUCATION_OVERRIDES_EXPERIENCE,
        &[
            "akademischer werdegang",
            "akademischen werdegang",
            "wissenschaftlicher werdegang",
            "wissenschaftlichen werdegang",
            "schulischer werdegang",
            "bildungswerdegang",
            "akademische laufbahn",
            "ausbildungswerdegang",
        ],
        "the override list changed — every entry is load-bearing (each one              was found filing a degree as a job), so a removal must be a              deliberate edit here, not a silent side effect"
    );

    // Case-varied to prove the lowercasing in `classify_section` rather
    // than assuming it.
    for phrase in sections::EDUCATION_OVERRIDES_EXPERIENCE {
        for heading in [phrase.to_string(), phrase.to_uppercase()] {
            assert_eq!(
                classify_section(&heading),
                SectionKind::Education,
                "{heading:?} names an academic record; classifying it Experience                      files degree entries as work bullets under roles the candidate                      never held"
            );
        }
    }

    // Control: the same stem WITHOUT an education qualifier is still a work
    // history, so the exception must not have swallowed the rule.
    assert_eq!(
        classify_section("Beruflicher Werdegang"),
        SectionKind::Experience
    );
    assert_eq!(classify_section("Werdegang"), SectionKind::Experience);
}

/// The Italian half of the same invariant, and the regression a pre-PR
/// review caught in the first draft of this change: putting the BARE stem
/// `esperienze` in [`AMBIGUOUS_EXPERIENCE_HEADINGS`] sent "Esperienze di
/// formazione" — an education heading — to Experience, because that set
/// yields to summary and skills but never to education. A degree entry
/// under it became a fabricated job.
///
/// Only the WORK-QUALIFIED plurals are taught, so this pins both
/// directions at once: the education headings stay Education, the
/// qualified plurals reach Experience, and the bare word is an honest
/// miss rather than a misfile.
#[test]
fn italian_experience_plurals_do_not_capture_education_headings() {
    for heading in [
        "Esperienze di formazione",
        "Formazione ed esperienze",
        "Istruzione ed esperienze",
    ] {
        assert_eq!(
            classify_section(heading),
            SectionKind::Education,
            "{heading:?} is an education heading; a bare `esperienze` stem files                  its degrees as work bullets"
        );
    }

    for heading in [
        "Esperienze professionali",
        "Esperienze lavorative",
        // Work-qualified spellings must also stay OUT of the
        // section-deleting Skills hole `AMBIGUOUS_EXPERIENCE_HEADINGS`
        // documents — nothing in `extract_evidence` reads a Skills
        // section, so a work history landing there is erased, not
        // mislabelled.
        "Esperienze professionali e competenze",
    ] {
        assert_eq!(
            classify_section(heading),
            SectionKind::Experience,
            "{heading:?} is work-qualified and must reach Experience"
        );
    }

    // The bare stem is deliberately NOT taught: it cannot be, without
    // re-opening the education capture above. `Other` is the honest
    // answer and `KNOWN_MISSES` records it.
    assert_eq!(classify_section("Esperienze"), SectionKind::Other);
    // Substring control for the word-bounded `istruzione`.
    assert_eq!(classify_section("Distruzione"), SectionKind::Other);
    assert_eq!(classify_section("Istruzione"), SectionKind::Education);
}
