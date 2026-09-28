use super::*;

use serde_json::Value;

use super::super::assist_registry::JobCanceller;

/// A tiny local copy of `assist_registry::tests::RecordingCanceller` —
/// duplicated (not shared) so this file's test module stays independent
/// of that module's own private test internals. Used only by the ONE
/// test below that needs to prove a cancel finds the Pending marker
/// `begin_or_reject_duplicate` leaves behind.
#[derive(Default)]
struct RecordingCanceller {
    cancelled: std::cell::RefCell<Vec<String>>,
}

impl JobCanceller for RecordingCanceller {
    fn cancel_job(&self, job_id: &str) {
        self.cancelled.borrow_mut().push(job_id.to_string());
    }
}

// ── begin_or_reject_duplicate (HIGH fix: pre-begin cancel-drop —
// `begin` must run synchronously, on the read loop's own thread, BEFORE
// `tokio::spawn` ever schedules the streaming task) ────────────────────

#[test]
fn begin_or_reject_duplicate_marks_pending_synchronously_before_any_task_runs() {
    // This whole test has no `.await` at all — `begin_or_reject_duplicate`
    // is a plain, non-async fn — so a `Some(gen)` return, and `contains`
    // reporting `true` immediately after, already proves `begin` ran on
    // the CALLER's thread, not deferred into whatever thread a spawned
    // task eventually runs on.
    let registry = AssistStreamRegistry::default();
    let (out_tx, _out_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    let r#gen =
        begin_or_reject_duplicate(&registry, "req-1", &out_tx).expect("a fresh reqId is accepted");
    assert!(
        registry.contains("req-1"),
        "begin must have run synchronously — before any spawn, before any await"
    );

    // The exact race this fix closes: a same-connection `assist.cancel`
    // dispatched right after `spawn_answer_assist` returns (before the
    // spawned task has run AT ALL) must still find the Pending marker,
    // never nothing.
    let canceller = RecordingCanceller::default();
    registry.cancel(&canceller, "req-1");
    assert!(
        canceller.cancelled.borrow().is_empty(),
        "no job exists yet — Pending just becomes CancelledEarly, nothing to job_cancel"
    );
    assert!(
        !registry.register("req-1", r#gen, "job-1"),
        "the cancel that raced ahead of the spawned task's own register call must still win"
    );
}

#[test]
fn begin_or_reject_duplicate_rejects_an_already_active_req_id_via_out_tx() {
    let registry = AssistStreamRegistry::default();
    registry.begin("req-1"); // the original request is already in flight
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    assert!(begin_or_reject_duplicate(&registry, "req-1", &out_tx).is_none());

    let frame = out_rx
        .try_recv()
        .expect("a duplicate-rejection reply must be enqueued through out_tx directly");
    let v: Value = serde_json::from_str(&as_text(frame)).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(
        v["payload"]["error"],
        super::super::answer_assist::DUPLICATE_REQUEST_MESSAGE
    );
    assert!(
        registry.contains("req-1"),
        "the ORIGINAL request's entry must be left untouched by the rejected duplicate"
    );
}

// `start_and_register`'s tests (TOCTOU fix — job_start before register)
// now live in `assist_registry` alongside the function itself.

fn as_text(m: Message) -> String {
    match m {
        Message::Text(t) => t.to_string(),
        other => panic!("expected a text frame, got {other:?}"),
    }
}

// ── `agent_query_or_cancelled` (MAJOR fix — security review round 2):
// an in-flight `agent.query` must never send its reply once this
// connection's cancellation token has fired — see `spawn_agent_query`'s
// doc for the token-revocation scenario this closes. ────────────────────

#[tokio::test]
async fn agent_query_or_cancelled_suppresses_the_reply_once_cancelled() {
    let cancel = CancellationToken::new();
    cancel.cancel();
    // The query future here never resolves — proof this doesn't wait for
    // it once `cancel` has already fired. Bounded well past any
    // reasonable budget so a regression that ignores `cancel` hangs this
    // test instead of the whole suite.
    let outcome = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        agent_query_or_cancelled(std::future::pending::<String>(), &cancel),
    )
    .await;
    assert_eq!(
        outcome.ok(),
        Some(None),
        "a cancelled connection must suppress the query's reply, never send it"
    );
}

#[tokio::test]
async fn agent_query_or_cancelled_returns_the_reply_when_never_cancelled() {
    // The normal case, unaffected by this fix: an un-cancelled
    // connection must still deliver the query's own result unchanged.
    let cancel = CancellationToken::new();
    let outcome =
        agent_query_or_cancelled(std::future::ready("agent.result".to_string()), &cancel).await;
    assert_eq!(outcome, Some("agent.result".to_string()));
}

#[tokio::test]
async fn agent_query_or_cancelled_races_a_cancel_that_fires_mid_flight() {
    // A cancel arriving WHILE the query is still in flight (not already
    // cancelled before the race even starts) — the realistic timing for
    // a token revoked mid-`best-matches`.
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        cancel_clone.cancel();
    });
    let outcome = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        agent_query_or_cancelled(std::future::pending::<String>(), &cancel),
    )
    .await;
    assert_eq!(
        outcome.ok(),
        Some(None),
        "a cancel that fires mid-flight must still suppress the reply"
    );
}
