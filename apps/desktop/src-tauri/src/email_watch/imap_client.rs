//! Thin IMAP connector — the ONLY raw TLS socket construction in this crate.
//!
//! R5 pins `reqwest::Client` construction to `net/http.rs`, but IMAP is not
//! HTTP: it can't ride `reqwest` (no IMAP support), so this is a deliberate new
//! network primitive living in its own small, reviewed module (mirrors the
//! existing raw-socket precedents — the extension-bridge's loopback
//! `tokio-tungstenite` server and `chromiumoxide`'s CDP socket). Egress here is
//! **solely** the user-configured mail host (data-driven; defaults to
//! [`DEFAULT_IMAP_HOST`]/[`DEFAULT_IMAP_PORT`], v1 is Gmail-branded), gated
//! behind the opt-in `email_watch` feature (default OFF) — never a third-party
//! endpoint.
//!
//! TLS is `native-tls` (the `imap` crate's default backend — SChannel/
//! SecureTransport/OpenSSL per OS), NOT `rustls-tls`: that feature bridges
//! through `rustls-connector`, which is pinned to `rustls 0.22`/
//! `rustls-webpki 0.102` with no semver-compatible update path (see the
//! Cargo.toml comment on this dependency) — 4 live RUSTSEC advisories
//! `cargo deny check` fails on. `native-tls` instead reuses the SAME per-OS
//! TLS stack `net/http.rs` already depends on for `reqwest` — not a second TLS
//! stack, just a second consumer of the one already in the tree.
//!
//! **PR A** exposes exactly the seam the connect/check-now commands need:
//! [`validate_connection`] — TLS connect, `LOGIN`, `SELECT INBOX`, then log
//! out. It proves a host/address/app-password combination actually works
//! before anything is persisted, and is called again (unchanged) by
//! `email_watch_check_now` to re-validate an existing connection. PR B extends
//! this file with `UID SEARCH SINCE` + header/body fetch for the poller — the
//! connect/select step here stays as-is; the sync `imap` crate has no async
//! API, so a persistent session can't be held across ticks anyway (PR B opens
//! a fresh connection per tick, same as this function does).
//!
//! **Blocking**: every call here is synchronous — callers MUST run it inside
//! `tokio::task::spawn_blocking`, never directly on an async worker.

use crate::error::{AppError, AppResult};

/// Default IMAP host/port — v1's Settings UI is Gmail-branded, but the value
/// is DATA (stored in `EmailWatchStore`'s `account` row), not hardcoded into
/// this connector, so a future non-Gmail provider needs no code change here.
pub const DEFAULT_IMAP_HOST: &str = "imap.gmail.com";
pub const DEFAULT_IMAP_PORT: u16 = 993;

/// TLS-connect to `host:port`, `LOGIN` with `address`/`app_password`, and
/// `SELECT INBOX` to prove the mailbox is reachable — then best-effort log
/// out. Returns `Ok(())` only if every step succeeds; the caller must not
/// persist the account/credential on `Err`.
///
/// Blocking (the `imap` crate has no async API) — call only from
/// `spawn_blocking`.
///
/// Privacy: never logs `address`/`app_password`. A failure logs the host/port
/// plus a content-free error-KIND label only (see [`error_kind`]) — never the
/// library error's `Display`/`Debug` text, which can echo server-controlled
/// content (some IMAP servers echo the attempted username/address back into a
/// `NO`/`BAD` response's message).
pub fn validate_connection(
    host: &str,
    port: u16,
    address: &str,
    app_password: &str,
) -> AppResult<()> {
    let client = imap::ClientBuilder::new(host, port)
        .connect()
        .map_err(|e| {
            log::warn!(
                "[email_watch] IMAP connect to {host}:{port} failed: {}",
                error_kind(&e)
            );
            AppError::Network("could not connect to the mail server".to_string())
        })?;

    let mut session = client
        .login(address, app_password)
        .map_err(|(e, _client)| {
            log::warn!(
                "[email_watch] IMAP login against {host}:{port} failed: {}",
                error_kind(&e)
            );
            AppError::Config(
                "sign-in failed — check the email address and app password".to_string(),
            )
        })?;

    let select_result = session.select("INBOX").map(|_| ()).map_err(|e| {
        log::warn!(
            "[email_watch] IMAP SELECT INBOX against {host}:{port} failed: {}",
            error_kind(&e)
        );
        AppError::Provider("could not open the mailbox inbox".to_string())
    });

    // Best-effort logout regardless of the SELECT outcome — a logout failure
    // must never mask (or be conflated with) the real result above.
    let _ = session.logout();

    select_result
}

/// A short, content-free classification of an `imap::Error` for logging.
/// Deliberately NOT the error's `Display`/`Debug` — see [`validate_connection`]
/// for why. `imap::Error` is `#[non_exhaustive]` (and some variants are
/// feature-gated, e.g. `RustlsHandshake` doesn't exist under this crate's
/// `native-tls`-only build), so this always ends in a wildcard arm.
fn error_kind(e: &imap::Error) -> &'static str {
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

// No automated test here: this function's every branch is a real network
// round-trip against an IMAP server (TLS handshake, LOGIN, SELECT) — there is
// no fake IMAP server in this crate's test harness, and standing one up is
// out of scope for PR A (mirrors the project's own documented gap: IMAP
// integration is manual smoke, tracked for PR B alongside the poller/parser).
// The store-level logic this wraps (EmailWatchStore) is covered in `tests.rs`.
