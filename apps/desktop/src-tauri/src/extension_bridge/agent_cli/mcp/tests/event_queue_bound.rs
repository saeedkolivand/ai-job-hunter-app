use super::*;
// ── The bounded reader → writer event queue ──────────────────────────────

/// The OTHER half of the backpressure the reader split lost. Bounding the DISPATCH queue only
/// stopped `tools/call` frames piling up; a client that stops draining stdout parks the loop
/// inside `emit`, and with an unbounded `Event` channel the reader would go on turning a
/// never-blocking stdin (a file, or a pipelining client) into `Event::Line`s without limit.
///
/// Measured as the reader's own progress, which is the only place the difference shows: with the
/// writer held still, the reader may hand over exactly [`MCP_EVENT_QUEUE_MAX`] queued lines, plus
/// the one the loop already took out of the queue, plus the one it is parked in `send` holding —
/// and then it must STOP. Mutation-visible and not by a hair: swap the `sync_channel` back for a
/// `channel()` and the reader swallows the whole input in the same microseconds, so the exact
/// count below is out by a factor of four rather than by one frame.
#[test]
fn a_parked_writer_stops_the_reader_at_the_event_queue_bound() {
    // Four times the bound, so "stopped at the bound" and "read the whole input" are nowhere
    // near each other.
    let total = MCP_EVENT_QUEUE_MAX * 4 + 16;
    let lines: Vec<String> = (1..=total)
        .map(|id| line(json!({ "jsonrpc": "2.0", "id": id, "method": "ping" })))
        .collect();
    let produced = Arc::new(AtomicUsize::new(0));
    let input = PacedInput {
        lines: lines.into_iter(),
        current: Vec::new(),
        pos: 0,
        produced: Arc::clone(&produced),
    };

    let buffer = Arc::new(Mutex::new(Vec::new()));
    let (parked_tx, writer_parked) = std::sync::mpsc::channel::<()>();
    let (release, blocked) = std::sync::mpsc::channel::<()>();
    let writer = ParkingWriter {
        buffer: Arc::clone(&buffer),
        parked: Some(parked_tx),
        release: blocked,
    };

    let server = std::thread::spawn(move || {
        let server = Server::new(true, true);
        serve(input, writer, &server, stub_ok, INVOCATION_TIMEOUT)
    });

    writer_parked
        .recv_timeout(SIGNAL_BUDGET)
        .expect("the writer must reach its first frame");

    // `+ 2`: the line the loop pulled out of the queue before parking in `emit`, and the line the
    // reader is parked in `send` holding. Everything else must still be unread.
    let ceiling = MCP_EVENT_QUEUE_MAX + 2;
    let deadline = std::time::Instant::now() + SIGNAL_BUDGET;
    while produced.load(Ordering::SeqCst) < ceiling && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    // The reader is now against the wall (or the assertion below says it never got there). The
    // settle exists for the MUTATION direction only — it gives an unbounded reader, which needs
    // microseconds for the remaining lines, all the room it could want to run away in. A correct
    // reader cannot move at all while the writer is parked, so this changes nothing here.
    std::thread::sleep(Duration::from_millis(50));
    let while_parked = produced.load(Ordering::SeqCst);
    assert_eq!(
        while_parked, ceiling,
        "with the writer parked, the reader must stop at the {MCP_EVENT_QUEUE_MAX}-deep event \
         queue (+2 in flight) rather than buffering all {total} lines of a stdin that never \
         blocks on its own"
    );

    drop(release);
    let code = server.join().expect("serve must not panic");
    assert_eq!(code, 0);

    // Backpressure, not loss: once the writer drains, the reader resumes and every frame is still
    // answered exactly once — the bound would be worthless if it dropped lines to hold.
    let text = String::from_utf8(lock(&buffer).clone()).expect("valid utf8");
    assert_eq!(
        reply_ids(&text),
        (1..=total as i64).collect::<Vec<i64>>(),
        "every line must still be answered once, in order, after the writer unblocks"
    );
    assert_eq!(
        produced.load(Ordering::SeqCst),
        total,
        "the reader must have gone on to read the rest of the input, not abandoned it"
    );
}
