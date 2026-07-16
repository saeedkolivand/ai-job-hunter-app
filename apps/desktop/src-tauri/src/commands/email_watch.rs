//! Email-confirmation watching (Task #23, auto-track Layer C) — the IPC seam
//! over [`crate::email_watch::EmailWatchStore`] + the connect-time IMAP
//! validation ([`crate::email_watch::imap_client`]).
//!
//! **PR A scope only**: connect/status/enable/disconnect, plus a manual
//! connectivity re-check. `email_watch_check_now` re-validates the stored
//! connection (`LOGIN` + `SELECT INBOX`) — it does NOT fetch or parse any
//! mail. That is a deliberate choice over a stub: it is honest (it does
//! exactly what its name says) and useful for debugging a broken app
//! password/host before the poller exists. The poller/parser/matcher land in
//! PR B.
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

/// Re-validate the existing connection (`LOGIN` + `SELECT INBOX`) using the
/// stored host/address and the keychain-stored app password. Fetches or
/// parses no mail — see the module doc. Errors if no account/credential is
/// configured yet.
#[tauri::command]
pub async fn email_watch_check_now(app: AppHandle) -> AppResult<EmailWatchStatus> {
    let store = store_or_err(&app)?;
    let account = store.account();
    let address = account
        .address
        .ok_or_else(|| AppError::Config("no email account is connected".to_string()))?;
    let host = account
        .host
        .unwrap_or_else(|| DEFAULT_IMAP_HOST.to_string());
    let port = account.port.unwrap_or(DEFAULT_IMAP_PORT);
    let app_password = credentials(&app)
        .lock()
        .get_decrypted(CREDENTIAL_SLOT)
        .map(|(_, password)| password)
        .ok_or_else(|| {
            AppError::Config("no app password is stored for this account".to_string())
        })?;

    validate_connection_blocking(host, port, address, app_password).await?;

    store.record_check(now_ms())?;
    Ok(store.status())
}

#[cfg(test)]
mod tests {
    use super::strip_ascii_whitespace;

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
}
