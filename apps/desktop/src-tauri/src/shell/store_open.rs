//! What happens when a persistent store fails to open during `setup`.
//!
//! `state::manage_opened` used to log and skip `manage`, so the first command that
//! reached for that store panicked with "state() called before manage()" and the
//! crash report had no hint of the real cause (#1525). The failure is now collected
//! at that one point ([`open_or_record`]); once every store has been attempted,
//! [`abort_if_failed`] reports it, locks the IPC surface, tells the user, and exits
//! non-zero.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use tauri::{App, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::crash_reporting;
use crate::error::AppResult;

pub(super) type Failures = Vec<(&'static str, &'static str)>;

/// `(store label, AppError::code())` of every store that failed to open.
static FAILURES: Mutex<Failures> = Mutex::new(Vec::new());

/// Set once a failure is being reported: every IPC command is refused from then on.
/// Never reset, so no test may set it.
static IPC_LOCKED: AtomicBool = AtomicBool::new(false);

/// Bounded wait for the Sentry flush.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(2);

/// The ONLY writer of the failure list.
fn record(label: &'static str, code: &'static str) {
    FAILURES.lock().push((label, code));
}

/// Unwrap an opened store, or log (path-free code only) and [`record`] the failure.
pub(super) fn open_or_record<T>(
    label: &'static str,
    what: &str,
    opened: AppResult<T>,
) -> Option<T> {
    match opened {
        Ok(store) => Some(store),
        Err(e) => {
            log::error!("[setup] {what} failed to open: {}", e.code());
            record(label, e.code());
            None
        }
    }
}

/// Drain the collected failures; `None` when every store opened.
pub(super) fn take_failures() -> Option<Failures> {
    let failures = std::mem::take(&mut *FAILURES.lock());
    (!failures.is_empty()).then_some(failures)
}

/// True once a store failed to open: the IPC handler must run nothing.
pub(super) fn ipc_locked() -> bool {
    IPC_LOCKED.load(Ordering::Relaxed)
}

/// The dialog body: every failed store with its path-free error code, and the data
/// folder (shown to the user only; never sent or logged). The cause is not known
/// beyond the code, so it points at the folder rather than guessing at a newer
/// version.
fn dialog_text(failures: &[(&str, &str)], data_dir: &Path) -> String {
    let list = failures
        .iter()
        .map(|(label, code)| format!("  - {label} ({code})"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "AI Job Hunter could not open its data and has to close.\n\n{list}\n\n\
         Data folder: {}\n\n\
         Check that this folder exists, that you can read and write it, and that its \
         files are intact (for example not locked by another program or a sync tool).",
        data_dir.display()
    )
}

/// The post-store decision of `setup`: when any store failed to open, run `abort` and
/// nothing else; otherwise run `start_services` (everything that reaches for stores).
pub(super) fn finish_setup<E>(
    failures: Option<Failures>,
    abort: impl FnOnce(Failures),
    start_services: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    match failures {
        Some(failures) => {
            abort(failures);
            Ok(())
        }
        None => start_services(),
    }
}

/// The abort sequence with its effects injected, so the ORDER is testable: lock IPC
/// first (no command may run from here on), then report, hide the window, and show
/// the dialog (which ends in the exit).
fn abort_sequence(
    failures: &[(&str, &str)],
    data_dir: &Path,
    lock_ipc: impl FnOnce(),
    report: impl FnOnce(&[(&str, &str)]),
    hide_window: impl FnOnce(),
    show_dialog_then_exit: impl FnOnce(String),
) {
    lock_ipc();
    report(failures);
    hide_window();
    show_dialog_then_exit(dialog_text(failures, data_dir));
}

/// Report the failures (one event), lock IPC, show the dialog and exit with code 1.
///
/// The dialog's blocking API must not run on the main thread (where `setup` runs),
/// and `setup` has no Tokio reactor, so a plain std thread owns the flush, the dialog
/// and the exit; the main thread keeps pumping the event loop that dialog needs.
pub(super) fn abort(app: &App, data_dir: &Path, failures: Failures) {
    abort_sequence(
        &failures,
        data_dir,
        || IPC_LOCKED.store(true, Ordering::Relaxed),
        crash_reporting::report_store_open_failures,
        || {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        },
        |text| {
            let handle = app.handle().clone();
            let spawned = std::thread::Builder::new()
                .name("store-open-failure".into())
                .spawn(move || {
                    // Before the dialog: the event must leave even if nobody dismisses it.
                    crash_reporting::flush(FLUSH_TIMEOUT);
                    let acknowledged = handle
                        .dialog()
                        .message(text)
                        .title("AI Job Hunter")
                        .kind(MessageDialogKind::Error)
                        .blocking_show();
                    if !acknowledged {
                        log::error!(
                            "[setup] store-open error dialog was closed or could not be shown"
                        );
                    }
                    std::process::exit(1);
                });
            if spawned.is_err() {
                log::error!("[setup] could not start the store-open error thread");
                crash_reporting::flush(FLUSH_TIMEOUT);
                std::process::exit(1);
            }
        },
    );
}

#[cfg(test)]
mod tests;
