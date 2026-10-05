use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use sentry::protocol::Event;

use super::*;

/// Stand-in for the ureq transport: records what actually reached the wire.
#[derive(Default)]
struct RecordingTransport {
    sent: Mutex<Vec<Envelope>>,
    flushed: AtomicU64,
    shut_down: AtomicU64,
}

impl RecordingTransport {
    fn sent_count(&self) -> usize {
        self.sent.lock().unwrap().len()
    }
}

impl Transport for RecordingTransport {
    fn send_envelope(&self, envelope: Envelope) {
        self.sent.lock().unwrap().push(envelope);
    }
    fn flush(&self, _timeout: Duration) -> bool {
        self.flushed.fetch_add(1, Ordering::Relaxed);
        true
    }
    fn shutdown(&self, _timeout: Duration) -> bool {
        self.shut_down.fetch_add(1, Ordering::Relaxed);
        true
    }
}

fn guarded(consent: bool) -> (GuardedTransport, Arc<RecordingTransport>) {
    let inner = Arc::new(RecordingTransport::default());
    let gate = Arc::new(AtomicBool::new(consent));
    let transport = GuardedTransport::new(
        inner.clone(),
        Box::new(move || gate.load(Ordering::Relaxed)),
    );
    (transport, inner)
}

/// A real envelope with a real item, built the way the capture path does.
fn parsed_envelope() -> Envelope {
    let mut envelope = Envelope::new();
    envelope.add_item(Event::default());
    envelope
}

/// A real raw envelope — the exact constructor `tauri-plugin-sentry` (0.7)
/// reaches for when `from_slice` fails.
fn raw_envelope() -> Envelope {
    Envelope::from_bytes_raw(b"{\"event_id\":\"nope\"}\n{\"type\":\"event\"}\n{}".to_vec())
        .expect("from_bytes_raw never fails")
}

#[test]
fn a_raw_envelope_is_dropped_and_counted() {
    let (transport, inner) = guarded(true);

    // Precondition: this really is an `Items::Raw` envelope, not an empty
    // one we accidentally built — a raw envelope round-trips its bytes.
    let mut written = Vec::new();
    raw_envelope().to_writer(&mut written).unwrap();
    assert!(
        !written.is_empty(),
        "precondition: the raw envelope carries bytes that WOULD reach the wire"
    );

    transport.send_envelope(raw_envelope());

    assert_eq!(inner.sent_count(), 0, "raw bytes must never reach the wire");
    assert_eq!(
        transport.dropped_opaque.load(Ordering::Relaxed),
        1,
        "the drop must be counted so it is observable"
    );
    assert_eq!(
        transport.dropped_without_consent.load(Ordering::Relaxed),
        0,
        "consent was granted — this must be attributed to opacity, not consent"
    );
}

#[test]
fn nothing_is_forwarded_without_consent() {
    let (transport, inner) = guarded(false);

    transport.send_envelope(parsed_envelope());
    transport.send_envelope(raw_envelope());

    assert_eq!(
        inner.sent_count(),
        0,
        "a closed gate must stop a perfectly valid envelope too"
    );
    assert_eq!(
        transport.dropped_without_consent.load(Ordering::Relaxed),
        2,
        "consent is checked first, so both drops are attributed to consent"
    );
}

#[test]
fn a_parsed_envelope_is_forwarded_once_when_consent_is_granted() {
    let (transport, inner) = guarded(true);

    transport.send_envelope(parsed_envelope());

    assert_eq!(
        inner.sent_count(),
        1,
        "the gate must not break ordinary crash reporting"
    );
    assert_eq!(transport.dropped_opaque.load(Ordering::Relaxed), 0);
    assert_eq!(transport.dropped_without_consent.load(Ordering::Relaxed), 0);
}

/// The gate follows consent as it changes — a mid-session opt-out closes it
/// for the rest of the session without a restart.
#[test]
fn the_gate_is_re_read_per_envelope_not_captured_once() {
    let inner = Arc::new(RecordingTransport::default());
    let gate = Arc::new(AtomicBool::new(true));
    let transport = GuardedTransport::new(inner.clone(), {
        let gate = gate.clone();
        Box::new(move || gate.load(Ordering::Relaxed))
    });

    transport.send_envelope(parsed_envelope());
    assert_eq!(inner.sent_count(), 1, "precondition: open gate forwards");

    gate.store(false, Ordering::Relaxed);
    transport.send_envelope(parsed_envelope());

    assert_eq!(
        inner.sent_count(),
        1,
        "an opt-out mid-session must take effect on the very next envelope"
    );
}

/// A wrapper that swallowed these would strand the supervisor's pre-exit
/// flush, losing the crash it was forked to report.
#[test]
fn flush_and_shutdown_reach_the_inner_transport() {
    let (transport, inner) = guarded(true);

    assert!(transport.flush(Duration::from_secs(1)));
    assert!(transport.shutdown(Duration::from_secs(1)));

    assert_eq!(inner.flushed.load(Ordering::Relaxed), 1, "flush delegated");
    assert_eq!(
        inner.shut_down.load(Ordering::Relaxed),
        1,
        "shutdown delegated"
    );
}

/// Pins the detector itself, so a future refactor of [`is_opaque`] has to
/// keep distinguishing the two shapes.
#[test]
fn is_opaque_separates_raw_from_parsed() {
    assert!(is_opaque(&raw_envelope()), "raw envelopes are opaque");
    assert!(
        !is_opaque(&parsed_envelope()),
        "an envelope with items is inspectable"
    );
}
