use parking_lot::Mutex;
use serde_json::json;

use super::*;
use crate::tray::PendingMenu;

#[test]
fn returns_buffered_intent_then_clears() {
    let payload = json!({ "route": "/settings", "section": null });
    let buf = PendingMenu(Mutex::new(Some((
        "menu:navigate".to_string(),
        payload.clone(),
    ))));

    assert_eq!(
        take_pending(&buf),
        Some(PendingMenuIntent {
            event: "menu:navigate".to_string(),
            payload,
        })
    );
    // Atomic take cleared the slot — a second pull (e.g. a later focus) is empty,
    // so an intent is delivered exactly once and can't re-fire.
    assert_eq!(take_pending(&buf), None);
}

#[test]
fn returns_none_when_empty() {
    let buf = PendingMenu(Mutex::new(None));
    assert_eq!(take_pending(&buf), None);
}
