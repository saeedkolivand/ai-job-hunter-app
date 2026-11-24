use super::*;
use crate::credentials::CredentialStore;

// A store that cannot clear must be reported, and must not stop the others.
struct FakeStore {
    fail: bool,
    cleared: std::sync::atomic::AtomicBool,
}
impl Resettable for FakeStore {
    fn reset(&self) -> AppResult<()> {
        if self.fail {
            return Err(crate::error::AppError::Storage("boom".into()));
        }
        self.cleared
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

fn fake(fail: bool) -> FakeStore {
    FakeStore {
        fail,
        cleared: false.into(),
    }
}

#[test]
fn failed_labels_resets_every_store_and_reports_only_failures() {
    let (a, b, c) = (fake(false), fake(true), fake(false));
    let stores: [(&'static str, &FakeStore); 3] = [("a", &a), ("b", &b), ("c", &c)];
    let failed = failed_labels(stores.iter().map(|(l, s)| (*l, s.reset())));
    assert_eq!(failed, vec!["b"]);
    assert!(a.cleared.load(std::sync::atomic::Ordering::SeqCst));
    assert!(
        c.cleared.load(std::sync::atomic::Ordering::SeqCst),
        "a store after the failing one must still be reset"
    );
}

#[test]
fn failed_labels_is_empty_when_all_clear() {
    let a = fake(false);
    assert!(failed_labels([("a", a.reset())]).is_empty());
}

#[test]
fn reset_result_reports_failed_stores_as_not_success() {
    let v = reset_result(&["portable_prefs"], true);
    assert_eq!(v["success"], false);
    assert_eq!(v["failedStores"], json!(["portable_prefs"]));
    assert!(v.get("browserStateRetained").is_none());

    let both = reset_result(&["x"], false);
    assert_eq!(both["success"], false);
    assert_eq!(both["browserStateRetained"], true);
}

#[test]
fn reset_result_success_and_browser_state_only_shapes_are_unchanged() {
    assert_eq!(reset_result(&[], true), json!({ "success": true }));
    let v = reset_result(&[], false);
    assert_eq!(v["success"], false);
    assert_eq!(v["browserStateRetained"], true);
    assert!(v.get("failedStores").is_none());
}

// A real store whose clear fails must be reported under its label, not Ok.
#[test]
fn a_real_store_that_fails_to_clear_is_reported_by_label() {
    let dir = tempfile::TempDir::new().unwrap();
    let store = crate::referrals::ReferralStore::open(dir.path()).unwrap();
    rusqlite::Connection::open(dir.path().join("referrals.db"))
        .unwrap()
        .execute_batch("DROP TABLE referrals")
        .unwrap();
    let failed = failed_labels([("referrals", store.reset())]);
    assert_eq!(failed, vec!["referrals"]);
}

// An unreadable credential index must fail the reset (a missing one is just empty).
#[test]
fn credentials_reset_fails_on_a_corrupt_index_but_accepts_a_missing_one() {
    let dir = tempfile::TempDir::new().unwrap();
    let new = || Mutex::new(CredentialStore::new(&dir.path().to_path_buf()));
    new().reset().unwrap();
    std::fs::write(dir.path().join("credential-meta.json"), "{x").unwrap();
    assert!(new().reset().is_err());
}
