//! The TLS-connect half of [`super`]: a timeout-bounded `imap::Client` (resolve, TCP connect, TLS
//! handshake, greeting) and the content-free error-kind label every log line in this module family
//! uses instead of the library error's own `Display`/`Debug`.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use native_tls::TlsConnector;

use crate::error::{AppError, AppResult};
use crate::observability::sanitize_reason;

/// Bounds the initial TCP connect (`TcpStream::connect_timeout`) — a
/// black-holed/firewalled host must fail the Connect/Check-now button rather
/// than pin a `spawn_blocking` worker forever.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Read/write timeout applied to the socket AFTER it connects (covers TLS
/// handshake, greeting, `LOGIN`, `EXAMINE`) — bounds a server that ACCEPTS the
/// connection but then never answers (a slow-loris-style stall), which
/// `CONNECT_TIMEOUT` alone does not cover.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// Manual TLS-connect with both a connect timeout and a read/write timeout on
/// the socket, then read the server's IMAP greeting.
///
/// The crate's own `ClientBuilder::connect()` sets NEITHER: `TcpStream::
/// connect()` has no timeout, and the builder's timeout-capable
/// `connect_with` is private — so a black-holing host hangs forever. This
/// mirrors the crate's own `examples/timeout.rs` (resolve → try each
/// `SocketAddr` with `connect_timeout` → wrap in TLS → `Client::new` →
/// `read_greeting`), plus the read/write timeouts that example doesn't set.
///
/// DNS can resolve to multiple addresses (IPv4 + IPv6); each is tried in
/// order, returning the first that connects.
pub(super) fn connect_with_timeout(
    host: &str,
    port: u16,
) -> AppResult<imap::Client<native_tls::TlsStream<TcpStream>>> {
    let addrs = (host, port).to_socket_addrs().map_err(|e| {
        log::warn!("[email_watch] resolving {host}:{port} failed: {}", e.kind());
        AppError::Network("could not resolve the mail server".to_string())
    })?;

    let connector = TlsConnector::new()
        .map_err(|_| AppError::Network("could not initialize TLS".to_string()))?;

    let mut last_err = AppError::Network("could not resolve the mail server".to_string());
    for addr in addrs {
        let tcp = match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(tcp) => tcp,
            Err(e) => {
                log::warn!(
                    "[email_watch] TCP connect to {addr} ({host}:{port}) failed: {}",
                    e.kind()
                );
                last_err = AppError::Network("could not connect to the mail server".to_string());
                continue;
            }
        };
        // A failed `set_*_timeout` must drop this socket and move on to the
        // next resolved address — NOT log-and-proceed — so no socket ever
        // enters TLS/LOGIN without an I/O deadline (deferred LOW from the
        // PR A review; the pre-fix behavior let a `set_read_timeout`/
        // `set_write_timeout` failure fall through to `connector.connect`
        // below with no read/write timeout applied at all).
        if let Err(e) = tcp.set_read_timeout(Some(IO_TIMEOUT)) {
            log::warn!(
                "[email_watch] set_read_timeout for {host}:{port} failed: {}",
                e.kind()
            );
            last_err = AppError::Network("could not connect to the mail server".to_string());
            continue;
        }
        if let Err(e) = tcp.set_write_timeout(Some(IO_TIMEOUT)) {
            log::warn!(
                "[email_watch] set_write_timeout for {host}:{port} failed: {}",
                e.kind()
            );
            last_err = AppError::Network("could not connect to the mail server".to_string());
            continue;
        }

        let tls = match connector.connect(host, tcp) {
            Ok(tls) => tls,
            Err(e) => {
                log::warn!(
                    "[email_watch] TLS handshake with {host}:{port} failed: {}",
                    sanitize_reason(&e.to_string())
                );
                last_err = AppError::Network("could not connect to the mail server".to_string());
                continue;
            }
        };

        let mut client = imap::Client::new(tls);
        match client.read_greeting() {
            Ok(_) => return Ok(client),
            Err(e) => {
                log::warn!(
                    "[email_watch] IMAP greeting from {host}:{port} failed: {}",
                    error_kind(&e)
                );
                last_err = AppError::Network("could not connect to the mail server".to_string());
            }
        }
    }

    Err(last_err)
}

/// A short, content-free classification of an `imap::Error` for logging.
/// Deliberately NOT the error's `Display`/`Debug` — see [`validate_connection`]
/// for why. `imap::Error` is `#[non_exhaustive]` (and some variants are
/// feature-gated, e.g. `RustlsHandshake` doesn't exist under this crate's
/// `native-tls`-only build), so this always ends in a wildcard arm.
pub(super) fn error_kind(e: &imap::Error) -> &'static str {
    match e {
        imap::Error::Io(_) => "io",
        imap::Error::Tls(_) => "tls",
        imap::Error::TlsHandshake(_) => "tls-handshake",
        imap::Error::Bad(_) => "bad-response",
        imap::Error::No(_) => "no-response",
        imap::Error::Bye(_) => "bye-response",
        imap::Error::ConnectionLost => "connection-lost",
        imap::Error::Parse(_) => "parse",
        imap::Error::Validate(_) => "validate",
        imap::Error::Append => "append",
        imap::Error::Unexpected(_) => "unexpected-response",
        imap::Error::MissingStatusResponse => "missing-status-response",
        imap::Error::TagMismatch(_) => "tag-mismatch",
        imap::Error::StartTlsNotAvailable => "starttls-not-available",
        imap::Error::TlsNotConfigured => "tls-not-configured",
        _ => "other",
    }
}
