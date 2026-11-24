use serde_json::{json, Map, Value};
use tempfile::TempDir;

use super::*;
use crate::data_store::{DataStore, SyncSection};

fn store() -> (TempDir, PortablePrefsStore) {
    let dir = TempDir::new().unwrap();
    let s = PortablePrefsStore::open(&dir.path().to_path_buf()).unwrap();
    (dir, s)
}

fn map(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn remote(key: &str, value: Value, at: u64) -> Value {
    json!({ "version": 1, "keys": { key: { "value": value, "updatedAt": at } } })
}

#[test]
fn a_never_set_store_exports_no_keys() {
    let (_d, s) = store();
    assert_eq!(s.export(), json!({ "version": 1, "keys": {} }));
}

#[test]
fn set_rejects_a_key_outside_the_allowlist() {
    let (_d, s) = store();
    for key in [
        "apiKey",
        "syncToken",
        "performanceMode",
        "closeToTray",
        "fetchCompanyLogos",
    ] {
        let err = s.set(&map(json!({ key: "x" })), 5).unwrap_err();
        assert!(
            err.to_string().contains("not a portable preference"),
            "{key}"
        );
    }
    assert_eq!(s.values().unwrap().0.len(), 0);
}

#[test]
fn set_rejects_null_and_oversized_values_without_a_partial_write() {
    let (_d, s) = store();
    assert!(s.set(&map(json!({ "language": null })), 5).is_err());
    let big = "x".repeat(MAX_VALUE_BYTES + 1);
    assert!(s
        .set(&map(json!({ "language": "de", "userName": big })), 5)
        .is_err());
    assert!(s.values().unwrap().0.is_empty(), "nothing was written");
}

#[test]
fn an_unchanged_value_keeps_its_timestamp_and_a_change_is_strictly_newer() {
    let (_d, s) = store();
    s.set(&map(json!({ "language": "de" })), 100).unwrap();
    s.set(&map(json!({ "language": "de" })), 200).unwrap();
    assert_eq!(parse_wire(&s.export())["language"].updated_at, 100);
    // A clock that stepped back still yields a strictly newer stamp.
    s.set(&map(json!({ "language": "en" })), 50).unwrap();
    assert_eq!(parse_wire(&s.export())["language"].updated_at, 101);
}

#[test]
fn a_received_payload_drops_unknown_and_malformed_keys() {
    let (_d, s) = store();
    let payload = json!({ "version": 1, "keys": {
        "language": { "value": "de", "updatedAt": 9 },
        "apiKey": { "value": "sk-secret", "updatedAt": 9 },
        "closeToTray": { "value": false, "updatedAt": 9 },
        "outputTone": { "value": null, "updatedAt": 9 },
        "theme": "not an entry",
    }});
    s.upsert_record(SECTION, &payload).unwrap();
    let (values, _) = s.values().unwrap();
    assert_eq!(Value::Object(values), json!({ "language": "de" }));
}

#[test]
fn a_newer_remote_key_wins_an_older_one_loses_and_a_tie_keeps_local() {
    let (_d, s) = store();
    s.set(
        &map(json!({ "language": "de", "outputTone": "casual" })),
        100,
    )
    .unwrap();
    s.upsert_record(SECTION, &remote("language", json!("fr"), 50))
        .unwrap();
    s.upsert_record(SECTION, &remote("outputTone", json!("formal"), 100))
        .unwrap();
    s.upsert_record(SECTION, &remote("theme", json!({ "scheme": "dark" }), 1))
        .unwrap();
    let (values, _) = s.values().unwrap();
    assert_eq!(values["language"], "de", "older remote loses");
    assert_eq!(values["outputTone"], "casual", "a tie keeps local");
    assert_eq!(
        values["theme"],
        json!({ "scheme": "dark" }),
        "new key lands"
    );

    s.upsert_record(SECTION, &remote("language", json!("fr"), 101))
        .unwrap();
    assert_eq!(s.values().unwrap().0["language"], "fr", "newer remote wins");
}

#[test]
fn migration_fills_only_missing_keys_stamped_zero_and_runs_once() {
    let (_d, s) = store();
    s.upsert_record(SECTION, &remote("language", json!("de"), 40))
        .unwrap();
    s.migrate(&map(json!({ "language": "en", "userName": "Ada" })))
        .unwrap();
    let (values, migrated) = s.values().unwrap();
    assert!(migrated);
    assert_eq!(
        values["language"], "de",
        "a received value is never overwritten"
    );
    assert_eq!(values["userName"], "Ada");
    assert_eq!(parse_wire(&s.export())["userName"].updated_at, 0);

    s.migrate(&map(json!({ "outputTone": "casual" }))).unwrap();
    assert!(
        !s.values().unwrap().0.contains_key("outputTone"),
        "once only"
    );
}

#[test]
fn a_migrated_value_loses_to_any_real_edit_from_another_device() {
    let (_d, s) = store();
    s.migrate(&map(json!({ "language": "en" }))).unwrap();
    s.upsert_record(SECTION, &remote("language", json!("de"), 1))
        .unwrap();
    assert_eq!(s.values().unwrap().0["language"], "de");
}

#[test]
fn migration_rejects_a_key_outside_the_allowlist_and_stays_pending() {
    let (_d, s) = store();
    assert!(s.migrate(&map(json!({ "apiKey": "x" }))).is_err());
    assert!(!s.values().unwrap().1);
}

#[test]
fn the_synced_payload_holds_only_allowlisted_keys() {
    let (_d, s) = store();
    // Everything a caller could try to smuggle in, through every write path.
    let smuggled = [
        "apiKey",
        "token",
        "path",
        "extensionPermissions",
        "emailWatch",
    ];
    for key in smuggled {
        let _ = s.set(&map(json!({ key: "x" })), 1);
        let _ = s.migrate(&map(json!({ key: "x" })));
    }
    s.import(&json!({ "keys": {
        "apiKey": { "value": "x", "updatedAt": 1 },
        "language": { "value": "de", "updatedAt": 1 },
    }}))
    .unwrap();
    s.upsert_record(SECTION, &remote("token", json!("t"), 9))
        .unwrap();

    let record = &s.sync_records().unwrap()[0];
    let keys = record.payload["keys"].as_object().unwrap();
    assert!(keys.keys().all(|k| PORTABLE_KEYS.contains(&k.as_str())));
    assert_eq!(keys.len(), 1);
    let text = record.payload.to_string();
    assert!(smuggled.iter().all(|k| !text.contains(k)));
}

#[test]
fn the_allowlist_names_no_per_device_or_secret_setting() {
    for banned in [
        "performanceMode",
        "customPerformance",
        "closeToTray",
        "launchAtLogin",
        "fetchCompanyLogos",
        "autoIndexOnUpload",
        "semanticScoring",
        "aiProviderConfig",
        "debugMode",
    ] {
        assert!(!PORTABLE_KEYS.contains(&banned), "{banned}");
    }
}

#[test]
fn import_replaces_the_keys_and_a_bundle_without_the_section_is_untouched() {
    let (_d, s) = store();
    s.set(&map(json!({ "language": "de" })), 5).unwrap();
    assert_eq!(s.import(&Value::Null).unwrap(), 0);
    assert_eq!(s.values().unwrap().0["language"], "de");

    let n = s.import(&remote("outputTone", json!("casual"), 7)).unwrap();
    assert_eq!(n, 1);
    let (values, migrated) = s.values().unwrap();
    assert!(migrated);
    assert_eq!(Value::Object(values), json!({ "outputTone": "casual" }));
}

#[test]
fn clear_forgets_values_and_keeps_the_migration_done() {
    let (_d, s) = store();
    s.set(&map(json!({ "language": "de" })), 5).unwrap();
    s.clear().unwrap();
    let (values, migrated) = s.values().unwrap();
    assert!(values.is_empty() && migrated);
}
