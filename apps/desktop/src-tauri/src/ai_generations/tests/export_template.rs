//! The nullable `last_export_template` column: migration, write-at-export, merge, and
//! the backup / sync round trip.

use super::{support::*, *};

const JOB: &str = "https://acme.com/job/1";

#[test]
fn migration_adds_a_nullable_column_and_keeps_legacy_rows() {
    let dir = TempDir::new().unwrap();
    {
        let (conn, _) = conn_before(&dir, "add_last_export_template");
        assert!(
            !crate::db::column_exists(&conn, "ai_generations", "last_export_template"),
            "precondition: the legacy schema has no template column"
        );
        conn.execute(
            "INSERT INTO ai_generations (id, created_at, resume_text) VALUES ('legacy', 1000, 'R')",
            [],
        )
        .unwrap();
    }
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let list = store.list();
    assert_eq!(list.len(), 1, "the legacy row survives");
    assert_eq!(list[0].resume_text, "R");
    assert_eq!(list[0].last_export_template, None, "old rows stay NULL");
}

#[test]
fn recording_an_export_stores_the_template_on_the_jobs_generation() {
    let (_dir, store) = open_store();
    insert(&store, "g1", JOB);
    insert(&store, "g2", "https://acme.com/job/2");

    assert!(store.record_export_template(JOB, "swiss-minimal").unwrap());

    let by_id = |id: &str| store.list().into_iter().find(|r| r.id == id).unwrap();
    assert_eq!(
        by_id("g1").last_export_template.as_deref(),
        Some("swiss-minimal")
    );
    assert_eq!(
        by_id("g2").last_export_template,
        None,
        "other jobs untouched"
    );

    assert!(store.record_export_template(JOB, "classic").unwrap());
    assert_eq!(
        by_id("g1").last_export_template.as_deref(),
        Some("classic"),
        "last export wins"
    );
}

#[test]
fn recording_an_export_for_a_job_without_a_generation_is_a_no_op() {
    let (_dir, store) = open_store();
    assert!(!store
        .record_export_template("https://nowhere.example/job", "classic")
        .unwrap());
    assert!(store.list().is_empty());
}

#[test]
fn a_later_content_save_keeps_the_recorded_template() {
    let (_dir, store) = open_store();
    insert(&store, "g1", JOB);
    store.record_export_template(JOB, "atelier").unwrap();

    // An answers-only save for the same job merges onto the row.
    store.save_application(content_less("g-new", JOB)).unwrap();

    let rec = store.find_for_job(JOB).unwrap();
    assert_eq!(rec.last_export_template.as_deref(), Some("atelier"));
}

#[test]
fn a_content_save_built_from_a_stale_read_never_writes_the_old_template_back() {
    let (_dir, store) = open_store();
    insert(&store, "g1", JOB);
    store.record_export_template(JOB, "classic").unwrap();
    // The save reads (and merges onto) the row while it still says `classic` ...
    let stale = merge_application(store.find_for_job(JOB).unwrap(), content_less("g-new", JOB));
    // ... an export lands before the save's UPDATE runs.
    store.record_export_template(JOB, "atelier").unwrap();
    store.update(&stale).unwrap();

    let rec = store.find_for_job(JOB).unwrap();
    assert_eq!(rec.last_export_template.as_deref(), Some("atelier"));
}

#[test]
fn recording_for_a_generation_deleted_in_the_meantime_reports_false() {
    let (_dir, store) = open_store();
    insert(&store, "g1", JOB);
    assert!(store.set_export_template("g1", "atelier").unwrap());
    assert!(!store.set_export_template("gone", "atelier").unwrap());
}

#[test]
fn merge_prefers_the_incoming_template_and_falls_back_to_the_existing_one() {
    let mut existing = record("a", JOB);
    existing.last_export_template = Some("classic".into());
    let mut incoming = record("b", JOB);

    assert_eq!(
        merge_application(existing.clone(), incoming.clone())
            .last_export_template
            .as_deref(),
        Some("classic")
    );
    incoming.last_export_template = Some("meridian".into());
    assert_eq!(
        merge_application(existing, incoming)
            .last_export_template
            .as_deref(),
        Some("meridian")
    );
}

#[test]
fn export_import_round_trip_preserves_the_template_and_an_absent_one_stays_absent() {
    let (_src_dir, src) = open_store();
    insert(&src, "g1", JOB);
    insert(&src, "g2", "https://acme.com/job/2");
    src.record_export_template(JOB, "swiss-minimal").unwrap();

    let exported = src.export();
    let rows = exported.as_array().unwrap();
    let wire = |id: &str| rows.iter().find(|r| r["id"] == id).unwrap().clone();
    assert_eq!(wire("g1")["lastExportTemplate"], "swiss-minimal");
    assert!(
        wire("g2").get("lastExportTemplate").is_none(),
        "absent is omitted, not null"
    );

    let (_dst_dir, dst) = open_store();
    assert_eq!(dst.import(&exported).unwrap(), 2);
    let by_id = |id: &str| dst.list().into_iter().find(|r| r.id == id).unwrap();
    assert_eq!(
        by_id("g1").last_export_template.as_deref(),
        Some("swiss-minimal")
    );
    assert_eq!(by_id("g2").last_export_template, None);
}

#[test]
fn a_pre_migration_bundle_without_the_field_imports_with_none() {
    let (_src_dir, src) = open_store();
    insert(&src, "g1", JOB);
    let mut exported = src.export();
    exported[0]
        .as_object_mut()
        .unwrap()
        .remove("lastExportTemplate");

    let (_dst_dir, dst) = open_store();
    assert_eq!(dst.import(&exported).unwrap(), 1);
    assert_eq!(dst.list()[0].last_export_template, None);
}

#[test]
fn a_sync_upsert_carries_the_template() {
    use crate::data_store::SyncSection;
    let (_src_dir, src) = open_store();
    insert(&src, "g1", JOB);
    src.record_export_template(JOB, "atelier").unwrap();
    let payload = src.sync_records().unwrap().remove(0).payload;

    let (_dst_dir, dst) = open_store();
    dst.upsert_record("g1", &payload).unwrap();
    assert_eq!(
        dst.list()[0].last_export_template.as_deref(),
        Some("atelier")
    );
}
