use super::{support::*, *};

// ── Follow-up reminder marker (`add_applications_next_action_notified_at`) ────

#[test]
fn follow_up_candidates_only_carry_rows_with_a_reminder() {
    let (_dir, store) = open_store();
    let with_reminder = track(&store, "Acme", "Engineer");
    let without = track(&store, "Globex", "Designer");
    set_reminder(&store, &with_reminder, Some(5_000));

    let candidates = store.follow_up_candidates();
    let ids: Vec<&str> = candidates.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec![with_reminder.as_str()]);
    assert!(!ids.contains(&without.as_str()));
    let c = candidates.first().expect("one candidate");
    assert_eq!(c.next_action_at, Some(5_000));
    assert_eq!(c.notified_at, None, "a fresh reminder starts un-notified");
    assert_eq!(c.company, "Acme");
    assert_eq!(c.title, "Engineer");
    assert_eq!(c.status, ApplicationStatus::Applied);
}

#[test]
fn the_notified_marker_survives_unrelated_edits_and_clears_on_reschedule() {
    let (_dir, store) = open_store();
    let id = track(&store, "Acme", "Engineer");
    let marker = || notified_at(&store);

    set_reminder(&store, &id, Some(5_000));
    assert!(
        store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
        "stamping the due date the sweep read must match a row"
    );
    assert_eq!(marker(), Some(9_000), "the sweep's stamp persists");

    // An unrelated patch rewrites every column — the marker must NOT be lost,
    // or a still-overdue reminder would re-notify on the very next sweep.
    edit(&store, &id, |p| p.notes = Some("called them".into()));
    assert_eq!(marker(), Some(9_000), "an unrelated edit must not clear it");

    // Patching next_action_at to the SAME value is not a reschedule.
    set_reminder(&store, &id, Some(5_000));
    assert_eq!(
        marker(),
        Some(9_000),
        "an unchanged due date is not a reschedule"
    );

    // Moving the due date IS — the new date must be announceable once.
    set_reminder(&store, &id, Some(7_000));
    assert_eq!(marker(), None, "rescheduling clears the marker");

    // Clearing the reminder entirely also clears the marker, so re-setting the
    // same date later still notifies.
    store.mark_next_action_notified(&id, 7_000, 9_500).unwrap();
    set_reminder(&store, &id, None);
    assert!(
        store.follow_up_candidates().is_empty(),
        "a cleared reminder leaves no candidate"
    );
    set_reminder(&store, &id, Some(7_000));
    assert_eq!(marker(), None, "clearing then re-setting starts fresh");
}

#[test]
fn stamping_a_stale_due_date_matches_nothing_and_leaves_the_new_one_notifiable() {
    // The sweep's read → stamp window: the user reschedules in between. An
    // unconditional stamp would mark the row notified for a due date the sweep
    // never evaluated, silencing the NEW reminder forever.
    let (_dir, store) = open_store();
    let id = track(&store, "Acme", "Engineer");

    set_reminder(&store, &id, Some(5_000)); // the sweep reads due = 5_000 …
    set_reminder(&store, &id, Some(7_000)); // … the user reschedules before the stamp lands

    assert!(
        !store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
        "a stamp for the OLD due date must match no row"
    );
    let candidate = store.follow_up_candidates();
    assert_eq!(
        candidate.first().and_then(|c| c.notified_at),
        None,
        "the new due date must stay notifiable"
    );

    // A missing id is the same no-op, not an error.
    assert!(!store
        .mark_next_action_notified("does-not-exist", 7_000, 9_000)
        .unwrap());
}

#[test]
fn a_status_change_to_terminal_between_the_read_and_the_stamp_blocks_the_claim() {
    // The sweep reads candidates under one lock and stamps under another. In the
    // window between, the user can close the pursuit — and a Rust-side
    // `is_terminal` re-check would have that exact same window. Only the
    // predicate INSIDE the stamping UPDATE closes it.
    let (_dir, store) = open_store();
    let id = track(&store, "Acme", "Engineer");
    set_reminder(&store, &id, Some(5_000));

    // 1. The sweep reads: due, un-notified, not terminal → it would notify.
    let read = store
        .follow_up_candidates()
        .into_iter()
        .next()
        .expect("one candidate");
    assert!(
        !read.status.is_terminal(),
        "baseline: notifiable at read time"
    );
    assert_eq!(read.notified_at, None);

    // 2. The user rejects the application, inside the window.
    store
        .set_status(&id, ApplicationStatus::Rejected, "")
        .unwrap();

    // 3. The claim must lose.
    assert!(
        !store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
        "a pursuit that went terminal after the read must not be claimed"
    );
    assert_eq!(
        notified_at(&store),
        None,
        "a refused claim leaves the row unstamped, so reviving it still reminds"
    );

    // Reviving it makes the very same claim succeed — the ONLY thing that
    // changed is the status, which is what pins the predicate.
    store
        .set_status(&id, ApplicationStatus::Interviewing, "")
        .unwrap();
    assert!(
        store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
        "a revived pursuit is claimable again"
    );
}

#[test]
fn every_terminal_status_blocks_the_claim_and_ghosted_does_not() {
    // The predicate must track `is_terminal` exactly — `ghosted` is deliberately
    // soft-terminal (a ghosted pursuit can revive), so it still reminds.
    let (_dir, store) = open_store();
    for status in ApplicationStatus::ALL {
        let id = track(&store, "Acme", "Engineer");
        set_reminder(&store, &id, Some(5_000));
        store.set_status(&id, *status, "").unwrap();
        assert_eq!(
            store.mark_next_action_notified(&id, 5_000, 9_000).unwrap(),
            !status.is_terminal(),
            "{status:?}: claimability must follow is_terminal()"
        );
    }
}

#[test]
fn a_broken_follow_up_query_degrades_to_an_empty_sweep_instead_of_panicking() {
    // The reminder sweep is this query's only consumer. If a schema change ever
    // renames or drops a column it reads, an `unwrap` would take the scheduler
    // task down and a bare `.ok()` would silently kill EVERY reminder for the
    // rest of the process. It must return empty and log. Forced with the same
    // second-raw-connection trick as
    // `transition_status_if_rolls_back_status_when_event_insert_fails`.
    let (dir, store) = open_store();
    let id = track(&store, "Acme", "Engineer");
    set_reminder(&store, &id, Some(5_000));
    assert_eq!(
        store.follow_up_candidates().len(),
        1,
        "baseline: the row IS a candidate before the table goes away"
    );

    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute("DROP TABLE applications", []).unwrap();
    }

    // SQLite compiles a statement against the CONNECTION's cached schema, and
    // only reloads it when a statement actually steps. So the same broken schema
    // surfaces through two different arms, in this order — both are exercised
    // here, and both must degrade to an empty sweep rather than a panic.
    //
    // Arm 1 — the store has not noticed yet: `prepare` succeeds against the
    // stale schema and the failure arrives while iterating (`SQLITE_SCHEMA` →
    // re-prepare → "no such table"), i.e. the per-row arm.
    assert!(
        store.follow_up_candidates().is_empty(),
        "a mid-iteration schema failure must yield an empty sweep, never a panic"
    );

    // Arm 2 — that step reloaded the schema, so `prepare` itself now fails. This
    // is the arm a renamed/dropped COLUMN hits on every subsequent sweep, i.e.
    // the one that would silence reminders forever if it were swallowed.
    assert!(
        store.follow_up_candidates().is_empty(),
        "a prepare failure must yield an empty sweep, never a panic"
    );
    // Verified empirically, not assumed: replacing the `match conn.prepare(…)`
    // guard with `.expect(…)` panics on THIS second call with
    // `no such table: applications` (and passes on the first).

    // (The remaining arm — `query_map` itself returning `Err` — is unreachable
    // for this statement: `query_map` only fails while binding, and the
    // statement takes zero parameters. It stays a defensive `match` arm rather
    // than an `unwrap` so a future parameterised rewrite cannot become a panic.)
}

#[test]
fn a_follow_up_row_that_fails_to_decode_is_skipped_and_the_healthy_ones_still_sweep() {
    // The per-row arm: one corrupted row must not cost the user every OTHER
    // reminder. A `next_action_at` holding text SQLite cannot coerce to an
    // integer is what a hand-edited or partially-restored db looks like — it
    // passes the `IS NOT NULL` filter, then fails `row.get::<_, i64>`.
    let (dir, store) = open_store();
    let good = track(&store, "Acme", "Engineer");
    set_reminder(&store, &good, Some(5_000));

    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute(
            "INSERT INTO applications (id, status, created_at, updated_at, next_action_at)
             VALUES ('bad-row', 'applied', 1000, 1000, 'not-a-timestamp')",
            [],
        )
        .unwrap();
        // Fixture sanity: INTEGER affinity keeps a non-numeric string as TEXT,
        // which is what makes the row undecodable in the first place.
        let kind: String = conn
            .query_row(
                "SELECT typeof(next_action_at) FROM applications WHERE id = 'bad-row'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kind, "text", "fixture must store a non-integer value");
    }

    let ids: Vec<String> = store
        .follow_up_candidates()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(
        ids,
        vec![good],
        "the undecodable row is skipped; the healthy reminder still sweeps"
    );
}
