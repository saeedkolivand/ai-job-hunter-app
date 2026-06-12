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

use crate::notifications::{AppNotification, NotificationStore};

/// Renderer event: the notification list changed (push / read / remove / clear).
/// A live inbox refetches on this.
const CHANGED_EVENT: &str = "notifications:changed";

/// Renderer event: open the notification inbox (OS-banner / tray click target).
const OPEN_EVENT: &str = "notifications:open";

fn store(app: &AppHandle) -> tauri::State<'_, NotificationStore> {
    app.state::<NotificationStore>()
}

/// Emit `notifications:changed` so a live renderer inbox refetches.
fn emit_changed(app: &AppHandle) {
    let _ = app.emit(CHANGED_EVENT, ());
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
