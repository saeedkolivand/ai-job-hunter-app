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
//! `tauri-plugin-sentry` 0.7 (its envelope handling is unchanged since 0.6)
//! walks through exactly that door. When a renderer envelope fails to parse —
//! routinely in apps that use `@sentry/vite-plugin`,
//! whose debug-ID injection writes a `debug_meta` sourcemap image sentry-rust
//! cannot deserialize (getsentry/sentry-rust#1267); this app ships no
//! `@sentry/*` package, so the drop path is expected to stay cold here — the
//! plugin rebuilds it with `Envelope::from_bytes_raw` and calls `send_envelope`
//! (`commands.rs:30-35`). Such an envelope keeps the original bytes in a
//! private `Items::Raw`, and `to_writer` copies them to the socket **verbatim**
//! (sentry-types `protocol/envelope.rs:590`), so nothing we can install on the
//! event pipeline is able to reach inside it.
//!
//! Plugin 0.5 dropped those envelopes on the floor; 0.6 onward forwards them.
//! That is new, unredactable egress, so the guarantee moves to the one place
//! that sees every byte: the transport.
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
mod tests;
