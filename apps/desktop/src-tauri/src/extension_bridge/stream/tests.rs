use super::*;

fn as_text(m: Message) -> String {
    match m {
        Message::Text(t) => t.to_string(),
        other => panic!("expected a text frame, got {other:?}"),
    }
}
#[tokio::test]
async fn a_slow_streaming_producer_never_blocks_a_concurrently_enqueued_frame() {
    // Mirrors the HIGH fix this module exists for: before, a streaming
    // handler was awaited INLINE in the read loop, so nothing else —
    // including a same-connection `assist.cancel` reply — could reach
    // the writer until it finished. Now every producer (the read loop
    // itself, and any spawned streaming task) enqueues through its OWN
    // `ChannelFrameSink` clone into the SAME channel; a slow producer
    // must never delay another producer's frame from being observed.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    let mut slow_sink = ChannelFrameSink(tx.clone());
    tokio::spawn(async move {
        for i in 0..3 {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            slow_sink.send_frame(format!("chunk-{i}")).await;
        }
        slow_sink.send_frame("done".to_string()).await;
    });

    // A concurrent fast frame — e.g. the read loop's own dispatch for a
    // synchronous verb, or an `assist.cancel` acknowledgement — enqueued
    // through its OWN sink immediately, before any of the slow
    // producer's sleeps elapse.
    let mut fast_sink = ChannelFrameSink(tx.clone());
    fast_sink.send_frame("fast-reply".to_string()).await;

    let first = rx.recv().await.unwrap();
    assert_eq!(
        as_text(first),
        "fast-reply",
        "the fast frame must never queue behind the slow stream"
    );

    for i in 0..3 {
        let msg = rx.recv().await.unwrap();
        assert_eq!(as_text(msg), format!("chunk-{i}"));
    }
    assert_eq!(as_text(rx.recv().await.unwrap()), "done");
}
