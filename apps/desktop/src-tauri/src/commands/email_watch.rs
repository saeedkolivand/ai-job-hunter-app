//! Email-confirmation watching (Task #23, auto-track Layer C) — the IPC seam
//! over [`crate::email_watch::EmailWatchStore`] + the connect-time IMAP
//! validation ([`crate::email_watch::imap_client`]).
//!
//! `email_watch_check_now` runs the SAME fetch+parse+match+notify pass the
//! background scheduler runs (`crate::email_watch_scheduler::run_check`),
//! gated by its own [`MIN_CHECK_NOW_GAP_MS`] guard — a typed
//! [`AppError::RateLimited`] refusal, never a silent no-op, so a renderer bug
//! or a determined caller can't spam Gmail logins (a `tauri-security-reviewer`
//! requirement carried into PR B).
//!
//! Every IMAP call runs inside `spawn_blocking` — never directly on the async
//! runtime (the `imap` crate is fully synchronous). Wire-error discipline
//! mirrors `extension_bridge::status_update`: the app password is never
//! logged or returned; a validation failure surfaces a fixed, non-credential
//! sentinel message (the concrete IMAP detail is logged host-only in
//! `imap_client`).

use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

use crate::credentials::CredentialStore;
use crate::db::now_ms;
use crate::email_watch::imap_client::{validate_connection, DEFAULT_IMAP_HOST, DEFAULT_IMAP_PORT};
use crate::email_watch::{EmailWatchStatus, EmailWatchStore, CREDENTIAL_SLOT};
use crate::error::{AppError, AppResult};

/// Resolve the managed `EmailWatchStore`, or a typed error when it isn't
/// managed (the boot-time `EmailWatchStore::open` failed — a non-fatal,
/// logged-and-continue startup path, see `lib.rs::setup`). Using `try_state`
/// (never the panicking `state()`) is load-bearing here: unlike
/// `email_watch_status` (which degrades to a default status), the four
/// mutating commands below must reject with an `AppError` instead of
/// panicking the whole `invoke` call — a panic never resolves/rejects the
/// renderer's promise, leaving the UI stuck.
fn store_or_err(app: &AppHandle) -> AppResult<tauri::State<'_, EmailWatchStore>> {
    app.try_state::<EmailWatchStore>()
        .ok_or_else(|| AppError::Storage("email watch is unavailable".to_string()))
}

fn credentials(app: &AppHandle) -> tauri::State<'_, Mutex<CredentialStore>> {
    app.state::<Mutex<CredentialStore>>()
}

/// Remove ASCII whitespace from `s` (Google's copy/paste app-password format
/// is 4 space-separated groups, e.g. `"abcd efgh ijkl mnop"`). Pure so it's
/// cheaply unit-testable without a Tauri harness; see [`email_watch_connect`]
/// for why this is scoped to the default Gmail host only.
fn strip_ascii_whitespace(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii_whitespace()).collect()
}

/// Run [`validate_connection`] on the blocking pool and flatten the
/// `spawn_blocking` join outcome into the same `AppResult` the caller already
/// works with. A join failure (task panic) is logged with the host only —
/// the renderer gets a fixed sentinel, never the panic payload.
async fn validate_connection_blocking(
    host: String,
    port: u16,
    address: String,
    app_password: String,
) -> AppResult<()> {
    let host_for_log = host.clone();
    match tokio::task::spawn_blocking(move || {
        validate_connection(&host, port, &address, &app_password)
    })
    .await
    {
        Ok(result) => result,
        Err(e) => {
            log::warn!("[email_watch] connection check task against {host_for_log} panicked: {e}");
            Err(AppError::Message(
                "connection check failed unexpectedly".to_string(),
            ))
        }
    }
}

#[tauri::command]
pub async fn email_watch_status(app: AppHandle) -> EmailWatchStatus {
    match app.try_state::<EmailWatchStore>() {
        Some(store) => store.status(),
        None => EmailWatchStatus::default(),
    }
}

/// Validate `address`/`app_password` against the stored (or default Gmail)
/// host, then persist the account row + the app password in the OS keychain.
/// Nothing is persisted when validation fails.
#[tauri::command]
pub async fn email_watch_connect(
    app: AppHandle,
    address: String,
    app_password: String,
) -> AppResult<EmailWatchStatus> {
    let address = address.trim().to_string();
    if address.is_empty() || app_password.is_empty() {
        return Err(AppError::Validation(
            "email address and app password are required".to_string(),
        ));
    }

    let existing = store_or_err(&app)?.account();
    let host = existing
        .host
        .unwrap_or_else(|| DEFAULT_IMAP_HOST.to_string());
    let port = existing.port.unwrap_or(DEFAULT_IMAP_PORT);

    // Google displays an app password as 4 space-separated groups ("abcd efgh
    // ijkl mnop") and users commonly paste it with the spaces intact; strip
    // them before validating/storing. Scoped to the default Gmail host only
    // — a future custom IMAP host's password could legitimately contain
    // meaningful whitespace, and stripping it there would corrupt a valid
    // credential.
    let app_password = if host == DEFAULT_IMAP_HOST {
        strip_ascii_whitespace(&app_password)
    } else {
        app_password
    };

    validate_connection_blocking(host.clone(), port, address.clone(), app_password.clone()).await?;

    credentials(&app)
        .lock()
        .set(CREDENTIAL_SLOT, &address, &app_password)?;
    let store = store_or_err(&app)?;
    store.connect(&address, &host, port)?;
    store.record_check(now_ms())?;

    Ok(store.status())
}

/// Remove the stored app password and clear the account row (does NOT touch
/// `seen` dedupe rows semantically differently than a factory reset — both go
/// through the same [`crate::email_watch::EmailWatchStore::clear`]).
#[tauri::command]
pub async fn email_watch_disconnect(app: AppHandle) -> AppResult<EmailWatchStatus> {
    credentials(&app).lock().remove(CREDENTIAL_SLOT)?;
    let store = store_or_err(&app)?;
    store.clear()?;
    Ok(store.status())
}

#[tauri::command]
pub async fn email_watch_set_enabled(app: AppHandle, enabled: bool) -> AppResult<EmailWatchStatus> {
    let store = store_or_err(&app)?;
    store.set_enabled(enabled)?;
    Ok(store.status())
}

/// Minimum gap between two `email_watch_check_now` invocations — measured
/// against `last_check_ms` (stamped by ANY check, manual or scheduled), so a
/// renderer bug/loop can't spam Gmail logins. Refuses with
/// [`AppError::RateLimited`] rather than silently no-oping, so the renderer
/// can surface (and retry) the refusal.
const MIN_CHECK_NOW_GAP_MS: u64 = 60_000;

/// Whether a fresh `email_watch_check_now` call must be refused because the
/// last recorded check (manual or scheduled) is under [`MIN_CHECK_NOW_GAP_MS`]
/// old. Pure so it's cheaply unit-testable without a Tauri harness.
fn is_check_now_rate_limited(last_check_ms: Option<u64>, now_ms: u64) -> bool {
    match last_check_ms {
        None => false,
        Some(last) => now_ms.saturating_sub(last) < MIN_CHECK_NOW_GAP_MS,
    }
}

/// Run a real fetch+parse+match+notify pass (`crate::email_watch_scheduler::
/// run_check` — the SAME pass the background scheduler runs) against the
/// stored connection, gated by [`MIN_CHECK_NOW_GAP_MS`]. Errors if no
/// account/credential is configured yet, or if a check already ran too
/// recently.
#[tauri::command]
pub async fn email_watch_check_now(app: AppHandle) -> AppResult<EmailWatchStatus> {
    let store = store_or_err(&app)?;
    if is_check_now_rate_limited(store.account().last_check_ms, now_ms()) {
        // Same literal `run_check`'s own concurrent-run guard rejects with —
        // single source of truth, see `email_watch_scheduler::RATE_LIMITED_MESSAGE`.
        return Err(AppError::RateLimited(
            crate::email_watch_scheduler::RATE_LIMITED_MESSAGE.to_string(),
        ));
    }
    crate::email_watch_scheduler::run_check(&app).await
}

#[cfg(test)]
mod tests {
    use super::{is_check_now_rate_limited, strip_ascii_whitespace, MIN_CHECK_NOW_GAP_MS};

    #[test]
    fn strip_ascii_whitespace_removes_googles_4_group_spacing() {
        assert_eq!(
            strip_ascii_whitespace("abcd efgh ijkl mnop"),
            "abcdefghijklmnop"
        );
    }

    #[test]
    fn strip_ascii_whitespace_is_a_no_op_without_whitespace() {
        assert_eq!(
            strip_ascii_whitespace("abcdefghijklmnop"),
            "abcdefghijklmnop"
        );
    }

    #[test]
    fn strip_ascii_whitespace_also_removes_leading_trailing_and_tabs() {
        assert_eq!(strip_ascii_whitespace("  ab\tcd\n"), "abcd");
    }

    #[test]
    fn check_now_is_never_rate_limited_when_nothing_has_run_yet() {
        assert!(!is_check_now_rate_limited(None, 1_000_000));
    }

    #[test]
    fn check_now_is_rate_limited_within_the_min_gap() {
        let now = 1_000_000_000u64;
        let last = now - (MIN_CHECK_NOW_GAP_MS / 2);
        assert!(is_check_now_rate_limited(Some(last), now));
    }

    #[test]
    fn check_now_is_allowed_once_the_min_gap_has_elapsed() {
        let now = 1_000_000_000u64;
        let last = now - MIN_CHECK_NOW_GAP_MS;
        assert!(!is_check_now_rate_limited(Some(last), now));
    }
}
