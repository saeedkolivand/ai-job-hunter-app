//! A refused email suggestion stays ONE sync record, and an older payload of it
//! never brings it back as pending.

use super::{support::*, *};
use crate::data_store::{SyncRecord, SyncSection};

fn email_record(store: &ApplicationStore) -> SyncRecord {
    ApplicationStatusEvents(store)
        .sync_records()
        .unwrap()
        .into_iter()
        .find(|r| r.payload["source"] == "email")
        .expect("the email event has a record")
}

#[test]
fn a_dismissal_keeps_the_record_id_and_an_older_payload_does_not_resurrect_it() {
    let (_dir, store) = open_store();
    let id = track(&store, "Acme", "Analyst");
    assert!(email_write(
        &store,
        &id,
        ApplicationStatus::Applied,
        ApplicationStatus::Screening,
        None
    ));
    let pending = email_record(&store);
    assert_eq!(pending.payload["confirmed"], false);

    // The user moves the status on by hand, then dismisses the suggestion.
    store
        .set_status(&id, ApplicationStatus::Rejected, "")
        .unwrap();
    let event_id = store
        .events(&id)
        .iter()
        .find(|e| e.is_unconfirmed_email_write())
        .unwrap()
        .event_id;
    assert!(!store.reject_status_event(&id, event_id).unwrap());

    let dismissed = email_record(&store);
    assert_eq!(dismissed.id, pending.id, "updates in place, no new record");
    assert_eq!(dismissed.payload["dismissed"], true);

    // The peer still holds the original pending payload (no `dismissed` field).
    let section = ApplicationStatusEvents(&store);
    section
        .upsert_record(&pending.id, &pending.payload)
        .unwrap();

    let after = store
        .events(&id)
        .into_iter()
        .filter(|e| e.source == EVENT_SOURCE_EMAIL)
        .collect::<Vec<_>>();
    assert_eq!(after.len(), 1);
    assert!(after[0].dismissed && after[0].confirmed, "stays dismissed");
    assert!(!after[0].is_unconfirmed_email_write());
}

#[test]
fn a_peers_dismissal_reaches_a_pending_local_row() {
    let (_dir, store) = open_store();
    let id = track(&store, "Acme", "Analyst");
    email_write(
        &store,
        &id,
        ApplicationStatus::Applied,
        ApplicationStatus::Screening,
        None,
    );
    let record = email_record(&store);
    let mut payload = record.payload.clone();
    payload["confirmed"] = true.into();
    payload["dismissed"] = true.into();

    ApplicationStatusEvents(&store)
        .upsert_record(&record.id, &payload)
        .unwrap();

    let event = store.events(&id).into_iter().last().unwrap();
    assert!(event.dismissed && event.confirmed);
}
