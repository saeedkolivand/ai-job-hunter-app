use super::{support::*, *};

/// The email fields merge exactly like `cover_letter_text` (`pick`): a save that
/// carries a draft OVERWRITES the stored one (so re-generating replaces it),
/// while a save from another surface leaves it alone.
#[test]
fn merge_email_draft_overwrites_on_regeneration_and_survives_other_saves() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.email_subject = "OLD SUBJECT".into();
    existing.email_body = "OLD BODY".into();

    // A regenerated email carries both fields → both are replaced.
    let mut regenerated = record("g2", "https://acme.com/job/1");
    regenerated.email_subject = "NEW SUBJECT".into();
    regenerated.email_body = "NEW BODY".into();
    let merged = merge_application(existing.clone(), regenerated);
    assert_eq!(merged.email_subject, "NEW SUBJECT");
    assert_eq!(merged.email_body, "NEW BODY");
    assert_eq!(merged.cover_letter_text, "C", "cover is untouched");

    // A résumé/answers save carries no email → the stored draft is preserved.
    let mut answers_only = record("g3", "https://acme.com/job/1");
    answers_only.application_answers = vec![answer("why-company")];
    let merged = merge_application(existing.clone(), answers_only);
    assert_eq!(merged.email_subject, "OLD SUBJECT", "draft is not wiped");
    assert_eq!(merged.email_body, "OLD BODY", "draft is not wiped");

    // Subject and body merge ATOMICALLY: a save that carries a body but an empty
    // subject owns the whole draft, so the stale subject must NOT survive glued
    // onto the new body. This is the model breaking the `Subject:` line contract
    // (the renderer's `splitEmail` then yields subject == "" and body == the raw
    // output) — the one case that actually reaches here, since the email surface
    // always writes both fields together.
    let mut preamble_output = record("g4", "https://acme.com/job/1");
    preamble_output.email_subject = String::new();
    preamble_output.email_body = "Sure! Here is your email: Hello,".into();
    let merged = merge_application(existing.clone(), preamble_output);
    assert_eq!(
        merged.email_subject, "",
        "a stale subject must not be glued onto a newly generated body"
    );
    assert_eq!(merged.email_body, "Sure! Here is your email: Hello,");

    // Mirror case: a subject with an empty body also owns the whole draft, so
    // the stale body must not survive under a newly generated subject.
    let mut subject_only = record("g5", "https://acme.com/job/1");
    subject_only.email_subject = "NEW SUBJECT".into();
    subject_only.email_body = String::new();
    let merged = merge_application(existing, subject_only);
    assert_eq!(merged.email_subject, "NEW SUBJECT");
    assert_eq!(
        merged.email_body, "",
        "a stale body must not survive under a newly generated subject"
    );
}

#[test]
fn merge_layers_interview_questions_without_clobbering_other_fields() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.cover_letter_text = "COVER".into();
    existing.interview_questions = vec![];

    // An interview-questions-only save: empty résumé/cover, carries questions.
    let mut incoming = content_less("g2", "https://acme.com/job/1");
    incoming.interview_questions = vec![interview_question("iq-1")];

    let merged = merge_application(existing, incoming);

    assert_eq!(merged.cover_letter_text, "COVER", "cover is not wiped");
    assert_eq!(merged.interview_questions, vec![interview_question("iq-1")]);
}

/// A corrected regeneration must be able to CLEAR a stale language-mismatch
/// warning, and a content-less (answers/interview-only) save must NOT clobber a
/// real prior verdict. The old `incoming || existing` made a `true` permanent.
#[test]
fn merge_mismatch_follows_a_language_bearing_save_and_ignores_a_content_less_one() {
    // Prior verdict: mismatch (a German résumé generated against an English ad).
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.mismatch = true;

    // A corrected regeneration carries the language pair and says "no mismatch".
    let mut fixed = record("g2", "https://acme.com/job/1");
    fixed.resume_language = "en".into();
    fixed.job_ad_language = "en".into();
    fixed.mismatch = false;
    assert!(
        !merge_application(existing.clone(), fixed).mismatch,
        "a language-bearing save with mismatch=false must clear the stale warning"
    );

    // An answers-only save carries no language pair; its default mismatch=false
    // must not wipe the existing verdict.
    let mut answers_only = record("g3", "https://acme.com/job/1");
    answers_only.resume_language = String::new();
    answers_only.job_ad_language = String::new();
    answers_only.mismatch = false;
    assert!(
        merge_application(existing, answers_only).mismatch,
        "a content-less save must not clobber a real prior mismatch verdict"
    );
}

#[test]
fn merge_layers_answers_onto_an_existing_cover_without_clobbering() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.cover_letter_text = "COVER".into();
    existing.application_answers = vec![];

    // An answers-only save: empty résumé/cover, but carries answers + brief.
    let mut incoming = content_less("g2", "https://acme.com/job/1");
    incoming.application_answers = vec![answer("why-company")];
    incoming.company_brief = "brief".into();

    let merged = merge_application(existing, incoming);

    assert_eq!(merged.id, "g1", "keeps the existing row id");
    assert_eq!(merged.cover_letter_text, "COVER", "cover is not wiped");
    assert_eq!(merged.application_answers, vec![answer("why-company")]);
    assert_eq!(merged.company_brief, "brief");
}

/// A résumé-writing save carries a wrapper with a `resume` key: it overlays
/// the envelope fields (`schemaVersion`/`pipeline`/`generatedAt`) plus
/// `resume`, and leaves a stored `coverLetter` sub-report untouched.
#[test]
fn merge_quality_report_resume_save_overlays_resume_key_and_envelope() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.quality_report =
        r#"{"schemaVersion":1,"pipeline":"combined","generatedAt":100,"coverLetter":{"ok":false}}"#
            .into();

    let mut resume_save = record("g2", "https://acme.com/job/1");
    resume_save.quality_report =
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":200,"resume":{"ok":true}}"#.into();

    let merged = merge_application(existing, resume_save);
    let value: serde_json::Value = serde_json::from_str(&merged.quality_report).unwrap();
    assert_eq!(value["pipeline"], "resume");
    assert_eq!(value["generatedAt"], 200);
    assert_eq!(value["resume"]["ok"], true);
    assert_eq!(
        value["coverLetter"]["ok"], false,
        "the coverLetter sub-report from the existing wrapper must survive a résumé-only save"
    );
}

/// An answers-only save carries no report at all (`quality_report` defaults to
/// `""`) — the existing wrapper must survive byte-for-byte.
#[test]
fn merge_quality_report_content_less_save_keeps_existing_report_untouched() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.quality_report =
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":100,"resume":{"ok":true}}"#.into();

    let answers_only = record("g2", "https://acme.com/job/1"); // quality_report == ""
    let merged = merge_application(existing.clone(), answers_only);
    assert_eq!(merged.quality_report, existing.quality_report);
}

/// A letter-only save carries a wrapper with a `coverLetter` key: it must
/// preserve a stored `resume` sub-report rather than clobbering it.
#[test]
fn merge_quality_report_letter_save_preserves_stored_resume_key() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.quality_report =
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":150,"resume":{"ok":false}}"#.into();

    let mut letter_save = record("g2", "https://acme.com/job/1");
    letter_save.quality_report =
        r#"{"schemaVersion":1,"pipeline":"letter","generatedAt":300,"coverLetter":{"ok":true}}"#
            .into();

    let merged = merge_application(existing, letter_save);
    let value: serde_json::Value = serde_json::from_str(&merged.quality_report).unwrap();
    assert_eq!(value["pipeline"], "letter");
    assert_eq!(value["coverLetter"]["ok"], true);
    assert_eq!(
        value["resume"]["ok"], false,
        "a letter-only save must not clobber the stored résumé sub-report"
    );
}

/// A garbage/corrupt existing value (not parseable JSON at all) must not block
/// a genuinely fresh incoming report — `incoming` wins outright since there is
/// nothing to overlay a key onto.
#[test]
fn merge_quality_report_unparseable_existing_recovers_via_incoming() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.quality_report = "not json at all".into();

    let mut incoming = record("g2", "https://acme.com/job/1");
    incoming.quality_report =
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":600,"resume":{"ok":true}}"#.into();

    let merged = merge_application(existing, incoming);
    assert_eq!(
        merged.quality_report,
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":600,"resume":{"ok":true}}"#
    );
}

/// Each side clamps its OWN write to `QUALITY_REPORT_MAX_BYTES`, but the merge
/// unions both sub-reports — a small, fresh résumé-only `incoming` overlaid
/// onto an `existing` wrapper whose stored `coverLetter` sub-report is already
/// near the cap can still push the merged object over budget. The merge must
/// not truncate the oversized JSON (unparseable on the next read, silently
/// reverting to the stale stored report on the read after that) — it falls
/// back to `incoming` whole, which is both known-parseable and already within
/// budget.
#[test]
fn merge_quality_report_oversized_union_falls_back_to_incoming() {
    let mut existing = record("g1", "https://acme.com/job/1");
    let huge = "x".repeat(QUALITY_REPORT_MAX_BYTES);
    existing.quality_report = serde_json::json!({
        "schemaVersion": 1,
        "pipeline": "letter",
        "generatedAt": 100,
        "coverLetter": { "blob": huge }
    })
    .to_string();
    assert!(
        existing.quality_report.len() > QUALITY_REPORT_MAX_BYTES,
        "test fixture must actually exceed the cap on its own"
    );

    let mut resume_save = record("g2", "https://acme.com/job/1");
    resume_save.quality_report =
        r#"{"schemaVersion":1,"pipeline":"resume","generatedAt":200,"resume":{"ok":true}}"#.into();

    let merged = merge_application(existing, resume_save.clone());
    assert_eq!(
        merged.quality_report, resume_save.quality_report,
        "an oversized merged union must fall back to the fresh incoming report verbatim, not a truncated blob"
    );
}
