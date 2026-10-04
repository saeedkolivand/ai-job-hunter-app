use super::{support::*, *};

// ── Contact unification (migration `unify_application_contact`) ───────────────
//
// `contact_name`/`contact_email` became THE single primary contact per
// application; `recipient_name`/`recipient_email` are deprecated aliases. The
// migration promotes an alias-only value onto the canonical pair and leaves the
// deprecated COLUMNS untouched (additive-only, never destructive).

/// Seed `applications.db` at `user_version = 6` — the pre-unification schema
/// (recipient + salary columns present) — with one row per interesting
/// contact/recipient population combination.
///
/// The four fields are independently empty-or-not (16 states); these rows cover
/// every state the promotion rule can treat differently — both pairs fully
/// populated / fully empty, EITHER pair half-populated (the cases that
/// distinguish a pair-atomic promotion from a per-column one), each flavour of
/// whitespace-only canonical pair, and an identical alias pair. Returns the ids
/// in declaration order.
fn seed_pre_unification_db(dir: &Path) -> [&'static str; 12] {
    let rows: [(&'static str, &str, &str, &str, &str); 12] = [
        (
            "app-recipient-only",
            "",
            "",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        ("app-contact-only", "Cora Contact", "cora@acme.com", "", ""),
        (
            "app-both",
            "Cora Contact",
            "cora@acme.com",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        ("app-neither", "", "", "", ""),
        // Canonical HALF-populated + a full alias pair. The per-column rule
        // fused the two people here; a pair-atomic one must not promote at all.
        (
            "app-contact-name-only",
            "Cora Contact",
            "",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        (
            "app-contact-email-only",
            "",
            "cora@acme.com",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        // Canonical empty + an alias pair that is itself half-populated: the
        // promotion must move BOTH columns, leaving the empty side empty.
        ("app-recipient-name-only", "", "", "Rita Recruiter", ""),
        ("app-recipient-email-only", "", "", "", "rita@acme.com"),
        // Whitespace-only canonical pairs — reachable from pre-trim builds. Each
        // flavour is seeded separately because SQLite's BARE `TRIM(x)` strips
        // only U+0020: a TAB or an NBSP (endemic in text copied out of scraped
        // HTML) read as non-empty in SQL while `str::trim` calls them empty, so
        // the same row folded one way in place and the other way through a
        // restored bundle until the migration passed an explicit charset.
        (
            "app-space-contact",
            "   ",
            "  ",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        (
            "app-tab-contact",
            "\t",
            "\t\t",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        (
            "app-nbsp-contact",
            "\u{A0}",
            "\u{A0} \u{A0}",
            "Rita Recruiter",
            "rita@acme.com",
        ),
        // Alias pair IDENTICAL to the canonical one — nothing is being dropped,
        // so the `<>` distinctness guard must suppress the preserved note.
        (
            "app-identical-pair",
            "Rita Recruiter",
            "rita@acme.com",
            "Rita Recruiter",
            "rita@acme.com",
        ),
    ];
    let conn = legacy_db(dir, 6);
    for (id, contact_name, contact_email, recipient_name, recipient_email) in rows {
        conn.execute(
            "INSERT INTO applications
             (id, status, created_at, updated_at, contact_name, contact_email,
              recipient_name, recipient_email)
             VALUES (?1, 'applied', 1000, 1000, ?2, ?3, ?4, ?5)",
            params![
                id,
                contact_name,
                contact_email,
                recipient_name,
                recipient_email
            ],
        )
        .unwrap();
    }
    rows.map(|row| row.0)
}

/// Read a raw column straight from SQLite, bypassing the store's projection —
/// used to assert the DEPRECATED columns were left alone by the migration.
fn raw_column(dir: &Path, id: &str, column: &str) -> String {
    let conn = Connection::open(dir.join("applications.db")).unwrap();
    conn.query_row(
        &format!("SELECT {column} FROM applications WHERE id = ?1"),
        params![id],
        |r| r.get::<_, String>(0),
    )
    .unwrap()
}

#[test]
fn contact_backfill_promotes_the_pair_atomically() {
    let dir = TempDir::new().unwrap();
    let [recipient_only, contact_only, both, neither, contact_name_only, contact_email_only, recipient_name_only, recipient_email_only, space_contact, tab_contact, nbsp_contact, identical_pair] =
        seed_pre_unification_db(dir.path());

    // Opening runs migration 7 (unify) + 8 (reminder marker).
    let store = ApplicationStore::open(dir.path()).unwrap();
    let contact_of = |id: &str| {
        let a = store.get(id).unwrap();
        (a.contact_name, a.contact_email)
    };

    // 1. recipient-only → the whole pair is promoted.
    assert_eq!(
        contact_of(recipient_only),
        ("Rita Recruiter".into(), "rita@acme.com".into())
    );

    // 2. contact-only → untouched.
    assert_eq!(
        contact_of(contact_only),
        ("Cora Contact".into(), "cora@acme.com".into())
    );

    // 3. both populated → the canonical pair wins; the alias is not merged in.
    assert_eq!(
        contact_of(both),
        ("Cora Contact".into(), "cora@acme.com".into())
    );

    // 4. neither → still empty (no phantom value invented).
    assert_eq!(contact_of(neither), (String::new(), String::new()));

    // 5-6. THE IDENTITY-FUSION CASES. The canonical pair is half-populated, so
    // it already belongs to someone (Cora). A per-column promotion would fill
    // the empty half from the alias row and hand back "Cora Contact
    // <rita@acme.com>" — a mailto: addressed to Rita under Cora's name, or
    // Cora's address under Rita's name. Pair-atomic: promote nothing.
    assert_eq!(
        contact_of(contact_name_only),
        ("Cora Contact".into(), String::new()),
        "a half-populated canonical pair must never absorb the alias EMAIL"
    );
    assert_eq!(
        contact_of(contact_email_only),
        (String::new(), "cora@acme.com".into()),
        "a half-populated canonical pair must never absorb the alias NAME"
    );

    // 7-8. Canonical empty + a half-populated alias → both columns move, and the
    // empty half stays empty (never back-filled from anywhere else).
    assert_eq!(
        contact_of(recipient_name_only),
        ("Rita Recruiter".into(), String::new())
    );
    assert_eq!(
        contact_of(recipient_email_only),
        (String::new(), "rita@acme.com".into())
    );

    // 9-11. A whitespace-only canonical pair counts as EMPTY and is promoted like
    // a truly empty one — for every flavour of whitespace, not just U+0020.
    // SQLite's bare `TRIM(x)` strips only spaces, so the TAB and NBSP rows failed
    // to promote here while `str::trim` (the import path) folded them: the same
    // row ended up with a different contact depending on whether it migrated in
    // place or came back through a restored bundle.
    for (id, flavour) in [
        (space_contact, "spaces"),
        (tab_contact, "tabs"),
        (nbsp_contact, "NBSP"),
    ] {
        assert_eq!(
            contact_of(id),
            ("Rita Recruiter".into(), "rita@acme.com".into()),
            "a canonical pair holding only {flavour} must count as empty and promote"
        );
    }

    // 12. Alias pair IDENTICAL to the canonical one: nothing is being dropped, so
    // the `<>` distinctness guard must suppress the preserved note (asserted
    // below) while the contact itself stays exactly as it was.
    assert_eq!(
        contact_of(identical_pair),
        ("Rita Recruiter".into(), "rita@acme.com".into())
    );

    // Every response mirrors the canonical pair onto the deprecated wire names,
    // so a renderer still reading `recipientName` sees the unified contact.
    for id in [
        recipient_only,
        contact_only,
        both,
        neither,
        contact_name_only,
        contact_email_only,
        recipient_name_only,
        recipient_email_only,
        space_contact,
        tab_contact,
        nbsp_contact,
        identical_pair,
    ] {
        let app = store.get(id).unwrap();
        assert_eq!(
            app.recipient_name, app.contact_name,
            "{id}: recipientName must mirror the canonical contactName"
        );
        assert_eq!(
            app.recipient_email, app.contact_email,
            "{id}: recipientEmail must mirror the canonical contactEmail"
        );
    }

    // The alias pair that was NOT promoted (it belongs to a second, distinct
    // person) is preserved into notes — the store stops reading the deprecated
    // columns and an export mirrors the canonical pair, so this is its only
    // recoverable home.
    for id in [both, contact_name_only, contact_email_only] {
        assert!(
            store
                .get(id)
                .unwrap()
                .notes
                .contains("Apply-by-email: Rita Recruiter <rita@acme.com>"),
            "{id}: a dropped distinct apply-by-email contact must survive in notes"
        );
    }
    // Nothing was dropped for these, so no note may be appended: the promoted
    // rows (their alias pair BECAME the contact), the empty one, and — the case
    // the `<>` distinctness guard exists for — the row whose alias pair was
    // already identical to its canonical pair.
    for id in [
        recipient_only,
        space_contact,
        tab_contact,
        nbsp_contact,
        neither,
        identical_pair,
    ] {
        assert!(
            !store.get(id).unwrap().notes.contains("Apply-by-email:"),
            "{id}: nothing was dropped, so no note may be appended"
        );
    }

    // Non-destructive: the DEPRECATED columns keep their original values on disk
    // (the migration only ever writes the canonical pair).
    assert_eq!(
        raw_column(dir.path(), both, "recipient_name"),
        "Rita Recruiter"
    );
    assert_eq!(
        raw_column(dir.path(), both, "recipient_email"),
        "rita@acme.com"
    );
    assert_eq!(
        raw_column(dir.path(), recipient_only, "recipient_name"),
        "Rita Recruiter"
    );
}

#[test]
fn contact_backfill_appends_the_preserved_note_after_existing_text() {
    let dir = TempDir::new().unwrap();
    seed_pre_unification_db(dir.path());
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute(
            "UPDATE applications SET notes = 'call back Tuesday' WHERE id = 'app-both'",
            [],
        )
        .unwrap();
    }
    let store = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(
        store.get("app-both").unwrap().notes,
        "call back Tuesday\n\nApply-by-email: Rita Recruiter <rita@acme.com>",
        "the user's own note must be kept, with the preserved contact appended"
    );
}

#[test]
fn contact_backfill_sql_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let ids = seed_pre_unification_db(dir.path());
    // Snapshot `notes` too: the preservation statement APPENDS, so a replay that
    // is not guarded would stack duplicate "Apply-by-email:" lines.
    let snapshot = |store: &ApplicationStore| -> Vec<(String, String, String)> {
        ids.iter()
            .map(|id| {
                let a = store.get(id).unwrap();
                (a.contact_name, a.contact_email, a.notes)
            })
            .collect()
    };
    let store = ApplicationStore::open(dir.path()).unwrap();
    let before = snapshot(&store);
    drop(store);

    // Re-run the migration BODY itself (not just `run_migrations`, which
    // short-circuits on user_version) — the SQL must be safe to replay.
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        let unify = &super::migrations::MIGRATIONS[6];
        assert_eq!(
            unify.name, "unify_application_contact",
            "migration order is pinned — entries are append-only"
        );
        (unify.up)(&conn).unwrap();
        (unify.up)(&conn).unwrap();
    }

    let store = ApplicationStore::open(dir.path()).unwrap();
    assert_eq!(
        before,
        snapshot(&store),
        "replaying the backfill must change nothing"
    );
}
