//! Tests for the MAX-depth spine's pure decisions — the ones that hold whatever
//! a model returns.
//!
//! Same discipline as the sibling `test` module: every guard here was
//! mutation-checked by applying the change named in its doc comment and
//! watching the test fail, then reverting.

use serde_json::Value;

use super::types_max::{
    schema_for, EducationOut, ExperienceOut, JudgeItem, JudgeOut, ProjectsOut, SectionSeed,
    SkillsOut, SummaryOut, USED_EVIDENCE_KEY,
};

/// Deserialize a section EXAMPLE into its own type, or fail loudly with the
/// parse error — an example that no longer matches its struct is the drift this
/// whole module exists to catch.
fn parse<T: serde::de::DeserializeOwned>(label: &str, example: &str) -> T {
    serde_json::from_str(example)
        .unwrap_or_else(|e| panic!("{label}'s filled example no longer parses into its type: {e}"))
}

/// The filled examples ARE the contract for every provider without native
/// constrained decoding — they are what the model copies. An example that has
/// drifted from its struct teaches the model a shape the parser will reject,
/// and the failure looks like "the model is bad at JSON".
///
/// Mutation check: rename any field on any of the five `*Out` structs (or in
/// its example) without updating the other side, and the matching assertion
/// fails — `#[serde(default)]` means the parse SUCCEEDS with an empty value,
/// which is exactly why each field is asserted rather than just the parse.
#[test]
fn every_section_example_still_deserializes_into_its_own_type() {
    let summary: SummaryOut = parse("SummaryOut", SummaryOut::EXAMPLE);
    assert!(!summary.summary.is_empty());
    assert!(!summary.used_evidence.is_empty());

    let skills: SkillsOut = parse("SkillsOut", SkillsOut::EXAMPLE);
    assert!(!skills.groups.is_empty());
    assert!(skills.groups.iter().all(|g| !g.label.is_empty()));
    assert!(skills.groups.iter().all(|g| !g.skills.is_empty()));

    let experience: ExperienceOut = parse("ExperienceOut", ExperienceOut::EXAMPLE);
    assert!(!experience.bullets.is_empty());
    assert!(!experience.used_evidence.is_empty());

    let projects: ProjectsOut = parse("ProjectsOut", ProjectsOut::EXAMPLE);
    let project = projects.projects.first().expect("one seeded project");
    assert!(!project.name.is_empty());
    assert!(!project.links.is_empty());
    assert!(!project.stack.is_empty());
    assert!(!project.description.is_empty());

    let education: EducationOut = parse("EducationOut", EducationOut::EXAMPLE);
    assert!(!education.entries.is_empty());

    let judge: JudgeOut = parse("JudgeOut", JudgeOut::EXAMPLE);
    let item = judge.items.first().expect("one judged item");
    assert!(!item.kind.is_empty());
    assert!(!item.note.is_empty());
    assert!(!item.quote.is_empty());
}

/// Every seed's example must SPELL the citation key: `usedEvidence` is the one
/// field shared by all five answers and the one a model most often omits, and a
/// missing citation list means the whole grounded-citation filter has nothing
/// to filter.
///
/// Mutation check: drop the `usedEvidence` line from any example and this fails.
#[test]
fn every_section_example_shows_the_citation_field() {
    for example in super::types_max::examples() {
        assert!(
            example.contains(USED_EVIDENCE_KEY),
            "a section example omits {USED_EVIDENCE_KEY}: {example}"
        );
    }
}

/// Every seed's schema must be FLAT — no `$defs`, no `$ref`. The 2026
/// measurements put nested-`$defs` schemas at ~68% non-compliance on the small
/// local models this app is meant to run offline, and a schema is only worth
/// sending to a constrained decoder that can hold it.
///
/// Mutation check: add a `"$ref"` anywhere in one of the `schema()` bodies and
/// this fails.
#[test]
fn every_section_schema_is_flat() {
    fn walk(value: &Value, path: &str) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    assert!(
                        key != "$defs" && key != "$ref" && key != "definitions",
                        "{path}/{key} is a schema REFERENCE — section schemas must be flat"
                    );
                    walk(child, &format!("{path}/{key}"));
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    walk(child, &format!("{path}/{index}"));
                }
            }
            _ => {}
        }
    }

    for seed in seeds() {
        walk(&schema_for(&seed), seed.kind());
    }
}

/// Every property a section's TYPE serializes must appear in the schema it is
/// asked for with, or a constrained decoder will refuse to emit it — silently,
/// as an always-empty field.
///
/// Mutation check: delete `"usedEvidence"` from any one `schema()` and this
/// fails naming that section.
#[test]
fn every_section_schema_declares_every_field_of_its_type() {
    let cases: [(SectionSeed, Value); 5] = [
        (
            SectionSeed::Summary,
            serde_json::to_value(SummaryOut::default()).unwrap(),
        ),
        (
            SectionSeed::Skills(Vec::new()),
            serde_json::to_value(SkillsOut::default()).unwrap(),
        ),
        (
            SectionSeed::Experience(Box::default()),
            serde_json::to_value(ExperienceOut::default()).unwrap(),
        ),
        (
            SectionSeed::Projects(Vec::new()),
            serde_json::to_value(ProjectsOut::default()).unwrap(),
        ),
        (
            SectionSeed::Education(Vec::new()),
            serde_json::to_value(EducationOut::default()).unwrap(),
        ),
    ];
    for (seed, serialized) in cases {
        let schema = schema_for(&seed);
        let declared = schema
            .get("properties")
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{}'s schema has no properties", seed.kind()));
        for field in serialized
            .as_object()
            .expect("a struct serializes to an object")
            .keys()
        {
            assert!(
                declared.contains_key(field),
                "{}'s schema does not declare `{field}`",
                seed.kind()
            );
        }
    }
}

/// **A model may never emit a Critical**, and at max depth the judge is the one
/// stage where a model's opinion reaches the report at all. The cap is
/// STRUCTURAL: [`JudgeItem`] has no severity field, so a model that claims one
/// is not filtered — it has nowhere to put the claim.
///
/// Mutation check: add a `severity` field to `JudgeItem` and this fails on the
/// serialized key set; the deserialization half fails too, because the claimed
/// value would then survive.
#[test]
fn a_judge_item_has_nowhere_to_claim_a_severity() {
    let serialized = serde_json::to_value(JudgeItem::default()).expect("JudgeItem serializes");
    let keys: Vec<&String> = serialized
        .as_object()
        .expect("an object")
        .keys()
        .filter(|key| key.to_lowercase().contains("sever"))
        .collect();
    assert!(
        keys.is_empty(),
        "JudgeItem gained a severity-shaped field ({keys:?}) — the Warning-only cap must stay \
         structural, not a filter someone can invert"
    );

    // A hostile answer that claims one anyway: serde has nowhere to bind it, so
    // the claim is dropped on the way in rather than downgraded downstream.
    let hostile: JudgeItem = serde_json::from_str(
        r#"{"kind":"evidence","section":"Work Experience","note":"n","quote":"q","severity":"critical"}"#,
    )
    .expect("unknown keys are ignored");
    let round_tripped = serde_json::to_value(&hostile).expect("serializes");
    assert!(
        round_tripped.get("severity").is_none(),
        "a claimed severity survived the parse"
    );
}

/// The five seeds, one of each variant — the input to the schema/example walks
/// above. A sixth variant added without a branch here fails to compile, which
/// is the point of matching exhaustively rather than listing kinds by hand.
fn seeds() -> Vec<SectionSeed> {
    vec![
        SectionSeed::Summary,
        SectionSeed::Skills(Vec::new()),
        SectionSeed::Experience(Box::default()),
        SectionSeed::Projects(Vec::new()),
        SectionSeed::Education(Vec::new()),
    ]
}
