//! `abort_if_cancelled_early` + `until_cancelled` (#1232 spend guards — see
//! their own docs). `resolve_answer_assist` itself needs an `AppHandle` and
//! this crate has no `tauri::test` harness for it, so these are tested
//! directly against a real registry.

use crate::extension_bridge::stream::AssistStreamRegistry;

use super::super::grounding::{abort_if_cancelled_early, until_cancelled};
use super::support::NoopCanceller;

#[test]
fn abort_if_cancelled_early_lets_a_live_generation_through() {
    let registry = AssistStreamRegistry::default();
    let r#gen = registry.begin("req-1").expect("a fresh reqId");

    assert!(abort_if_cancelled_early(&registry, "req-1", r#gen).is_ok());
}

#[test]
fn abort_if_cancelled_early_stops_the_spend_once_a_cancel_raced_ahead() {
    let registry = AssistStreamRegistry::default();
    let r#gen = registry.begin("req-1").expect("a fresh reqId");
    // Cancel while still Pending (no job started yet) — the exact #1232 race:
    // Cancel clicked ~0ms after the draft began, long before registration.
    registry.cancel(&NoopCanceller, "req-1");

    let err = abort_if_cancelled_early(&registry, "req-1", r#gen)
        .expect_err("a cancelled generation must not reach a paid provider call");
    assert!(err.to_string().contains("cancelled"));

    // Another generation / another request is NOT this one's cancel.
    assert!(abort_if_cancelled_early(&registry, "req-1", r#gen + 1).is_ok());
    assert!(abort_if_cancelled_early(&registry, "req-2", r#gen).is_ok());
}

#[tokio::test]
async fn until_cancelled_returns_the_value_when_no_cancel_arrives() {
    let registry = AssistStreamRegistry::default();
    let r#gen = registry.begin("req-ok").expect("a fresh reqId");

    let out = until_cancelled(&registry, "req-ok", r#gen, async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        "finished"
    })
    .await;

    assert_eq!(out, Some("finished"));
}

#[tokio::test]
async fn until_cancelled_abandons_a_long_step_once_a_cancel_lands() {
    let registry = AssistStreamRegistry::default();
    let r#gen = registry.begin("req-cancel").expect("a fresh reqId");

    // A future that would "spend" if it were ever allowed to finish.
    let spent = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = std::sync::Arc::clone(&spent);

    let cancelling = async {
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        registry.cancel(&NoopCanceller, "req-cancel");
    };
    let watched = until_cancelled(&registry, "req-cancel", r#gen, async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        "should never finish"
    });

    let (out, ()) = tokio::join!(watched, cancelling);

    assert_eq!(out, None, "a cancelled step must be abandoned, not awaited");
    assert!(
        !spent.load(std::sync::atomic::Ordering::SeqCst),
        "the abandoned future must never have run to completion"
    );
}
