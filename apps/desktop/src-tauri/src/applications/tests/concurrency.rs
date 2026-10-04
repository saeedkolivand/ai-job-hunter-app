use std::sync::Arc;

use super::{support::*, *};

/// A thread appending `iters` individually-traceable answers (`merge-question-{i}`) to
/// `id` through the extension's append-only `merge_answers` path.
fn spawn_answer_merger(
    store: &Arc<ApplicationStore>,
    id: &str,
    iters: usize,
) -> std::thread::JoinHandle<()> {
    let store = store.clone();
    let id = id.to_string();
    std::thread::spawn(move || {
        for i in 0..iters {
            store
                .merge_answers(
                    &id,
                    vec![ans(
                        format!("merge-question-{i}"),
                        format!("merge-answer-{i}"),
                    )],
                )
                .unwrap();
        }
    })
}

/// Regression proxy for the upsert/`merge_answers` TOCTOU fix:
/// `upsert_internal` used to look up the existing row via the self-locking
/// `find_by_job_url` (its own lock acquired and released BEFORE the write
/// transaction re-acquired the lock), leaving a gap where a concurrent
/// `merge_answers` commit could be silently overwritten by the upsert's
/// stale pre-gap snapshot. The fix folds both into one lock/transaction via
/// `row_by_job_url_conn`.
///
/// A deterministic reproduction of the OLD race isn't feasible here: its
/// window was the interval between two `Mutex` acquisitions inside a single
/// call, on the order of nanoseconds, and hitting it reliably would need a
/// test-only pause hook inside `upsert_internal` — production-code scope
/// creep beyond this fix. As an honest proxy, this test instead hammers the
/// SAME Application from two real threads — one repeatedly appending via
/// `merge_answers`, the other repeatedly upserting via `upsert_for_origin` —
/// each iteration contributing one distinct, individually-traceable
/// question, and asserts every single one survives. With the fix, the
/// lookup+write critical section is atomic under the shared lock, so no
/// interleaving can lose an update; this test would be flaky (and could
/// fail) against the old two-lock structure under real contention.
#[test]
fn upsert_and_merge_answers_race_never_loses_an_update() {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(ApplicationStore::open(dir.path()).unwrap());
    let url = "https://acme.com/job/race";
    let id = saved(&store, url);

    const ITERS: usize = 40;

    let merge_thread = spawn_answer_merger(&store, &id, ITERS);

    let upsert_store = store.clone();
    let upsert_url = url.to_string();
    let upsert_thread = std::thread::spawn(move || {
        for i in 0..ITERS {
            let mut m = meta("Acme", "Engineer");
            m.answers = vec![ans(
                format!("upsert-question-{i}"),
                format!("upsert-answer-{i}"),
            )];
            upsert(
                &upsert_store,
                &upsert_url,
                "linkedin",
                &m,
                ApplicationOrigin::Generate,
            );
        }
    });

    merge_thread.join().unwrap();
    upsert_thread.join().unwrap();

    let app = store.get(&id).unwrap();
    for i in 0..ITERS {
        assert!(
            app.answers
                .iter()
                .any(|a| a.question == format!("merge-question-{i}")),
            "merge_answers entry {i} was lost to a concurrent upsert"
        );
        assert!(
            app.answers
                .iter()
                .any(|a| a.question == format!("upsert-question-{i}")),
            "upsert entry {i} was lost to a concurrent merge_answers"
        );
    }
    assert_eq!(
        app.answers.len(),
        ITERS * 2,
        "no answer from either concurrent writer may be dropped"
    );
}

/// `set_status` read the row through a lock it then RELEASED before opening the
/// write transaction, so a concurrent transition could land in the gap and the
/// `status_events` row it appended recorded a `from_status` the row no longer
/// had — a history chain that never happened.
///
/// Two threads drive the same Application between two statuses. Ordered by
/// `rowid` (insert order under the shared connection, so commit order), the
/// events must form an unbroken chain: each `from_status` is the previous
/// event's `to_status`. `at` is millisecond-resolution and ties freely, which is
/// why this reads `rowid` directly rather than going through `events()`.
#[test]
fn set_status_records_a_consistent_history_chain_under_contention() {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(ApplicationStore::open(dir.path()).unwrap());
    let id = saved(&store, "https://acme.com/job/status-race");

    const ITERS: usize = 60;

    let threads: Vec<_> = [ApplicationStatus::Applied, ApplicationStatus::Saved]
        .into_iter()
        .map(|to| {
            let store = store.clone();
            let id = id.clone();
            std::thread::spawn(move || {
                for _ in 0..ITERS {
                    store.set_status(&id, to, "").unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }

    let conn = store.conn.lock();
    let mut stmt = conn
        .prepare(
            "SELECT from_status, to_status FROM status_events
             WHERE application_id = ?1 ORDER BY rowid",
        )
        .unwrap();
    let events: Vec<(String, String)> = stmt
        .query_map(params![id], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    // Two threads × ITERS unconditional transitions each append exactly one
    // event (ITERS * 2), plus the single seed event from the Application's
    // creation above.
    assert_eq!(
        events.len(),
        ITERS * 2 + 1,
        "every transition records exactly one event (+1 creation seed)"
    );
    for (i, (from, _to)) in events.iter().enumerate().skip(1) {
        assert_eq!(
            from,
            &events[i - 1].1,
            "event {i} records from_status {from:?}, but the previous event moved the row to {:?} \
             — the read happened outside the write's transaction",
            events[i - 1].1
        );
    }
}

/// `update_fields` carried the SAME two-lock structure `upsert_internal` was
/// fixed for: `get` took and released the mutex, then the write retook it and
/// re-persisted EVERY column from the now-stale snapshot. A concurrent
/// `merge_answers` (extension `answers.save`) commit landing in that gap was
/// silently clobbered.
///
/// Same honest proxy as the test above — hammer one Application from two
/// threads, one appending an individually-traceable answer per iteration, the
/// other patching an unrelated field — and assert every answer survives.
#[test]
fn update_fields_and_merge_answers_race_never_loses_an_answer() {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(ApplicationStore::open(dir.path()).unwrap());
    let id = saved(&store, "https://acme.com/job/update-race");

    const ITERS: usize = 40;

    let merge_thread = spawn_answer_merger(&store, &id, ITERS);

    let update_store = store.clone();
    let update_id = id.clone();
    let update_thread = std::thread::spawn(move || {
        for i in 0..ITERS {
            edit(&update_store, &update_id, |p| {
                p.notes = Some(format!("note-{i}"))
            });
        }
    });

    merge_thread.join().unwrap();
    update_thread.join().unwrap();

    let app = store.get(&id).unwrap();
    for i in 0..ITERS {
        assert!(
            app.answers
                .iter()
                .any(|a| a.question == format!("merge-question-{i}")),
            "merge_answers entry {i} was lost to a concurrent update_fields"
        );
    }
    assert_eq!(
        app.answers.len(),
        ITERS,
        "no answer may be dropped by a concurrent update_fields"
    );
    assert!(
        app.notes.starts_with("note-"),
        "the last update_fields patch must survive, got {:?}",
        app.notes
    );
}
