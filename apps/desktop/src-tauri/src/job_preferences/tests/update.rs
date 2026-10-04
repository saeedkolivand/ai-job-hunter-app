use super::{support::*, *};

// ── Atomic read-merge-write (`update`) ────────────────────────────────────────

/// `job_preferences_set` merges its body over the stored row, and `set()` is a
/// full-row `UPDATE`, so that read and that write must be ONE critical section.
/// With two lock acquisitions, two concurrent partial updates both merge from
/// the same snapshot and the later write erases the earlier one's field.
///
/// Only this thread writes `location` and only the other writes
/// `salary_expectation`, so ANY value other than the one this thread last wrote
/// is a lost update — and it is checked on EVERY iteration, not just at the end:
/// a final-state assertion alone passes even on the racy version, because
/// whichever thread writes last happens to carry a fresh copy of the other's
/// field.
#[test]
fn two_concurrent_updates_never_lose_each_others_field() {
    const ITERATIONS: usize = 200;
    let (_dir, store) = open_store();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut previous: Option<String> = None;
            for i in 0..ITERATIONS {
                let mine = format!("location-{i}");
                store
                    .update(|stored| {
                        assert_eq!(
                            stored.location, previous,
                            "the other thread's merge overwrote this thread's location: \
                             read + write is not atomic"
                        );
                        Ok(JobPreferences {
                            location: Some(mine.clone()),
                            ..stored.clone()
                        })
                    })
                    .unwrap();
                previous = Some(mine);
            }
        });
        scope.spawn(|| {
            let mut previous: Option<String> = None;
            for i in 0..ITERATIONS {
                let mine = format!("€{i}");
                store
                    .update(|stored| {
                        assert_eq!(
                            stored.salary_expectation, previous,
                            "the other thread's merge overwrote this thread's salary: \
                             read + write is not atomic"
                        );
                        Ok(JobPreferences {
                            salary_expectation: Some(mine.clone()),
                            ..stored.clone()
                        })
                    })
                    .unwrap();
                previous = Some(mine);
            }
        });
    });

    let after = store.get();
    assert_eq!(
        after.location,
        Some(format!("location-{}", ITERATIONS - 1)),
        "the final row must hold BOTH threads' last values"
    );
    assert_eq!(
        after.salary_expectation,
        Some(format!("€{}", ITERATIONS - 1))
    );
}

/// The structural half of the guarantee above, deterministic where the racing
/// loop is statistical: a second update must not be able to COMPLETE while the
/// first one's merge is still in flight. Re-implement `update` as `get()` then
/// `set()` (two acquisitions) and this fails on every run — the merge would run
/// with no lock held, so the other thread's whole read-merge-write slips through
/// the sleep below.
#[test]
fn a_second_update_cannot_complete_while_a_merge_is_in_flight() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let (_dir, store) = open_store();
    let merging = AtomicBool::new(false);
    let other_finished = AtomicBool::new(false);

    std::thread::scope(|scope| {
        scope.spawn(|| {
            store
                .update(|stored| {
                    merging.store(true, Ordering::SeqCst);
                    // A window far wider than the other thread's whole update
                    // needs, so "it did not finish" means it was BLOCKED.
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    assert!(
                        !other_finished.load(Ordering::SeqCst),
                        "another update completed while this merge was still in flight: \
                         the read, the merge and the write are not one critical section"
                    );
                    Ok(JobPreferences {
                        location: Some("Berlin".to_string()),
                        ..stored.clone()
                    })
                })
                .unwrap();
        });
        scope.spawn(|| {
            while !merging.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            store
                .update(|stored| {
                    Ok(JobPreferences {
                        salary_expectation: Some("€75,000".to_string()),
                        ..stored.clone()
                    })
                })
                .unwrap();
            other_finished.store(true, Ordering::SeqCst);
        });
    });

    let after = store.get();
    assert_eq!(after.location.as_deref(), Some("Berlin"));
    assert_eq!(
        after.salary_expectation.as_deref(),
        Some("€75,000"),
        "the blocked update must still land once the lock is released"
    );
}

/// A merge that REFUSES the body writes nothing — the store-level half of the
/// command's "nothing may be written when the body is refused" test, pinned
/// here because `update` is where the write is now skipped.
#[test]
fn a_failing_merge_leaves_the_stored_row_untouched() {
    let (_dir, store) = open_store();
    store
        .set(&JobPreferences {
            location: Some("Berlin".to_string()),
            country_code: Some("DE".to_string()),
            ..blank()
        })
        .unwrap();

    let err = store
        .update(|_| Err(crate::error::AppError::Validation("nope".to_string())))
        .unwrap_err();

    assert_eq!(err.to_string(), "nope");
    assert_eq!(store.get().location, Some("Berlin".to_string()));
    assert_eq!(store.get().country_code, Some("DE".to_string()));
}
