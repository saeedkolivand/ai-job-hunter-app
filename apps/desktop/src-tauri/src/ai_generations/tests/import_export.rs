use super::{support::*, *};

#[test]
fn migration_defaults_link_fields_for_legacy_records() {
    // A record exported before the link columns existed (no jobUrl/board) must
    // import via serde defaults, not fail.
    let (_dir, store) = open_store();
    let legacy = serde_json::json!([{
        "id": "old-1",
        "createdAt": 1,
        "candidateName": "Jane",
        "jobTitle": "Engineer",
        "companyName": "Acme",
        "resumeLanguage": "en",
        "jobAdLanguage": "en",
        "targetLanguage": "en",
        "mismatch": false,
        "topRequirements": [],
        "mode": "ats",
        "resumeText": "",
        "coverLetterText": "",
        "jobAd": ""
    }]);
    let n = store.import(&legacy).unwrap();
    assert_eq!(n, 1);
    let list = store.list();
    assert_eq!(list[0].job_url, "");
    assert_eq!(list[0].board, "");
    assert!(list[0].application_answers.is_empty());
    assert_eq!(list[0].company_brief, "");
    assert!(list[0].interview_questions.is_empty());
    assert_eq!(list[0].email_subject, "");
    assert_eq!(list[0].email_body, "");
    assert_eq!(list[0].quality_report, "");
}

/// A backup round-trip must carry the email draft — otherwise restoring a
/// backup silently drops the user's saved application email.
#[test]
fn export_import_round_trip_preserves_the_email_draft() {
    let (_src_dir, src) = open_store();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.email_subject = "Application: Engineer".into();
    rec.email_body = "Hello,\n\nI'd like to apply.".into();
    src.insert(&rec).unwrap();

    let exported = src.export();

    let (_dst_dir, dst) = open_store();
    assert_eq!(dst.import(&exported).unwrap(), 1);

    let list = dst.list();
    assert_eq!(list[0].email_subject, "Application: Engineer");
    assert_eq!(list[0].email_body, "Hello,\n\nI'd like to apply.");
}

// ── R1 — Import rollback regression guard ────────────────────────────────────
//
// The C1 fix added a transaction around clear + repopulate in `DataStore::import`.
// These tests pin that fix: a malformed LATER record must abort the import and
// leave the store's PRIOR data fully intact (neither wiped nor half-restored).

/// A valid generation serialised with all required camelCase fields. Used as the
/// "good" payload in rollback tests.
fn valid_generation_json(id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "createdAt": 1_000_000u64,
        "candidateName": "Jane",
        "jobTitle": "Engineer",
        "companyName": "Acme",
        "resumeLanguage": "en",
        "jobAdLanguage": "en",
        "targetLanguage": "en",
        "mismatch": false,
        "topRequirements": ["rust"],
        "mode": "ats",
        "resumeText": "My resume",
        "coverLetterText": "My cover letter",
        "jobAd": "Job description"
    })
}

#[test]
fn import_with_invalid_later_record_returns_err_and_prior_data_intact() {
    // R1 — Build a store with one known record, then attempt an import whose
    // LAST element has an invalid type for `mismatch` (must be bool, not "bad").
    // Assert: import returns Err AND the prior record is still present.
    let (_dir, store) = open_store();

    // Seed the store with prior data.
    insert(&store, "prior-1", "");
    assert_eq!(store.list().len(), 1, "precondition: one prior record");

    // Build a bundle: first element is valid, second is malformed (`mismatch` is
    // a string instead of a bool). The malformed record must abort the whole import.
    let bundle = serde_json::json!([
        valid_generation_json("new-1"),
        {
            "id": "bad-2",
            "createdAt": 2_000_000u64,
            "candidateName": "Jane",
            "jobTitle": "Engineer",
            "companyName": "Acme",
            "resumeLanguage": "en",
            "jobAdLanguage": "en",
            "targetLanguage": "en",
            "mismatch": "not-a-bool",   // ← wrong type: must be bool
            "topRequirements": [],
            "mode": "ats",
            "resumeText": "",
            "coverLetterText": "",
            "jobAd": ""
        }
    ]);

    let result = store.import(&bundle);
    assert!(
        result.is_err(),
        "import of a malformed bundle must return Err; got Ok"
    );

    // Prior data must be fully intact — not wiped, not partially replaced.
    let remaining = store.list();
    assert_eq!(
        remaining.len(),
        1,
        "import rollback must leave the prior 1 record intact; got {} records",
        remaining.len()
    );
    assert_eq!(
        remaining[0].id, "prior-1",
        "the surviving record must be the original 'prior-1', not a partial import"
    );
}

#[test]
fn import_with_all_valid_records_replaces_prior_data() {
    // R1 happy-path companion: confirms the import transaction DOES commit when
    // the bundle is fully valid — prior data is replaced, not preserved.
    let (_dir, store) = open_store();

    insert(&store, "prior-1", "");
    insert(&store, "prior-2", "");
    assert_eq!(store.list().len(), 2, "precondition: two prior records");

    let bundle = serde_json::json!([
        valid_generation_json("new-1"),
        valid_generation_json("new-2"),
        valid_generation_json("new-3"),
    ]);

    let n = store.import(&bundle).unwrap();
    assert_eq!(n, 3, "import must report 3 records restored");

    let list = store.list();
    assert_eq!(list.len(), 3, "store must now hold the 3 imported records");
    let ids: Vec<&str> = list.iter().map(|r| r.id.as_str()).collect();
    assert!(
        ids.contains(&"new-1") && ids.contains(&"new-2") && ids.contains(&"new-3"),
        "imported ids must be present; got {ids:?}"
    );
    assert!(
        !ids.contains(&"prior-1"),
        "prior record 'prior-1' must not survive a successful import"
    );
}

#[test]
fn import_non_array_returns_err_and_prior_data_intact() {
    // R1 edge-case: the top-level value is not an array at all. The store must
    // return Err before touching any data.
    let (_dir, store) = open_store();

    insert(&store, "prior-1", "");

    let result = store.import(&serde_json::json!({"bad": true}));
    assert!(result.is_err(), "non-array input must be rejected");

    let remaining = store.list();
    assert_eq!(
        remaining.len(),
        1,
        "prior data must be intact after non-array import rejection"
    );
    assert_eq!(remaining[0].id, "prior-1");
}

// ── Finding 7 — application_id survives export → import round-trip ────────────
//
// `application_id` is the parent Application FK. Before the fix, export/import
// dropped it, so a backup round-trip orphaned every linked generation
// (`remove_for_application` stopped matching the restored rows). This pins the
// FK through the round-trip.

#[test]
fn export_import_round_trip_preserves_application_id() {
    let app_id = "app-123";

    // Source store: one generation linked to an application (the FK that
    // `applications::ApplicationStore::open` would set via its backfill UPDATE).
    let (_src_dir, src) = open_store();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.application_id = Some(app_id.to_string());
    src.insert(&rec).unwrap();

    // Export the backup (non-destructive — reads via `list`).
    let exported = src.export();

    // Fresh store in a NEW temp dir imports the backup.
    let (_dst_dir, dst) = open_store();
    let n = dst.import(&exported).unwrap();
    assert_eq!(n, 1, "one record restored");

    // The FK survived the round-trip: the restored row is still linked, so
    // `remove_for_application` matches it (== 1). Before the fix the FK was
    // dropped on export/import and this returned 0 (orphaned generation).
    assert_eq!(
        dst.remove_for_application(app_id).unwrap(),
        1,
        "application_id must survive export/import so the link still matches"
    );
}

// ── quality_report (ADR-007 addendum) ──────────────────────────────────────

/// A backup round-trip must carry the quality report — otherwise restoring a
/// backup silently drops the deterministic content-quality findings.
#[test]
fn export_import_round_trip_preserves_the_quality_report() {
    let (_src_dir, src) = open_store();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.quality_report = r#"{"ok":true,"issues":[],"metrics":{}}"#.into();
    src.insert(&rec).unwrap();

    let exported = src.export();

    let (_dst_dir, dst) = open_store();
    assert_eq!(dst.import(&exported).unwrap(), 1);

    assert_eq!(
        dst.list()[0].quality_report,
        r#"{"ok":true,"issues":[],"metrics":{}}"#
    );
}

/// L-5: `import` is a write path for a user-supplied backup FILE — just as
/// untrusted as the IPC save path, which already guards `quality_report`
/// (`commands::ai_generations::ai_generations_save`). Before this fix,
/// `import` wrote the bundle's `quality_report` straight to the column with
/// no guard at all, unlike every other write path for this column.
#[test]
fn import_sanitizes_an_oversized_quality_report_to_the_empty_string() {
    let huge = "x".repeat(QUALITY_REPORT_MAX_BYTES + 10_000);
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.quality_report = huge.clone();
    let bundle = serde_json::json!([rec]);

    let (_dir, store) = open_store();
    assert_eq!(store.import(&bundle).unwrap(), 1);

    let imported = store.list();
    assert_eq!(imported.len(), 1);
    assert_eq!(
        imported[0].quality_report, "",
        "an over-cap bundle's quality_report must drop to the empty sentinel on \
         import, not a byte-truncated blob"
    );
}

/// The finding this regression-tests: a byte-position clamp on an otherwise
/// VALID JSON object truncates it MID-STRUCTURE — unlike the "huge" garbage
/// fixture above (never valid JSON either way), this fixture is a real
/// wrapper that only becomes invalid because of where a byte clamp would cut
/// it. `import` must reach the same documented `''` sentinel, not a
/// truncated-but-still-invalid blob that only accidentally reads as
/// "no report".
#[test]
fn import_sanitizes_an_oversized_valid_json_report_to_the_empty_string() {
    let oversized = serde_json::json!({
        "schemaVersion": 1,
        "pipeline": "resume",
        "generatedAt": 1,
        "resume": { "blob": "x".repeat(QUALITY_REPORT_MAX_BYTES) }
    })
    .to_string();
    assert!(
        oversized.len() > QUALITY_REPORT_MAX_BYTES,
        "test fixture must actually exceed the cap on its own"
    );
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.quality_report = oversized;
    let bundle = serde_json::json!([rec]);

    let (_dir, store) = open_store();
    assert_eq!(store.import(&bundle).unwrap(), 1);

    assert_eq!(
        store.list()[0].quality_report,
        "",
        "an over-cap incoming report must be dropped to the empty sentinel, \
         never stored as truncated (unparseable) JSON"
    );
}
