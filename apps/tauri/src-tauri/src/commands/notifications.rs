//! Notification Center commands — the IPC surface over [`crate::notifications`].
//!
//! Phase 2: the read/mutate seam over the pure, AppHandle-free
//! [`crate::notifications::NotificationStore`] (Phase 1). The store methods are
//! infallible, so these handlers are infallible too — the reads return the value
//! and the mutators return `()`, mirroring the simpler `applications_*` reads
//! rather than the fallible `{ success } | { error }` mutators.
//!
//! Every mutator emits [`CHANGED_EVENT`] (`notifications:changed`) so any live
//! renderer inbox refetches. [`notifications_clicked`] is the unified OS-banner /
//! tray click target: it focuses the main window (via [`crate::tray::show_focus`])
//! and emits [`OPEN_EVENT`] (`notifications:open`) so the renderer opens the
//! inbox. (Phase 4 repoints the global notification `onAction` at it.)

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::{NotificationExt, PermissionState};

use crate::notifications::{AppNotification, NewNotification, NotificationStore};

/// Renderer event: the notification list changed (push / read / remove / clear).
/// A live inbox refetches on this.
const CHANGED_EVENT: &str = "notifications:changed";

/// Renderer event: open the notification inbox (OS-banner / tray click target).
const OPEN_EVENT: &str = "notifications:open";

/// Renderer event: show an in-app toast for a just-pushed notification. Fired by
/// [`push_and_notify`] only when the main window is focused (so an OS banner
/// would be redundant). Payload is `{ title, body, route }` (camelCase) lifted
/// from the freshly-created record, so the renderer's toast can offer a working
/// "View" without refetching.
const TOAST_EVENT: &str = "notifications:toast";

fn store(app: &AppHandle) -> tauri::State<'_, NotificationStore> {
    app.state::<NotificationStore>()
}

/// Emit `notifications:changed` so a live renderer inbox refetches.
fn emit_changed(app: &AppHandle) {
    let _ = app.emit(CHANGED_EVENT, ());
}

// ── Push orchestration (Phase 4a) ────────────────────────────────────────────

/// OS-banner policy for [`push_and_notify`]. Decoupled from the persisted record
/// (the banner is a delivery concern, not part of the notification) so each
/// source picks the right UX:
/// - [`OsBanner::Always`] — banner even when the window is focused (autopilot:
///   a background run finished, the user wants the OS-level nudge regardless).
/// - [`OsBanner::WhenUnfocused`] — banner only when the window is NOT focused;
///   when focused the in-app toast already covers it (extension import intent).
/// - [`OsBanner::Never`] — inbox + toast only, no OS banner.
pub enum OsBanner {
    Always,
    WhenUnfocused,
    Never,
}

/// The single permission-gated OS-notification path. Sends only when permission
/// is granted; when the state is already `Denied` it skips silently WITHOUT
/// re-requesting (a denied permission is terminal). Any other state (`Prompt` /
/// `PromptWithRationale`, or a read error) triggers a one-time request and shows
/// only if that returns `Granted`. Used by [`push_and_notify`]; the tray's
/// autopilot banner routes through here too, so there is exactly ONE gated
/// `.show()` call site in the app.
pub fn show_os_notification(app: &AppHandle, title: &str, body: &str) {
    // Permission gate: send when granted; skip silently (no re-request) when
    // already denied; otherwise request once and show only if that grants.
    let granted = match app.notification().permission_state() {
        Ok(PermissionState::Granted) => true,
        Ok(PermissionState::Denied) => false,
        _ => matches!(
            app.notification().request_permission(),
            Ok(PermissionState::Granted)
        ),
    };
    if !granted {
        return;
    }
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Push a notification and deliver it across every surface, best-effort
/// (infallible, like the store): persist it, refresh a live inbox, raise an
/// in-app toast when the window is focused, and raise an OS banner per `banner`.
///
/// Ordering:
/// 1. `store.push(input)` — persist + assign id/created_at (source of truth).
/// 2. `notifications:changed` — a live inbox refetches.
/// 3. If the main window is focused, `notifications:toast` with the new record's
///    `{ title, body, route }` so the renderer shows an in-app toast.
/// 4. OS banner when `Always`, or `WhenUnfocused` && the window is NOT focused.
///    (`Always` fires even while focused — that is the autopilot intent.)
pub fn push_and_notify(app: &AppHandle, input: NewNotification, banner: OsBanner) {
    let rec = store(app).push(input);
    emit_changed(app);

    let focused = app
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);

    if focused {
        let _ = app.emit(
            TOAST_EVENT,
            serde_json::json!({
                "title": rec.title,
                "body": rec.body,
                "route": rec.route,
            }),
        );
    }

    let show_banner = matches!(banner, OsBanner::Always)
        || (matches!(banner, OsBanner::WhenUnfocused) && !focused);
    if show_banner {
        show_os_notification(app, &rec.title, &rec.body);
    }
}

#[tauri::command]
pub async fn notifications_list(app: AppHandle) -> Vec<AppNotification> {
    store(&app).list()
}

#[tauri::command]
pub async fn notifications_mark_read(app: AppHandle, id: String) {
    if store(&app).mark_read(&id) {
        emit_changed(&app);
    }
}

#[tauri::command]
pub async fn notifications_mark_all_read(app: AppHandle) {
    store(&app).mark_all_read();
    emit_changed(&app);
}

#[tauri::command]
pub async fn notifications_remove(app: AppHandle, id: String) {
    if store(&app).remove(&id) {
        emit_changed(&app);
    }
}

#[tauri::command]
pub async fn notifications_clear_all(app: AppHandle) {
    store(&app).clear_all();
    emit_changed(&app);
}

/// Unified OS-banner / tray click target: focus the main window (the shared
/// reopen path used by the tray) and emit `notifications:open` so the renderer
/// opens the inbox. Phase 4 repoints the global notification `onAction` here.
#[tauri::command]
pub fn notifications_clicked(app: AppHandle) {
    crate::tray::show_focus(&app);
    let _ = app.emit(OPEN_EVENT, ());
}
