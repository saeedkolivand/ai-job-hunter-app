//! The wire gate — the last hop before anything reaches Sentry's servers.
//!
//! ## Why this exists instead of relying on `before_send`
//!
//! `before_send` is **not** the last hop. In sentry-core 0.49.1 it has exactly
//! one call site — `client/mod.rs:383`, inside the `capture_event` preparation
//! — so it only ever sees events that took the *capture* path. Anything handed
//! to `Client::send_envelope` (`client/mod.rs:500`) goes straight to the
//! transport with no callback in between.
//!
//! `tauri-plugin-sentry` 0.6 walks through exactly that door. When a renderer
//! envelope fails to parse — which happens routinely, because
//! `@sentry/vite-plugin`'s debug-ID injection writes a `debug_meta` sourcemap
//! image sentry-rust cannot deserialize (getsentry/sentry-rust#1267) — the
//! plugin rebuilds it with `Envelope::from_bytes_raw` and calls `send_envelope`
//! (`commands.rs:30-35`). Such an envelope keeps the original bytes in a
//! private `Items::Raw`, and `to_writer` copies them to the socket **verbatim**
//! (sentry-types `protocol/envelope.rs:590`), so nothing we can install on the
//! event pipeline is able to reach inside it.
//!
//! Plugin 0.5 dropped those envelopes on the floor; 0.6 forwards them. That is
//! new, unredactable egress, so the guarantee moves to the one place that sees
//! every byte: the transport.
//!
//! ## What is enforced here
//!
//! 1. **Consent**, re-checked per envelope. `Hub::current()` is thread-local,
//!    so [`super::disable_current`] only unbinds the calling thread, and the
//!    plugin holds its own `Client` clone in Tauri state that never consults the
//!    hub at all. A gate at the transport is the only one both paths must pass.
//! 2. **Opaque envelopes are dropped**, restoring 0.5's semantics. A payload we
//!    cannot inspect is a payload we cannot redact, and the privacy guarantee
//!    outranks the sourcemap edge case the plugin wanted to fix.
//!
//! Both drops are counted and logged content-free (a code and a count, never
//! the payload — the same rule the diagnostics bundle follows).
//!
//! `before_send` stays exactly what it was: the event-shaping hook that redacts
//! events on the capture path, where the structure is still typed and
//! rewritable.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sentry::{Envelope, Transport, TransportFactory, TransportOptions};

/// Reads the current transmission consent.
///
/// Injected rather than called directly so the guard can be driven in tests
/// without touching the process-wide consent file (and so two tests can hold
/// opposite opinions at the same time). Production passes
/// [`super::transmits_now`], which is a single relaxed atomic load — lock-free,
/// allocation-free, and safe to call on the transport's own thread.
type ConsentGate = Box<dyn Fn() -> bool + Send + Sync>;

/// Wraps the SDK's own transport instead of reimplementing HTTP.
///
/// Installed via `ClientOptions::transport`, which replaces the factory the SDK
/// would otherwise have used. We build that same factory
/// (`sentry::transports::DefaultTransportFactory`, which under our feature set
/// resolves to the ureq transport) and delegate to it, so retries, rate-limit
/// handling and the background sender thread stay upstream's problem.
pub struct GuardedTransportFactory;

impl TransportFactory for GuardedTransportFactory {
    fn create_transport_with_options(&self, options: TransportOptions) -> Arc<dyn Transport> {
        let inner =
            sentry::transports::DefaultTransportFactory.create_transport_with_options(options);
        Arc::new(GuardedTransport::new(inner, Box::new(super::transmits_now)))
    }
}

/// The guard itself. See the module docs for what it enforces and why.
struct GuardedTransport {
    inner: Arc<dyn Transport>,
    transmits: ConsentGate,
    dropped_without_consent: AtomicU64,
    dropped_opaque: AtomicU64,
}

impl GuardedTransport {
    fn new(inner: Arc<dyn Transport>, transmits: ConsentGate) -> Self {
        Self {
            inner,
            transmits,
            dropped_without_consent: AtomicU64::new(0),
            dropped_opaque: AtomicU64::new(0),
        }
    }
}

/// Whether this envelope carries nothing we can inspect.
///
/// `Envelope`'s `Items` enum is private, so there is no `is_raw()` to call.
/// There does not need to be: a raw envelope yields an **empty** item iterator
/// by construction (`Items::Raw(_) => [].iter()`, sentry-types
/// `protocol/envelope.rs:480`), because its bytes were never parsed into items.
/// So "no items" is exactly the set we must not forward — every raw envelope,
/// plus degenerate empty ones, which carry nothing worth sending anyway.
///
/// Parsed envelopes always carry at least one item (an event, a session, a
/// transaction), so this never swallows a legitimate payload.
fn is_opaque(envelope: &Envelope) -> bool {
    envelope.items().next().is_none()
}

impl Transport for GuardedTransport {
    fn send_envelope(&self, envelope: Envelope) {
        if !(self.transmits)() {
            let n = self.dropped_without_consent.fetch_add(1, Ordering::Relaxed) + 1;
            log::debug!("[crash-reporting] wire gate closed, envelope not sent (count {n})");
            return;
        }
        if is_opaque(&envelope) {
            let n = self.dropped_opaque.fetch_add(1, Ordering::Relaxed) + 1;
            log::warn!(
                "[crash-reporting] dropped an opaque (unparseable) envelope at the wire; \
                 it cannot be redacted (count {n})"
            );
            return;
        }
        self.inner.send_envelope(envelope);
    }

    // Both lifecycle methods delegate: swallowing them would silently break the
    // flush the minidump supervisor performs before the crash-reporter process
    // exits, and the drain `ClientInitGuard` runs on drop.
    fn flush(&self, timeout: Duration) -> bool {
        self.inner.flush(timeout)
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        self.inner.shutdown(timeout)
    }
}

#[cfg(test)]
mod tests {
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

    /// A real raw envelope — the exact constructor `tauri-plugin-sentry` 0.6
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
}
