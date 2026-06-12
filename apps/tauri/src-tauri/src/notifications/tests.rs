use super::*;
use tempfile::TempDir;

fn new_input(kind: &str, title: &str) -> NewNotification {
    NewNotification {
        kind: kind.to_string(),
        title: title.to_string(),
        body: format!("{title} body"),
        route: None,
    }
}

#[test]
fn push_prepends_newest_first_and_stamps_record() {
    let dir = TempDir::new().unwrap();
    let store = NotificationStore::new(dir.path());

    let first = store.push(new_input("import.result", "first"));
    let second = store.push(new_input("autopilot.new_jobs", "second"));

    // Returned record is well-formed.
    assert!(!second.id.is_empty(), "id is generated and non-empty");
    assert!(!second.read, "new notifications start unread");
    assert!(second.created_at > 0, "created_at is a sane epoch-millis stamp");

    // Newest-first ordering: the most recent push is at index 0.
    let list = store.list();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, second.id, "second push is newest → index 0");
    assert_eq!(list[1].id, first.id, "first push is oldest → index 1");
}

#[test]
fn cap_keeps_newest_50_and_drops_oldest() {
    let dir = TempDir::new().unwrap();
    let store = NotificationStore::new(dir.path());

    // Push 60 — only the newest 50 survive.
    for i in 0..60 {
        store.push(new_input("test.kind", &format!("n{i}")));
    }

    let list = store.list();
    assert_eq!(list.len(), MAX_NOTIFICATIONS, "trimmed to the cap");
    // Newest (n59) retained at the head; oldest 10 (n0..n9) dropped.
    assert_eq!(list[0].title, "n59", "newest retained at head");
    assert_eq!(list[MAX_NOTIFICATIONS - 1].title, "n10", "oldest survivor is n10");
    assert!(
        !list.iter().any(|n| n.title == "n9"),
        "the oldest over-cap entries are dropped"
    );
}

#[test]
fn persists_across_reopen_with_order() {
    let dir = TempDir::new().unwrap();
    {
        let store = NotificationStore::new(dir.path());
        store.push(new_input("a", "first"));
        store.push(new_input("b", "second"));
        store.push(new_input("c", "third"));
    } // drop the store — only the JSON file remains

    let reopened = NotificationStore::new(dir.path());
    let list = reopened.list();
    assert_eq!(list.len(), 3);
    // Same newest-first order survives the round-trip.
    assert_eq!(list[0].title, "third");
    assert_eq!(list[1].title, "second");
    assert_eq!(list[2].title, "first");
}

#[test]
fn mark_read_flips_one_and_reports_change() {
    let dir = TempDir::new().unwrap();
    let store = NotificationStore::new(dir.path());
    let n = store.push(new_input("k", "t"));

    assert!(store.mark_read(&n.id), "found + changed → true");
    assert!(!store.mark_read(&n.id), "already read → no change → false");
    assert!(!store.mark_read("nope"), "unknown id → false");

    // Persisted: a fresh store sees the read flag.
    let reopened = NotificationStore::new(dir.path());
    assert!(reopened.list()[0].read, "read flag persisted to disk");
}

#[test]
fn mark_all_read_flips_every_record() {
    let dir = TempDir::new().unwrap();
    let store = NotificationStore::new(dir.path());
    store.push(new_input("k", "a"));
    store.push(new_input("k", "b"));
    store.push(new_input("k", "c"));

    store.mark_all_read();

    assert!(store.list().iter().all(|n| n.read), "all flipped read");
    // Persisted.
    let reopened = NotificationStore::new(dir.path());
    assert!(reopened.list().iter().all(|n| n.read));
}

#[test]
fn remove_drops_one_and_reports_result() {
    let dir = TempDir::new().unwrap();
    let store = NotificationStore::new(dir.path());
    let keep = store.push(new_input("k", "keep"));
    let drop = store.push(new_input("k", "drop"));

    assert!(store.remove(&drop.id), "removed an existing → true");
    assert!(!store.remove("nope"), "unknown id → false");

    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, keep.id, "the right one survived");

    // Persisted.
    let reopened = NotificationStore::new(dir.path());
    assert_eq!(reopened.list().len(), 1);
    assert_eq!(reopened.list()[0].id, keep.id);
}

#[test]
fn clear_all_empties_and_persists() {
    let dir = TempDir::new().unwrap();
    let store = NotificationStore::new(dir.path());
    store.push(new_input("k", "a"));
    store.push(new_input("k", "b"));

    store.clear_all();
    assert!(store.list().is_empty(), "store emptied");

    // Re-open is empty.
    let reopened = NotificationStore::new(dir.path());
    assert!(reopened.list().is_empty(), "empty persisted to disk");
}

#[test]
fn over_cap_on_disk_file_is_trimmed_on_load() {
    let dir = TempDir::new().unwrap();
    // Hand-write an over-cap (newest-first) file directly, bypassing `push`.
    let oversized: Vec<AppNotification> = (0..70)
        .map(|i| AppNotification {
            id: format!("id-{i}"),
            kind: "k".to_string(),
            title: format!("n{i}"),
            body: String::new(),
            created_at: 1_000 + i as u64,
            read: false,
            route: None,
        })
        .collect();
    let json = serde_json::to_string_pretty(&oversized).unwrap();
    std::fs::write(dir.path().join("notifications.json"), json).unwrap();

    let store = NotificationStore::new(dir.path());
    let list = store.list();
    assert_eq!(list.len(), MAX_NOTIFICATIONS, "load defensively caps the file");
    assert_eq!(list[0].title, "n0", "newest-first head preserved");
}

#[test]
fn route_round_trips_with_camel_case_search() {
    let dir = TempDir::new().unwrap();
    let store = NotificationStore::new(dir.path());

    let mut search = serde_json::Map::new();
    search.insert("jobId".to_string(), serde_json::json!("abc"));
    let route = NotificationRoute {
        to: "/jobs".to_string(),
        search: Some(search),
    };
    let created = store.push(NewNotification {
        kind: "autopilot.new_jobs".to_string(),
        title: "New jobs".to_string(),
        body: "3 new".to_string(),
        route: Some(route.clone()),
    });
    assert_eq!(created.route, Some(route.clone()));

    // Survives the disk round-trip.
    let reopened = NotificationStore::new(dir.path());
    assert_eq!(reopened.list()[0].route, Some(route));
}

#[test]
fn record_serializes_camel_case_created_at() {
    let n = AppNotification {
        id: "x".to_string(),
        kind: "k".to_string(),
        title: "t".to_string(),
        body: "b".to_string(),
        created_at: 42,
        read: false,
        route: None,
    };
    let json = serde_json::to_string(&n).unwrap();
    assert!(json.contains("\"createdAt\":42"), "camelCase createdAt: {json}");
    assert!(!json.contains("created_at"), "no snake_case leak");
}

#[test]
fn push_clamps_oversized_title_and_body_char_safe() {
    let dir = TempDir::new().unwrap();
    let store = NotificationStore::new(dir.path());

    // Oversized input built from a multi-byte char (accented + emoji) so the
    // clamp is proven char-safe: each char is >1 byte, so a byte-wise cut
    // would split a codepoint and panic.
    let big_title: String = "é".repeat(MAX_TITLE_CHARS + 50);
    let big_body: String = "🚀".repeat(MAX_BODY_CHARS + 50);
    let created = store.push(NewNotification {
        kind: "import.result".to_string(),
        title: big_title,
        body: big_body,
        route: None,
    });

    // Clamped to the char limits (not bytes), no codepoint split / panic.
    assert_eq!(created.title.chars().count(), MAX_TITLE_CHARS, "title clamped by char");
    assert_eq!(created.body.chars().count(), MAX_BODY_CHARS, "body clamped by char");

    // The clamp survives the disk round-trip.
    let reopened = NotificationStore::new(dir.path());
    let stored = &reopened.list()[0];
    assert_eq!(stored.title.chars().count(), MAX_TITLE_CHARS);
    assert_eq!(stored.body.chars().count(), MAX_BODY_CHARS);

    // A normal short title/body is stored verbatim.
    let short = store.push(new_input("k", "short title"));
    assert_eq!(short.title, "short title", "under-limit title untouched");
    assert_eq!(short.body, "short title body", "under-limit body untouched");
}
