use super::*;
use std::time::{Duration, Instant};

fn state() -> (tempfile::TempDir, BridgeState) {
    let dir = tempfile::tempdir().unwrap();
    let state = BridgeState::load(dir.path());
    (dir, state)
}

#[test]
fn settings_get_reports_all_four_defaults_off() {
    let (_dir, state) = state();
    let reply = handle_settings_get("req-1", &state);
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::SETTINGS_RESULT);
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["settings"]["autofill"], false);
    assert_eq!(v["payload"]["settings"]["aiAssist"], false);
    assert_eq!(v["payload"]["settings"]["autotrack"], false);
    assert_eq!(v["payload"]["settings"]["saveAnswersOnSubmit"], false);
}

#[test]
fn origin_refused_reply_carries_the_extension_only_sentinel() {
    let v: Value = serde_json::from_str(&origin_refused_reply("req-5")).unwrap();
    assert_eq!(v["type"], msg::SETTINGS_RESULT);
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(v["payload"]["error"], ERR_EXTENSION_ONLY);
}

#[test]
fn throttled_reply_carries_the_shared_rate_limited_sentinel() {
    let v: Value = serde_json::from_str(&throttled_reply("req-6")).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(v["payload"]["error"], agent_call::ERR_RATE_LIMITED);
}

#[test]
fn settings_set_throttle_empties_then_refills() {
    let mut throttle = SettingsSetThrottle::new();
    let t0 = Instant::now();
    for _ in 0..SETTINGS_SET_BURST as u32 {
        assert!(throttle.try_acquire_at(t0));
    }
    assert!(
        !throttle.try_acquire_at(t0),
        "burst exhausted — the next request must be refused"
    );
    let refilled = t0 + Duration::from_secs_f64(SETTINGS_SET_REFILL_SECS);
    assert!(
        throttle.try_acquire_at(refilled),
        "one refill period later, one token is available again"
    );
}
