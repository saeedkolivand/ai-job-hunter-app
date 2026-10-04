use super::{support::*, *};

// ── v2 slice 2: status_events source/confirmed, accept/reject ───────────────

/// Position-indexed migrations: MUST always append, never insert — an
/// insertion would shift every later index and silently re-run (or skip)
/// migrations on an already-migrated database. Pins both the total count AND
/// the exact name at the new index, plus the immediately-preceding entry, so
/// an insertion anywhere in the list (not just at the very end) fails this.
#[test]
fn status_events_source_confirmed_migration_is_appended_not_inserted() {
    assert_eq!(
        migrations::MIGRATIONS.len(),
        10,
        "a new migration must be APPENDED — this pins the total count"
    );
    assert_eq!(
        migrations::MIGRATIONS[9].name,
        "add_status_events_source_confirmed"
    );
    assert_eq!(
        migrations::MIGRATIONS[8].name,
        "add_backfill_state",
        "the entry immediately before the new one must be unchanged — proves \
         nothing was inserted ahead of it"
    );
}

/// The newest status event's `event_id` for `id`.
fn last_event_id(store: &ApplicationStore, id: &str) -> i64 {
    store.events(id).into_iter().last().unwrap().event_id
}

/// Two coexisting provisional rows on one Application — what two ordinary email ticks
/// produce: a confirmation email writes `Saved -> Applied` (T1, unconfirmed), then a
/// LATER rejection email sees the still-live `Applied` and writes `Applied -> Rejected`
/// (T2, unconfirmed). Neither has been reviewed. Returns `(id, t1_event_id, t2_event_id)`.
fn two_pending_email_rows(store: &ApplicationStore) -> (String, i64, i64) {
    let id = track(store, "C", "T"); // starts `applied`
                                     // Start from `saved` so T1 is a real forward advance, matching the
                                     // shape a live confirmation-then-rejection pair actually produces.
    store.set_status(&id, ApplicationStatus::Saved, "").unwrap();

    email_write(
        store,
        &id,
        ApplicationStatus::Saved,
        ApplicationStatus::Applied,
        None,
    );
    let t1_event_id = last_event_id(store, &id);
    email_write(
        store,
        &id,
        ApplicationStatus::Applied,
        ApplicationStatus::Rejected,
        None,
    );
    let t2_event_id = last_event_id(store, &id);
    (id, t1_event_id, t2_event_id)
}

#[test]
fn accept_status_event_sets_confirmed_without_touching_status() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T"); // starts `applied`

    let wrote = email_write(
        &store,
        &id,
        ApplicationStatus::Applied,
        ApplicationStatus::Interviewing,
        Some("email-derived (unconfirmed)"),
    );
    assert!(wrote);
    let pending = store.events(&id).into_iter().last().unwrap();
    assert!(!pending.confirmed);

    let accepted = store.accept_status_event(&id, pending.event_id).unwrap();
    assert!(accepted);

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Interviewing,
        "accept must never change the status itself"
    );
    let last = store.events(&id).into_iter().last().unwrap();
    assert!(last.confirmed, "accept must set the confirmed flag");
    assert_eq!(
        last.source, EVENT_SOURCE_EMAIL,
        "accept must not touch the source"
    );
}

/// HIGH-1 deciding experiment: two provisional rows coexist on the ordinary
/// happy path — a confirmation email writes `Saved -> Applied` (T1,
/// unconfirmed), then a LATER rejection email sees the still-live `Applied`
/// and writes `Applied -> Rejected` (T2, unconfirmed). `next_status` never
/// consults `confirmed` for a live current status, and nothing settles the
/// older row when a newer one lands — this is not a contrived setup, it is
/// what two ordinary ticks produce. A user clicking Accept on the OLDER
/// (T1, "yes, I applied") row must confirm EXACTLY that row — never
/// whichever row happens to be newest.
#[test]
fn accept_targets_the_specific_row_requested_never_whichever_landed_most_recently() {
    let (_dir, store) = open_store();
    let (id, t1_event_id, t2_event_id) = two_pending_email_rows(&store);
    assert_ne!(
        t1_event_id, t2_event_id,
        "fixture requires two distinct rows"
    );

    // The user clicks Accept on the OLDER (T1) row specifically.
    let accepted = store.accept_status_event(&id, t1_event_id).unwrap();
    assert!(
        accepted,
        "the specifically-requested T1 row must be acceptable"
    );

    let events = store.events(&id);
    let t1 = events.iter().find(|e| e.event_id == t1_event_id).unwrap();
    let t2 = events.iter().find(|e| e.event_id == t2_event_id).unwrap();
    assert!(
        t1.confirmed,
        "T1 — the row the user actually clicked — must be confirmed"
    );
    assert!(
        !t2.confirmed,
        "T2 must be UNTOUCHED — the user never reviewed it. A pre-fix \
         recency-based resolution would confirm T2 (the newest pending row) \
         here instead, silently ratifying a rejection the user never saw"
    );
}

/// Mirror of the accept test above for reject — "recoverable only by
/// clicking again" per the fix-forward task, but still a real
/// wrong-row-touched bug until fixed: rejecting the OLDER row must not
/// revert/dismiss the NEWER one.
#[test]
fn reject_targets_the_specific_row_requested_never_whichever_landed_most_recently() {
    let (_dir, store) = open_store();
    let (id, t1_event_id, t2_event_id) = two_pending_email_rows(&store);

    // The user rejects the OLDER (T1) row specifically — "no, I never
    // actually applied". The CAS itself loses (the live status has already
    // moved on to `rejected` via T2, no longer `applied` — T1's own
    // `from`/`to` snapshot is stale, so nothing is reverted); T1 is still
    // marked reviewed/dismissed regardless — same "does not clobber"
    // contract as a status the user changed by hand.
    let reverted = store.reject_status_event(&id, t1_event_id).unwrap();
    assert!(
        !reverted,
        "the CAS must lose — the live status moved past `applied` via T2"
    );

    let events = store.events(&id);
    let t1 = events.iter().find(|e| e.event_id == t1_event_id).unwrap();
    let t2 = events.iter().find(|e| e.event_id == t2_event_id).unwrap();
    assert!(t1.confirmed, "T1 must be marked reviewed (dismissed)");
    assert!(
        !t2.confirmed,
        "T2 must be UNTOUCHED — a wrong-row resolution would instead revert/dismiss \
         T2 (the newest pending row) when the user meant to act on T1"
    );
}

#[test]
fn accept_status_event_is_a_noop_when_nothing_is_unconfirmed() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T");
    // A REAL row id (the user-sourced seed event) that is simply not an
    // unconfirmed email row -- proves the `source`/`confirmed` guard, not
    // just "an unknown id is ignored".
    let seed_event_id = last_event_id(&store, &id);
    assert!(!store.accept_status_event(&id, seed_event_id).unwrap());
}

/// The reversal event must be APPENDED, with the trail still showing the
/// original email-derived transition. Asserts the row COUNT and the
/// SEQUENCE, not just the final status.
#[test]
fn reject_status_event_reverts_status_and_appends_a_reversal_event_keeping_the_original() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T"); // starts `applied`
    store
        .set_status(&id, ApplicationStatus::Interviewing, "")
        .unwrap();
    let events_before = store.events(&id).len();

    let wrote = email_write(
        &store,
        &id,
        ApplicationStatus::Interviewing,
        ApplicationStatus::Rejected,
        Some("email-derived (unconfirmed)"),
    );
    assert!(wrote);
    assert_eq!(store.events(&id).len(), events_before + 1);
    let pending_event_id = last_event_id(&store, &id);

    let reverted = store.reject_status_event(&id, pending_event_id).unwrap();
    assert!(reverted);

    let events_after = store.events(&id);
    assert_eq!(
        events_after.len(),
        events_before + 2,
        "reject must APPEND a reversal event, never replace the original"
    );

    // Second-to-last event: the ORIGINAL email-derived transition, unedited
    // (from/to/source unchanged) — only its `confirmed` flag moved.
    let original = &events_after[events_after.len() - 2];
    assert_eq!(original.from_status, "interviewing");
    assert_eq!(original.to_status, "rejected");
    assert_eq!(original.source, EVENT_SOURCE_EMAIL);
    assert!(
        original.confirmed,
        "the original row must be marked reviewed, not deleted or hidden"
    );

    // The reversal is the NEW last event.
    let reversal = events_after.last().unwrap();
    assert_eq!(reversal.from_status, "rejected");
    assert_eq!(reversal.to_status, "interviewing");
    assert_eq!(reversal.source, status_events::EVENT_SOURCE_EMAIL_REJECT);
    assert!(reversal.confirmed);

    assert_eq!(
        store.get(&id).unwrap().status,
        ApplicationStatus::Interviewing,
        "status must be reverted to the value BEFORE the email-derived write"
    );
}

/// A reject on an application whose status the user changed by hand in the
/// meantime must NOT clobber it — anchored to the absolute expected final
/// status.
#[test]
fn reject_status_event_does_not_clobber_a_status_the_user_changed_by_hand() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T"); // starts `applied`
    store
        .set_status(&id, ApplicationStatus::Interviewing, "")
        .unwrap();

    email_write(
        &store,
        &id,
        ApplicationStatus::Interviewing,
        ApplicationStatus::Rejected,
        Some("email-derived (unconfirmed)"),
    );
    let pending_event_id = last_event_id(&store, &id);

    // The user changes the status BY HAND in the meantime. `set_status`
    // accepts any transition (including out of a terminal status — a
    // separate, known, pre-existing defect this test does not fix), which
    // is exactly what makes it the right tool to simulate an out-of-band
    // manual change here.
    store
        .set_status(&id, ApplicationStatus::Accepted, "user accepted the offer")
        .unwrap();

    let reverted = store.reject_status_event(&id, pending_event_id).unwrap();
    assert!(
        !reverted,
        "the CAS must lose — the status is no longer what the email set it to"
    );

    assert_eq!(
        store.get(&id).unwrap().status,
        ApplicationStatus::Accepted,
        "a reject must NEVER clobber a status the user changed by hand"
    );

    // The provisional row is dismissed (marked reviewed), not reverted.
    let email_events: Vec<_> = store
        .events(&id)
        .into_iter()
        .filter(|e| e.source == EVENT_SOURCE_EMAIL)
        .collect();
    assert_eq!(email_events.len(), 1);
    assert!(
        email_events[0].confirmed,
        "the dismissed provisional row must still be marked reviewed"
    );
}

#[test]
fn reject_status_event_is_a_noop_when_nothing_is_unconfirmed() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T");
    let seed_event_id = last_event_id(&store, &id);
    assert!(!store.reject_status_event(&id, seed_event_id).unwrap());
}

/// A second, later email must not re-apply a target status the user already
/// rejected — the store-level half of that guarantee. `was_transition_rejected`
/// is keyed on the TARGET alone (not the `(from, to)` pair — see its doc),
/// so this also proves the detour a pair-keyed guard would have missed: a
/// completely different `from` status reaching the SAME disputed target is
/// still blocked.
#[test]
fn was_transition_rejected_blocks_any_future_write_landing_at_the_rejected_target_via_any_path() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T"); // starts `applied`

    assert!(!store.was_transition_rejected(&id, ApplicationStatus::Interviewing));

    email_write(
        &store,
        &id,
        ApplicationStatus::Applied,
        ApplicationStatus::Interviewing,
        None,
    );
    assert!(
        !store.was_transition_rejected(&id, ApplicationStatus::Interviewing),
        "not rejected yet — only written"
    );

    let event_id = last_event_id(&store, &id);
    store.reject_status_event(&id, event_id).unwrap();
    assert!(
        store.was_transition_rejected(&id, ApplicationStatus::Interviewing),
        "must be true for the target that was rejected"
    );
    assert!(
        !store.was_transition_rejected(&id, ApplicationStatus::Offer),
        "a different target must not be flagged"
    );

    // The detour: the reject reverted status back to `Applied`. Move it BY
    // HAND to a different live status (simulating an intervening real
    // Screening stage), then confirm the SAME disputed target is still
    // blocked even via a pair (`Screening -> Interviewing`) that was never
    // itself rejected — the exact detour a pair-keyed guard let through.
    store
        .set_status(&id, ApplicationStatus::Screening, "")
        .unwrap();
    assert!(
        store.was_transition_rejected(&id, ApplicationStatus::Interviewing),
        "the guard must still block `Interviewing` as a target reached via a          completely different pair — this is the detour the fix closes"
    );
}

/// LOW fix: `was_transition_rejected` used to fold a genuine READ error
/// into "no rejection on record" (`false`), un-gating a target the user
/// explicitly disputed. Forces a genuine `rusqlite::Error` (not a
/// stand-in for "no matching row") by dropping the column the query's
/// `WHERE` clause selects on, then asserts the fn fails CLOSED (`true`,
/// "treat as rejected") rather than open.
#[test]
fn was_transition_rejected_fails_closed_on_a_genuine_read_error() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T");

    store
        .conn
        .lock()
        .execute_batch("ALTER TABLE status_events DROP COLUMN from_status;")
        .expect("drop column to force a genuine read error");

    assert!(
        store.was_transition_rejected(&id, ApplicationStatus::Interviewing),
        "a read error must fail CLOSED (true — treat as rejected), never \
         un-gate a target the user may have actually disputed"
    );
}
