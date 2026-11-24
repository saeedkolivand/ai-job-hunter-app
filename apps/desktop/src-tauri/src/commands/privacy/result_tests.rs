use super::*;

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
