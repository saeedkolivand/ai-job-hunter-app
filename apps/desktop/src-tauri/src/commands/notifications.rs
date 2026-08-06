//! Notification Center commands — the IPC surface over [`crate::notifications`].
//!
//! Phase 2: the read/mutate seam over the pure, AppHandle-free
//! [`crate::notifications::NotificationStore`] (Phase 1). The store methods are
//! infallible, so these handlers are infallible too — the reads return the value
//! and the mutators return `()`, mirroring the simpler `applications_*` reads
//! rather than the fallible `{ success } | { error }` mutators.
//!
//! Every mutator emits [`crate::events::NOTIFICATIONS_CHANGED`]
//! (`notifications:changed`) so any live renderer inbox refetches.
//! [`notifications_clicked`] is the tray/renderer click target: it focuses the
//! main window (via [`crate::tray::show_focus`]) and emits
//! [`crate::events::NOTIFICATIONS_OPEN`] (`notifications:open`) so the renderer
//! opens the inbox.
//!
//! An **OS-banner** click reaches the same place without a renderer round-trip —
//! [`show_clickable_banner`] registers the native callback per platform. It used
//! to be routed through the notification plugin's JS `onAction`, which resolves
//! to a command the plugin registers only on mobile; that never worked on
//! desktop and is gone.

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{NotificationExt, PermissionState};

use crate::events::{emit_event, NOTIFICATIONS_CHANGED, NOTIFICATIONS_OPEN, NOTIFICATIONS_TOAST};
use crate::notifications::{
    AppNotification, NewNotification, NotificationOpen, NotificationRoute, NotificationStore,
};

fn store(app: &AppHandle) -> tauri::State<'_, NotificationStore> {
    app.state::<NotificationStore>()
}

/// Emit `notifications:changed` so a live renderer inbox refetches.
fn emit_changed(app: &AppHandle) {
    emit_event(app, NOTIFICATIONS_CHANGED, ());
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
pub fn show_os_notification(
    app: &AppHandle,
    title: &str,
    body: &str,
    route: Option<NotificationRoute>,
) {
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
    show_clickable_banner(app, title, body, route);
}

/// Focus the window and open the inbox — the same thing [`notifications_clicked`]
/// does, callable from a native notification callback where there is no renderer
/// round-trip.
fn open_from_banner(app: &AppHandle, route: Option<NotificationRoute>) {
    crate::tray::show_focus(app);
    emit_event(app, NOTIFICATIONS_OPEN, NotificationOpen { route });
}

/// Show the OS banner so that clicking its BODY opens the app.
///
/// Not `tauri_plugin_notification`'s `.show()`: its desktop implementation is
/// notify-rust fire-and-forget, and the `onAction` JS helper it documents for
/// clicks resolves to `plugin:notification|register_listener`, which the plugin
/// registers **only on mobile** (its desktop `invoke_handler` has exactly
/// `notify`, `request_permission`, `is_permission_granted`). So the renderer's
/// subscription failed with "Command not found" at every startup and an OS-banner
/// click reached nothing — reported as "clicking a Windows notification doesn't
/// open the app", and visible in the log file only after the renderer console
/// bridge landed.
///
/// Each platform needs a different mechanism, and only Windows is callback-based:
///
/// - **Windows** — `tauri-winrt-notification`'s `on_activated` fires on a WinRT
///   event handler, so this returns immediately.
/// - **macOS / Linux** — notify-rust's `wait_for_action` BLOCKS the calling
///   thread until the user acts or the banner closes, so it must not run on a
///   runtime worker. Hence `spawn_blocking`.
///
/// Shared limitation, all platforms: the callback lives in THIS process, so it
/// only fires while the app is running. A click after the app exits does nothing
/// (that needs a COM activator on Windows / a launch agent elsewhere) — out of
/// scope, and not the reported case.
fn show_clickable_banner(
    app: &AppHandle,
    title: &str,
    body: &str,
    route: Option<NotificationRoute>,
) {
    #[cfg(windows)]
    {
        // Same app id the plugin uses (notify-rust passes `config().identifier`
        // straight through to this crate), so the toast keeps its existing
        // identity, icon and Action Center grouping.
        let handle = app.clone();
        let toast = tauri_winrt_notification::Toast::new(&app.config().identifier)
            .title(title)
            .text1(body)
            // `FnMut`, so the route is cloned per activation rather than moved.
            .on_activated(move |_action| {
                open_from_banner(&handle, route.clone());
                Ok(())
            });
        if let Err(e) = toast.show() {
            log::warn!("[notifications] windows toast failed: {e}");
        }
    }

    #[cfg(not(windows))]
    {
        let handle = app.clone();
        let (title, body) = (title.to_string(), body.to_string());
        // `wait_for_action` blocks; keep it off the async runtime's workers.
        tauri::async_runtime::spawn_blocking(move || {
            let mut n = notify_rust::Notification::new();
            n.summary(&title).body(&body);
            // Linux: a body click is delivered as the `default` action, and only
            // when the notification declares one. Harmless on macOS, which maps a
            // body click to "default" itself.
            #[cfg(target_os = "linux")]
            n.action("default", "default");
            match n.show() {
                Ok(handle_n) => handle_n.wait_for_action(|action| {
                    if action == "default" {
                        open_from_banner(&handle, route);
                    }
                }),
                Err(e) => log::warn!("[notifications] os banner failed: {e}"),
            }
        });
    }
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
        emit_event(
            app,
            NOTIFICATIONS_TOAST,
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
        show_os_notification(app, &rec.title, &rec.body, rec.route.clone());
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
    // No route: a tray / renderer click means "open the inbox", not "go to one
    // specific thing". Only an OS-banner click knows which notification it was.
    open_from_banner(&app, None);
}
