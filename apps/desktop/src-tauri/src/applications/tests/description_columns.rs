use super::{support::*, *};

// ── job_description column: migration, persistence, and merge-preserve ────────
//
// Three behaviours pinned in ONE test function:
//   1. Additive migration applies cleanly on top of a populated old-schema DB
//      (no job_description column) → existing row survives with DEFAULT ''.
//   2. upsert_for_origin with a non-empty JD persists it (mirrors the import path).
//   3. Merge-preserve: empty incoming JD keeps the stored JD; non-empty incoming
//      JD overwrites it.  One Application throughout (no accidental duplicates).

#[test]
fn job_description_migrates_persists_and_merge_preserves() {
    let dir = TempDir::new().unwrap();

    // ── Step 1: seed a legacy DB (migrations 1+2 applied, migration 3 not yet) ─
    //
    // We hand-create applications.db with the pre-job_description schema and set
    // PRAGMA user_version = 2 so ApplicationStore::open applies only migration 3
    // (ALTER TABLE … ADD COLUMN job_description …) when it opens.
    let legacy_id = "app-legacy-001";
    seed_legacy_row(dir.path(), 2, legacy_id);

    // Open the store — migration 3 (ADD COLUMN job_description … DEFAULT '')
    // must apply without error and the pre-existing row must survive intact.
    let store = ApplicationStore::open(dir.path()).unwrap();

    let legacy_app = store
        .get(legacy_id)
        .expect("legacy row must be readable after migration");
    assert_eq!(
        legacy_app.job_description, "",
        "legacy row must get DEFAULT '' for job_description after additive migration"
    );
    assert_eq!(
        legacy_app.id, legacy_id,
        "legacy row id must be unchanged after migration"
    );

    // ── Step 2: import path — upsert with a non-empty JD persists it ──────────
    let jd = "Senior Rust role. Async, Tokio.";
    let m_with_jd = ApplicationMeta {
        job_description: jd.into(),
        ..meta("Acme", "Engineer")
    };
    let app_id = store
        .upsert_for_origin(
            "https://acme.com/job/import/1",
            "linkedin",
            &m_with_jd,
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();

    assert_eq!(
        store.get(&app_id).unwrap().job_description,
        jd,
        "upsert_for_origin must persist the supplied job_description"
    );

    // ── Step 3a: merge-preserve — empty incoming JD keeps the stored JD ───────
    store
        .upsert_for_origin(
            "https://acme.com/job/import/1",
            "linkedin",
            &meta("Acme", "Engineer"), // job_description: String::new()
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();

    assert_eq!(
        store.get(&app_id).unwrap().job_description,
        jd,
        "empty incoming job_description must NOT overwrite the stored JD"
    );

    // Still exactly ONE Application for this URL — no duplicate created.
    assert_eq!(
        store
            .list()
            .iter()
            .filter(|a| a.job_url == "https://acme.com/job/import/1")
            .count(),
        1,
        "merge must never duplicate the Application"
    );

    // ── Step 3b: non-empty incoming JD overwrites the stored JD ───────────────
    let updated_jd = "Updated JD";
    let m_updated = ApplicationMeta {
        job_description: updated_jd.into(),
        ..meta("Acme", "Engineer")
    };
    store
        .upsert_for_origin(
            "https://acme.com/job/import/1",
            "linkedin",
            &m_updated,
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();

    assert_eq!(
        store.get(&app_id).unwrap().job_description,
        updated_jd,
        "non-empty incoming job_description must overwrite the stored JD"
    );

    // Final sanity: still one Application for the URL.
    assert_eq!(
        store
            .list()
            .iter()
            .filter(|a| a.job_url == "https://acme.com/job/import/1")
            .count(),
        1,
        "store must hold exactly one Application after all upserts"
    );
}

// ── Security: server-side job_description cap (the real trust boundary) ───────
//
// The renderer Zod cap is UX-only; the extension import path persists
// attacker-influenced page HTML that never passes through it. The store must
// clamp the JD to MAX_JOB_DESCRIPTION_BYTES on a UTF-8 char boundary (truncate,
// never reject) on BOTH write entry points: upsert_for_origin and update_fields.

#[test]
fn job_description_is_clamped_on_char_boundary_via_both_write_paths() {
    // Over-cap (~250 KB) JD whose 4-byte 'U+1F600' STARTS at byte MAX-1, so a
    // naive byte-cut at MAX lands mid-char and must be walked back to MAX-1.
    // After the walk-back the emoji and everything after it is dropped → stored
    // is exactly MAX-1 'a's.
    let jd = "a".repeat(MAX_JOB_DESCRIPTION_BYTES - 1) + "\u{1F600}" + &"b".repeat(1000);
    let expected = "a".repeat(MAX_JOB_DESCRIPTION_BYTES - 1);
    assert!(
        jd.len() > MAX_JOB_DESCRIPTION_BYTES,
        "precondition: input is over-cap"
    );

    // Direct helper assertion: an under-cap string is returned unchanged.
    let small = "short JD".to_string();
    assert_eq!(
        clamp_job_description(small.clone()),
        small,
        "under-cap input must pass through unchanged"
    );

    // ── Path A — upsert_for_origin (import funnel + every creation trigger) ────
    let (_dir_a, store_a) = open_store();
    let id_a = store_a
        .upsert_for_origin(
            "https://acme.com/job/clamp/a",
            "linkedin",
            &ApplicationMeta {
                job_description: jd.clone(),
                ..meta("Acme", "Eng")
            },
            ApplicationOrigin::Saved,
            Some(false),
        )
        .unwrap();
    let stored_a = store_a.get(&id_a).unwrap().job_description;
    assert!(
        stored_a.len() <= MAX_JOB_DESCRIPTION_BYTES,
        "upsert_for_origin must clamp JD to <= MAX (got {})",
        stored_a.len()
    );
    assert!(
        std::str::from_utf8(stored_a.as_bytes()).is_ok(),
        "stored JD must be valid UTF-8 (char-boundary cut)"
    );
    assert_eq!(
        stored_a.len(),
        MAX_JOB_DESCRIPTION_BYTES - 1,
        "cut must walk back off the 4-byte char to MAX-1"
    );
    assert_eq!(
        stored_a, expected,
        "the multibyte char and everything after it must be dropped"
    );

    // ── Path B — update_fields (applications_update IPC; attacker-reachable) ───
    let (_dir_b, store_b) = open_store();
    let id_b = track(&store_b, "C", "T");
    edit(&store_b, &id_b, |p| p.job_description = Some(jd.clone()));
    let stored_b = store_b.get(&id_b).unwrap().job_description;
    assert!(
        stored_b.len() <= MAX_JOB_DESCRIPTION_BYTES,
        "update_fields must clamp JD to <= MAX (got {})",
        stored_b.len()
    );
    assert!(
        std::str::from_utf8(stored_b.as_bytes()).is_ok(),
        "stored JD must be valid UTF-8 (char-boundary cut)"
    );
    assert_eq!(
        stored_b.len(),
        MAX_JOB_DESCRIPTION_BYTES - 1,
        "update_fields cut must walk back off the 4-byte char to MAX-1"
    );
    assert_eq!(stored_b, expected);

    // None must leave the (now-clamped) JD untouched.
    edit(&store_b, &id_b, |p| p.notes = Some("note".into()));
    assert_eq!(
        store_b.get(&id_b).unwrap().job_description,
        expected,
        "None job_description must preserve the existing (clamped) JD"
    );
}

/// Old-schema applications.db (no job_summary column) must gain it via the
/// additive migration with NO data loss, then accept/return a summary.
#[test]
fn job_summary_migration_adds_column_without_data_loss() {
    let dir = TempDir::new().unwrap();
    // Hand-build the PRE-job_summary applications table (the create_applications
    // shape) and seed one row, simulating a DB from before this migration.
    legacy_db(dir.path(), 0)
        .execute(
            "INSERT INTO applications (id, status, created_at, updated_at, company)
             VALUES ('old-1', 'applied', 1000, 1000, 'Legacy Corp')",
            [],
        )
        .unwrap();
    // Opening the store runs migrations (incl. add_applications_job_summary).
    let store = ApplicationStore::open(dir.path()).unwrap();
    let app = store
        .get("old-1")
        .expect("legacy row must survive migration");
    assert_eq!(app.company, "Legacy Corp", "no data loss on migrated row");
    assert_eq!(app.job_summary, "", "new column defaults to empty");
}

/// An upsert with a non-empty job_summary persists it; a follow-up upsert with an
/// EMPTY summary must NOT clobber the stored value (merge-preserve, like `brief`).
#[test]
fn job_summary_upsert_persists_and_merge_preserves() {
    let (_dir, store) = open_store();
    let url = "https://acme.com/job/777";

    let mut m = meta("Acme", "Engineer");
    m.job_summary = "A concise role summary.".into();
    let id = upsert(&store, url, "linkedin", &m, ApplicationOrigin::Generate);
    assert_eq!(
        store.get(&id).unwrap().job_summary,
        "A concise role summary."
    );

    // Re-upsert the same url with an EMPTY summary — must keep the stored one.
    let m2 = meta("Acme", "Engineer"); // job_summary == ""
    let id2 = upsert(&store, url, "linkedin", &m2, ApplicationOrigin::Generate);
    assert_eq!(id, id2, "same url merges into one Application");
    assert_eq!(
        store.get(&id).unwrap().job_summary,
        "A concise role summary.",
        "empty incoming summary must not clobber the stored one"
    );
}

/// `update_fields` can set the summary, and the 50 KB server cap truncates an
/// oversize value on a UTF-8 char boundary (no panic, no split char).
#[test]
fn job_summary_update_and_50kb_clamp_truncates_on_char_boundary() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T");

    // Normal update path persists a summary.
    edit(&store, &id, |p| p.job_summary = Some("hello".into()));
    assert_eq!(store.get(&id).unwrap().job_summary, "hello");

    // >50 KB of a 2-byte char ('é' = U+00E9). 50_000 is even and every boundary in
    // an all-'é' string is even, so exactly 25_000 whole chars (50_000 bytes) fit.
    let big = "é".repeat(40_000); // 80_000 bytes
    edit(&store, &id, |p| p.job_summary = Some(big));
    let stored = store.get(&id).unwrap().job_summary;
    assert!(
        stored.len() <= 50_000,
        "must be capped at 50 KB, got {}",
        stored.len()
    );
    assert!(
        stored.chars().all(|c| c == 'é'),
        "no split/garbage char at the cut"
    );
    assert_eq!(
        stored.chars().count(),
        25_000,
        "exactly the whole chars that fit"
    );
}
