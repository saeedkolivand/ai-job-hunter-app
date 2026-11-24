//! `redact_event` reduces debug-image paths to a basename (#1530).

use super::super::redact_event;

fn image_names(paths: &[&str]) -> Vec<String> {
    let images: Vec<_> = paths
        .iter()
        .map(|p| serde_json::json!({"type": "symbolic", "id": "00000000-0000-0000-0000-000000000000-1", "name": p, "image_addr": "0x0", "image_size": 1}))
        .collect();
    let event: sentry::protocol::Event<'static> =
        serde_json::from_value(serde_json::json!({"debug_meta": {"images": images}})).unwrap();
    let out = redact_event(event).expect("event survives");
    let json = serde_json::to_value(&out).unwrap();
    json["debug_meta"]["images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn spaced_profile_paths_keep_only_the_basename() {
    let names = image_names(&[
        r"C:\Users\Jane Doe\AppData\Local\AI Job Hunter\ajh-tauri.exe",
        "/home/jane doe/.local/bin/ajh-tauri",
        "/Users/jane doe/Applications/ajh-tauri",
    ]);
    assert_eq!(names, ["ajh-tauri.exe", "ajh-tauri", "ajh-tauri"]);
    assert!(names
        .iter()
        .all(|n| !n.contains("doe") && !n.contains("Doe")));
}
