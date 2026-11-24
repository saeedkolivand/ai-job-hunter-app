//! Version stamp + staleness of a note's inputs, and the wire/stored shape of its fit.

use super::*;
use crate::autopilot::tests::support::found_job_full;

fn noted(resume: &str, description: Option<&str>) -> FoundJob {
    FoundJob {
        description: description.map(str::to_string),
        assistant_notes: Some("note".into()),
        notes_basis: Some(notes_basis(resume, description)),
        ..found_job_full("https://acme.example/1", "CRM", "Acme", 0)
    }
}

#[test]
fn basis_is_stable_and_whitespace_insensitive() {
    assert_eq!(
        notes_basis("cv", Some("jd")),
        notes_basis("  cv\n", Some(" jd "))
    );
    assert_eq!(notes_basis("cv", None), notes_basis("cv", Some("")));
    assert_eq!(notes_basis("cv", Some("jd")).len(), 2 * STAMP_HEX + 1);
}

#[test]
fn basis_changes_when_either_input_changes() {
    let base = notes_basis("cv", Some("jd"));
    assert_ne!(base, notes_basis("cv v2", Some("jd")));
    assert_ne!(base, notes_basis("cv", Some("jd edited")));
}

#[test]
fn fresh_note_is_not_stale() {
    assert!(!notes_stale(&noted("cv", Some("jd")), "cv"));
}

#[test]
fn changed_resume_or_description_makes_the_note_stale() {
    let job = noted("cv", Some("jd"));
    assert!(notes_stale(&job, "a newer cv"));
    let edited = FoundJob {
        description: Some("jd edited".into()),
        ..job
    };
    assert!(notes_stale(&edited, "cv"));
}

#[test]
fn legacy_note_without_a_stamp_is_labelled_stale_but_never_regenerated() {
    let legacy = FoundJob {
        notes_basis: None,
        ..noted("cv", Some("jd"))
    };
    assert!(notes_stale(&legacy, "cv"));
    assert!(!needs_regeneration(&legacy, "cv", Some("jd")));
}

#[test]
fn a_job_without_a_note_is_never_stale() {
    let bare = found_job_full("https://acme.example/2", "CRM", "Acme", 0);
    assert!(!notes_stale(&bare, "cv"));
    assert!(!needs_regeneration(&bare, "cv", Some("jd")));
}

#[test]
fn regeneration_is_needed_only_on_a_stamp_mismatch() {
    let job = noted("cv", Some("jd"));
    assert!(!needs_regeneration(&job, "cv", Some("jd")));
    assert!(needs_regeneration(&job, "new cv", Some("jd")));
    assert!(needs_regeneration(&job, "cv", Some("jd edited")));
}

#[test]
fn snapshot_carries_note_and_fit_together_and_wire_omits_empties() {
    let job = FoundJob {
        mandatory_gaps: vec!["Italian C1 required, résumé shows B1".into()],
        fit_category: Some(FitCategory::ExplicitGap),
        notes_stale: true,
        ..noted("cv", Some("jd"))
    };
    let snap = NoteSnapshot::of(&job).expect("has a note");
    assert_eq!(snap.notes.as_deref(), Some("note"));
    let wire = serde_json::to_value(&snap.fit).unwrap();
    assert_eq!(wire["fitCategory"], "explicitGap");
    assert_eq!(wire["notesStale"], true);
    assert_eq!(
        wire["mandatoryGaps"][0],
        "Italian C1 required, résumé shows B1"
    );
    assert!(wire.get("toVerify").is_none());
    let empty = serde_json::to_value(NoteFitView::default()).unwrap();
    assert_eq!(empty, serde_json::json!({}));
    assert!(NoteSnapshot::of(&found_job_full("u", "t", "c", 0)).is_none());
}

#[test]
fn a_note_from_a_copy_with_another_description_is_stale_beside_the_canonical_one() {
    let copy = NoteSnapshot::of(&noted("cv", Some("copy jd"))).expect("has a note");
    assert!(!copy.clone().shown_with(Some("copy jd")).fit.notes_stale);
    assert!(
        copy.clone()
            .shown_with(Some("canonical jd"))
            .fit
            .notes_stale
    );
    assert!(copy.shown_with(None).fit.notes_stale);
    assert!(
        !NoteSnapshot::default()
            .shown_with(Some("x"))
            .fit
            .notes_stale
    );
}

#[test]
fn legacy_record_without_the_fit_fields_deserializes_to_absent() {
    let json = r#"{"title":"t","company":"c","url":"u","foundAt":1,"assistantNotes":"old"}"#;
    let job: FoundJob = serde_json::from_str(json).expect("legacy found job");
    assert!(job.mandatory_gaps.is_empty() && job.to_verify.is_empty());
    assert!(job.fit_category.is_none() && job.notes_basis.is_none() && !job.notes_stale);
}
