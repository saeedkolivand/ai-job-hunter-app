//! The portable preferences section through the real engine: two devices with
//! the real store, and an older device that has never heard of the section.

use super::*;
use crate::portable_prefs::{PortablePrefsStore, SECTION};

struct PrefsDevice {
    _dir: TempDir,
    state: SyncStateStore,
    prefs: PortablePrefsStore,
}

impl PrefsDevice {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let p = dir.path().to_path_buf();
        Self {
            state: SyncStateStore::open(&p).unwrap(),
            prefs: PortablePrefsStore::open(&p).unwrap(),
            _dir: dir,
        }
    }
    fn sync(&self, server: &FakeServer, rev: u64) -> SyncReport {
        let sections: [&dyn SyncSection; 1] = [&self.prefs];
        run(&sections, &self.state, server, "scope-a", rev).unwrap()
    }
    fn set(&self, key: &str, value: Value, at: u64) {
        let updates = json!({ key: value });
        self.prefs.set(updates.as_object().unwrap(), at).unwrap();
    }
    fn get(&self, key: &str) -> Option<Value> {
        self.prefs.values().unwrap().0.get(key).cloned()
    }
}

#[test]
fn a_preference_set_on_one_device_reaches_the_other() {
    let server = FakeServer::new();
    let (a, b) = (PrefsDevice::new(), PrefsDevice::new());
    a.set("language", json!("de"), 10);
    a.sync(&server, 10);
    b.sync(&server, 20);
    assert_eq!(b.get("language"), Some(json!("de")));
}

#[test]
fn a_fresh_device_never_overwrites_values_another_device_holds() {
    let server = FakeServer::new();
    let (a, b) = (PrefsDevice::new(), PrefsDevice::new());
    a.set("language", json!("de"), 10);
    a.set("outputTone", json!("casual"), 10);
    a.sync(&server, 10);

    // B has only ever run defaults: nothing set, and a migration of values
    // that equal its defaults, stamped 0.
    b.prefs
        .migrate(json!({ "language": "en" }).as_object().unwrap())
        .unwrap();
    b.sync(&server, 20);
    a.sync(&server, 30);

    for d in [&a, &b] {
        assert_eq!(d.get("language"), Some(json!("de")));
        assert_eq!(d.get("outputTone"), Some(json!("casual")));
    }
}

#[test]
fn a_device_without_the_section_holds_it_and_keeps_syncing_its_own() {
    let server = FakeServer::new();
    let newer = PrefsDevice::new();
    newer.set("language", json!("de"), 10);
    newer.sync(&server, 10);

    // An older app: it only knows `items`.
    let older = Device::new();
    older.items.put("1", v("kept"));
    let report = older.sync(&server, 20).unwrap();

    assert_eq!(report.held, 1, "the unknown section is held, not applied");
    assert_eq!(older.state.held("scope-a").unwrap()[0].store, SECTION);
    assert_eq!(older.items.get("1"), Some(v("kept")));
    assert_eq!(report.pushed, 1, "its own sections still sync");
}

#[test]
fn a_record_that_outranks_a_received_one_still_merges_its_newer_keys() {
    let server = FakeServer::new();
    let (a, b) = (PrefsDevice::new(), PrefsDevice::new());
    a.set("language", json!("de"), 1);
    a.sync(&server, 5);
    b.sync(&server, 6);

    // B edits and pushes at a low run rev; A edits locally and syncs at a
    // higher one, so the record-level order says "local wins" (Ignore).
    b.set("outputTone", json!("casual"), 10);
    b.sync(&server, 15);
    a.set("theme", json!("dark"), 20);
    a.sync(&server, 50);

    assert_eq!(a.get("outputTone"), Some(json!("casual")), "B's edit kept");
    b.sync(&server, 60);
    assert_eq!(b.get("theme"), Some(json!("dark")));
    assert_eq!(b.get("outputTone"), Some(json!("casual")));
}
