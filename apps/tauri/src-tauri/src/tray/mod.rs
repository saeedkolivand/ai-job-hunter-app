//! System tray + the "new jobs" notification surface (L3 shell).
//!
//! Autopilot is a background discovery agent: when a scheduled run surfaces
//! brand-new postings the user isn't looking at the app, so we (1) raise a
//! permission-gated OS notification and (2) bump a "New jobs: N" tray counter.
//! Clicking that counter focuses the window and emits `autopilot.focus` so the
//! renderer jumps to the autopilot whose run produced the newest finds.

use std::sync::Arc;

use parking_lot::Mutex;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_notification::{NotificationExt, PermissionState};

use crate::autopilot::{AutopilotStatus, AutopilotStore};

const SHOW_ID: &str = "tray_show";
const NEW_JOBS_ID: &str = "tray_new_jobs";
const PAUSE_ALL_ID: &str = "tray_pause_all";
const QUIT_ID: &str = "tray_quit";

/// Renderer event: focus an autopilot's found-jobs panel. An empty `autopilotId`
/// is a pure "refresh autopilots" signal (e.g. after a tray Pause-All) with no
/// navigation.
const FOCUS_EVENT: &str = "autopilot.focus";

/// Tray-owned state: the dynamic "New jobs" menu item handle plus the running
/// unseen-jobs total and which autopilot to focus when the user clicks it.
pub struct TrayState {
    new_jobs_item: MenuItem<Wry>,
    inner: Mutex<NewJobs>,
}

#[derive(Default)]
struct NewJobs {
    total: u32,
    last_autopilot: Option<String>,
}

/// Build the tray icon + menu and register `TrayState`. Called once from setup.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id(SHOW_ID, "Show AI Job Hunter").build(app)?;
    // Non-clickable until a run surfaces new jobs (label updates in place).
    let new_jobs = MenuItemBuilder::with_id(NEW_JOBS_ID, "New jobs: 0")
        .enabled(false)
        .build(app)?;
    let pause_all_item =
        MenuItemBuilder::with_id(PAUSE_ALL_ID, "Pause all autopilots").build(app)?;
    let quit = MenuItemBuilder::with_id(QUIT_ID, "Quit").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&new_jobs)
        .item(&pause_all_item)
        .separator()
        .item(&quit)
        .build()?;

    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("AI Job Hunter")
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_ID => show_focus(app),
            NEW_JOBS_ID => on_new_jobs_click(app),
            PAUSE_ALL_ID => pause_all(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_focus(tray.app_handle());
            }
        })
        .build(app)?;

    app.manage(TrayState {
        new_jobs_item: new_jobs,
        inner: Mutex::new(NewJobs::default()),
    });
    Ok(())
}

fn show_focus(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// After a run surfaces `new_count` brand-new jobs (>0): raise a permission-gated
/// notification and bump the tray "New jobs: N" counter, remembering which
/// autopilot to focus when the user clicks it.
pub fn on_new_jobs(app: &AppHandle, autopilot_id: &str, autopilot_name: &str, new_count: u32) {
    if new_count == 0 {
        return;
    }
    notify(app, autopilot_name, new_count);
    if let Some(state) = app.try_state::<TrayState>() {
        let total = {
            let mut g = state.inner.lock();
            g.total = g.total.saturating_add(new_count);
            g.last_autopilot = Some(autopilot_id.to_string());
            g.total
        };
        let _ = state.new_jobs_item.set_text(format!("New jobs: {total}"));
    }
}

/// Emit a focus event so the renderer jumps to a specific autopilot's found-jobs
/// panel (or, with an empty id, just refreshes the list). Shared by the tray and
/// the single-instance deep-link guard.
pub fn emit_focus(app: &AppHandle, autopilot_id: &str) {
    let _ = app.emit(
        FOCUS_EVENT,
        serde_json::json!({ "autopilotId": autopilot_id }),
    );
}

fn notify(app: &AppHandle, name: &str, count: u32) {
    // Permission gate: send only if granted; request once when not, skip on deny.
    let granted = matches!(
        app.notification().permission_state(),
        Ok(PermissionState::Granted)
    ) || matches!(
        app.notification().request_permission(),
        Ok(PermissionState::Granted)
    );
    if !granted {
        return;
    }
    let body = if count == 1 {
        format!("1 new job for “{name}”")
    } else {
        format!("{count} new jobs for “{name}”")
    };
    let _ = app
        .notification()
        .builder()
        .title("AI Job Hunter")
        .body(body)
        .show();
}

fn on_new_jobs_click(app: &AppHandle) {
    show_focus(app);
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    let target = {
        let mut g = state.inner.lock();
        let t = g.last_autopilot.take();
        g.total = 0;
        t
    };
    let _ = state.new_jobs_item.set_text("New jobs: 0");
    if let Some(id) = target {
        emit_focus(app, &id);
    }
}

fn pause_all(app: &AppHandle) {
    let Some(store) = app.try_state::<Arc<Mutex<AutopilotStore>>>() else {
        return;
    };
    let store = store.inner().clone();
    let ids: Vec<String> = store.lock().list().into_iter().map(|a| a.id).collect();
    for id in ids {
        store.lock().set_status(&id, AutopilotStatus::Paused);
    }
    // Empty id = "refresh autopilots" so the renderer reflects the pause.
    emit_focus(app, "");
}
