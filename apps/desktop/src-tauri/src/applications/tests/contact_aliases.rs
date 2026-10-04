use super::{support::*, *};

// ── Contact unification — the alias write path and the import-time fold ──────
//
// `contact_name`/`contact_email` are THE primary contact; `recipient_*` are deprecated
// aliases that fold onto it, both on a patch and on an imported pre-unification bundle.

#[test]
fn both_inbound_contact_names_write_the_same_storage() {
    let (_dir, store) = open_store();
    let id = track(&store, "Acme", "Engineer");

    // Alias write (what ApplyByEmailTab sends today) lands on the canonical pair.
    edit(&store, &id, |p| {
        p.recipient_name = Some("Rita Recruiter".into());
        p.recipient_email = Some("rita@acme.com".into());
    });
    let app = store.get(&id).unwrap();
    assert_eq!(app.contact_name, "Rita Recruiter");
    assert_eq!(app.contact_email, "rita@acme.com");
    assert_eq!(app.recipient_name, "Rita Recruiter");

    // A canonical write is visible under BOTH names too.
    edit(&store, &id, |p| {
        p.contact_name = Some("Cora Contact".into());
        p.contact_email = Some("cora@acme.com".into());
    });
    let app = store.get(&id).unwrap();
    assert_eq!(app.contact_name, "Cora Contact");
    assert_eq!(app.recipient_name, "Cora Contact");
    assert_eq!(app.recipient_email, "cora@acme.com");

    // Both names in ONE patch: the canonical one wins.
    edit(&store, &id, |p| {
        p.contact_name = Some("Canonical".into());
        p.recipient_name = Some("Alias".into());
    });
    assert_eq!(store.get(&id).unwrap().contact_name, "Canonical");

    // Clearing through the alias clears the canonical pair — the stale mirror
    // must never resurrect the old value.
    edit(&store, &id, |p| {
        p.recipient_name = Some(String::new());
        p.recipient_email = Some(String::new());
    });
    let cleared = store.get(&id).unwrap();
    assert_eq!(cleared.contact_name, "");
    assert_eq!(cleared.contact_email, "");
    assert_eq!(cleared.recipient_name, "");
}

/// One bundle entry for a pre-unification export, with the four contact fields
/// under test and everything else at a harmless default.
fn imported_row(
    id: &str,
    contact_name: &str,
    contact_email: &str,
    recipient_name: &str,
    recipient_email: &str,
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "status": "applied",
        "createdAt": 1000,
        "updatedAt": 1000,
        "jobUrl": "",
        "board": "",
        "company": "Acme",
        "title": "Engineer",
        "candidate": "Jane",
        "answers": [],
        "brief": "",
        "notes": "",
        "comp": "",
        "contactName": contact_name,
        "contactEmail": contact_email,
        "recipientName": recipient_name,
        "recipientEmail": recipient_email
    })
}

#[test]
fn importing_a_pre_unification_bundle_folds_exactly_like_the_migration() {
    // `canonicalize_contact` is the import-time copy of the migration's rule;
    // these are the same cases as `contact_backfill_promotes_the_pair_atomically`
    // driven through the bundle path instead of SQL.
    let (_dir, store) = open_store();
    let bundle = serde_json::json!([
        imported_row("imp-alias-only", "", "", "Rita Recruiter", "rita@acme.com"),
        imported_row(
            "imp-contact-name-only",
            "Cora Contact",
            "",
            "Rita Recruiter",
            "rita@acme.com"
        ),
        imported_row("imp-alias-name-only", "", "", "Rita Recruiter", ""),
        imported_row("imp-whitespace-contact", "  ", " ", "Rita Recruiter", ""),
        // The lockstep case: a blank canonical pair facing an equally blank
        // ALIAS pair. The migration's WHERE requires a non-blank alias, so SQL
        // leaves this row alone — and after the ruling in `contact.rs` so does
        // the import path. Before it, Rust promoted on `canonical_empty` alone
        // and OVERWROTE the stored whitespace with the empty alias.
        imported_row("imp-blank-both", " ", "", "\t", "  "),
    ]);
    assert_eq!(store.import(&bundle).unwrap(), 5);
    let contact_of = |id: &str| {
        let a = store.get(id).unwrap();
        (a.contact_name, a.contact_email)
    };

    // Alias-only → the whole pair is promoted.
    assert_eq!(
        contact_of("imp-alias-only"),
        ("Rita Recruiter".into(), "rita@acme.com".into())
    );
    // …and mirrored back onto the deprecated wire name.
    assert_eq!(
        store.get("imp-alias-only").unwrap().recipient_name,
        "Rita Recruiter"
    );

    // THE FUSION CASE: a half-populated canonical pair must not absorb the alias
    // email, or the mailto: sink would address Rita under Cora's name.
    assert_eq!(
        contact_of("imp-contact-name-only"),
        ("Cora Contact".into(), String::new())
    );
    // The distinct contact is preserved instead of silently dropped — an export
    // mirrors the canonical pair, so this note is its only recoverable home.
    assert_eq!(
        store.get("imp-contact-name-only").unwrap().notes,
        "Apply-by-email: Rita Recruiter <rita@acme.com>"
    );

    // A half-populated ALIAS pair still moves as a unit; the empty half stays empty.
    assert_eq!(
        contact_of("imp-alias-name-only"),
        ("Rita Recruiter".into(), String::new())
    );
    // Whitespace-only canonical counts as empty, same TRIM rule as the migration.
    assert_eq!(
        contact_of("imp-whitespace-contact"),
        ("Rita Recruiter".into(), String::new())
    );
    // LOCKSTEP: nothing to promote → no write at all, byte-for-byte what the
    // migration's `WHERE … AND (TRIM(recipient_name) <> '' OR …)` leaves on disk.
    assert_eq!(
        contact_of("imp-blank-both"),
        (" ".to_string(), String::new()),
        "an empty alias pair must never overwrite the canonical pair"
    );
    // …and no preservation note is invented for a contact that does not exist.
    assert_eq!(store.get("imp-blank-both").unwrap().notes, "");
}

#[test]
fn re_importing_does_not_stack_duplicate_preserved_contacts() {
    let (_dir, store) = open_store();
    let bundle = serde_json::json!([imported_row(
        "imp-repeat",
        "Cora Contact",
        "",
        "Rita Recruiter",
        "rita@acme.com"
    )]);
    store.import(&bundle).unwrap();
    let once = store.get("imp-repeat").unwrap().notes;

    // Re-importing the SAME pre-unification bundle must not append a second copy.
    store.import(&bundle).unwrap();
    assert_eq!(store.get("imp-repeat").unwrap().notes, once);

    // Nor does exporting the now-unified row and importing that back.
    let round_tripped = store.export();
    store.import(&round_tripped).unwrap();
    assert_eq!(store.get("imp-repeat").unwrap().notes, once);
}
