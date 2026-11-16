//! Crash-triage context that is not a crash itself: the handled "a store failed to
//! open" event, the `ipc.command` tag and the `install.channel` tag. Everything here
//! reaches Sentry only through the existing pipeline (`redact_event`, then the
//! consent-gated wire gate in [`super::transport`]); when no client is bound (no
//! consent or no DSN, so [`super::init`] never ran `sentry::init`) every call is a
//! no-op. None of it carries a path, an error message or user data.

use std::time::Duration;

use sentry::protocol::{Event, Level};

/// Longest `stores` tag value we send (Sentry itself caps tag values at 200).
const STORES_TAG_MAX: usize = 200;

/// Longest command name we tag; real command names are far shorter.
const IPC_COMMAND_MAX: usize = 64;

/// Build the ONE event for a launch whose stores failed to open. An unwritable data
/// dir fails every store at once, so one event per store would spend the event quota
/// 14 times over. Label and [`AppError::code`] only: both are static, path-free
/// identifiers. The first failure drives the tags and the fingerprint (grouping per
/// store and cause); `store_count` and `stores` describe the rest. `None` when
/// nothing failed.
///
/// [`AppError::code`]: crate::error::AppError::code
pub(crate) fn store_open_failed_event(failures: &[(&str, &str)]) -> Option<Event<'static>> {
    let &(label, code) = failures.first()?;
    let stores: String = failures
        .iter()
        .map(|(l, _)| *l)
        .collect::<Vec<_>>()
        .join(",")
        .chars()
        .take(STORES_TAG_MAX)
        .collect();
    let mut event = Event {
        level: Level::Error,
        message: Some("store failed to open".into()),
        fingerprint: vec![
            "store-open-failed".into(),
            label.to_owned().into(),
            code.to_owned().into(),
        ]
        .into(),
        ..Default::default()
    };
    event.tags.insert("store".into(), label.to_owned());
    event.tags.insert("error_code".into(), code.to_owned());
    event
        .tags
        .insert("store_count".into(), failures.len().to_string());
    event.tags.insert("stores".into(), stores);
    Some(event)
}

/// Report the store-open failures of this launch as one event (a no-op when crash
/// reporting is off).
pub(crate) fn report_store_open_failures(failures: &[(&str, &str)]) {
    if let Some(event) = store_open_failed_event(failures) {
        sentry::capture_event(event);
    }
}

/// Deliver queued events before the process exits, bounded by `timeout`.
pub(crate) fn flush(timeout: Duration) {
    if let Some(client) = sentry::Hub::main().client() {
        client.flush(Some(timeout));
    }
}

/// Whether `command` is safe to attach as a tag: a plain identifier-shaped name.
/// Anything else is skipped rather than sanitised.
fn is_taggable_command(command: &str) -> bool {
    !command.is_empty()
        && command.len() <= IPC_COMMAND_MAX
        && command
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Run `f` with the Sentry tag `ipc.command` set to `command` on the CURRENT
/// scope, removing it afterwards. A name that is not identifier-shaped is not tagged.
///
/// Scope facts that bound what this can promise:
/// * The main thread's hub IS the process hub, so its scope is shared, not
///   thread-local. Other threads get their own hub lazily, cloned from it.
/// * A synchronous command runs on the dispatching thread, inside `f`, so a panic
///   there carries the tag. With `panic = "abort"` in release a panic can never
///   outlive the call, so the tag cannot be left behind and no Drop guard is needed.
/// * An async command only has its future created inside `f`; it runs later on a
///   worker whose scope never saw the tag, so a panic there is untagged. Not covered.
/// * A worker thread whose hub is created lazily while the main thread is inside a
///   sync command clones that scope, tag included, and may mis-tag its later events.
///   Accepted as narrow, and why the value is validated above.
pub(crate) fn with_ipc_command<R>(command: &str, f: impl FnOnce() -> R) -> R {
    if !is_taggable_command(command) {
        return f();
    }
    sentry::configure_scope(|scope| scope.set_tag("ipc.command", command));
    let out = f();
    sentry::configure_scope(|scope| scope.remove_tag("ipc.command"));
    out
}

/// Tag every later event with the install channel; called once at Sentry init.
pub(crate) fn set_install_channel() {
    let channel = crate::platform::install_channel::detect();
    sentry::configure_scope(|scope| scope.set_tag("install.channel", channel));
}

#[cfg(test)]
mod tests;
