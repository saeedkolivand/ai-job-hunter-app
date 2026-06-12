//! System tray + the "new jobs" notification surface (L3 shell).
//!
//! Autopilot is a background discovery agent: when a scheduled run surfaces
//! brand-new postings the user isn't looking at the app, so we (1) raise a
//! permission-gated OS notification and (2) bump a "New jobs: N" tray counter.
//! Clicking that counter focuses the window and emits `autopilot:focus` so the
//! renderer jumps to the autopilot whose run produced the newest finds.

use std::sync::Arc;

use parking_lot::Mutex;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::autopilot::{AutopilotStatus, AutopilotStore};
use crate::commands::notifications::{push_and_notify, OsBanner};
use crate::notifications::{NewNotification, NotificationRoute};

const SHOW_ID: &str = "tray_show";
const SETTINGS_ID: &str = "tray_settings";
const CHECK_UPDATES_ID: &str = "tray_check_updates";
const NEW_JOBS_ID: &str = "tray_new_jobs";
const PAUSE_ALL_ID: &str = "tray_pause_all";
const QUIT_ID: &str = "tray_quit";

/// Renderer contract events emitted by the tray (mirrors the app-menu items).
/// `menu:navigate` → `{ route, section }`; `menu:action` → `{ action }`.
const NAVIGATE_EVENT: &str = "menu:navigate";
const ACTION_EVENT: &str = "menu:action";

/// Renderer event: focus an autopilot's found-jobs panel. An empty `autopilotId`
/// is a pure "refresh autopilots" signal (e.g. after a tray Pause-All) with no
/// navigation.
const FOCUS_EVENT: &str = "autopilot:focus";

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

/// The last menu intent (navigate/action), buffered by `dispatch_menu` and
/// pulled by the renderer (`menu_take_pending`) so it isn't lost to the
/// fire-and-forget `emit` race. Single-slot (last click wins) holding
/// `(event_name, payload)`; the renderer takes-and-clears it atomically.
pub struct PendingMenu(pub Mutex<Option<(String, serde_json::Value)>>);

/// Build the tray icon + menu and register `TrayState`. Called once from setup.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id(SHOW_ID, "Show AI Job Hunter").build(app)?;
    let settings = MenuItemBuilder::with_id(SETTINGS_ID, "Settings").build(app)?;
    let check_updates =
        MenuItemBuilder::with_id(CHECK_UPDATES_ID, "Check for Updates").build(app)?;
    // Non-clickable until a run surfaces new jobs (label updates in place).
    let new_jobs = MenuItemBuilder::with_id(NEW_JOBS_ID, "New jobs: 0")
        .enabled(false)
        .build(app)?;
    let pause_all_item =
        MenuItemBuilder::with_id(PAUSE_ALL_ID, "Pause all autopilots").build(app)?;
    let quit = MenuItemBuilder::with_id(QUIT_ID, "Quit").build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&settings)
        .item(&check_updates)
        .separator()
        .item(&new_jobs)
        .item(&pause_all_item)
        .separator()
        .item(&quit)
        .build()?;

    // macOS menu bar wants a monochrome template (the OS tints it for light/dark);
    // Windows/Linux want the full-color glyph. Both are full-bleed (no app-icon
    // padding) so the mark is legible in the small tray slot.
    let builder = TrayIconBuilder::new();
    #[cfg(target_os = "macos")]
    let builder = builder
        .icon(tauri::include_image!("icons/tray-template.png"))
        .icon_as_template(true);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.icon(tauri::include_image!("icons/tray.png"));
    builder
        .menu(&menu)
        .tooltip("AI Job Hunter")
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_ID => show_focus(app),
            SETTINGS_ID => dispatch_menu(
                app,
                NAVIGATE_EVENT,
                serde_json::json!({ "route": "/settings", "section": serde_json::Value::Null }),
            ),
            CHECK_UPDATES_ID => dispatch_menu(
                app,
                ACTION_EVENT,
                serde_json::json!({ "action": "check-updates" }),
            ),
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
    // Buffer for a menu intent missed by a backgrounded webview (see `dispatch_menu`).
    app.manage(PendingMenu(Mutex::new(None)));
    Ok(())
}

/// Restore and focus the main window. The single entry point for every reopen
/// path (tray Show, tray icon click, single-instance relaunch, deep link, the
/// notification command) so they all share the macOS Dock-icon restore: when the
/// window was hidden to the tray its activation policy is `Accessory` (no Dock
/// icon); switch back to `Regular` before showing so the Dock icon returns.
pub fn show_focus(app: &AppHandle) {
    // macOS only: restore the Dock icon dropped by close-to-tray. No-op (and the
    // policy stays `Regular`) on a normal show, so this is always safe to call.
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Restore the window and deliver a menu intent (`menu:navigate` / `menu:action`)
/// from the tray or the macOS menu bar.
///
/// `emit` is fire-and-forget — there is no per-listener queue — so a push fired
/// right after a menu click is easily lost: the webview is suspended
/// (close-to-tray), it hasn't re-attached its listeners yet, or WebView2 defers
/// IPC while the tray menu holds the foreground. So we ALWAYS buffer the intent
/// in `PendingMenu` and let the RENDERER pull it (`menu_take_pending`) once its JS
/// loop is provably live — it drains on the emitted event, on window
/// focus/visibility-restore, and on mount. The buffer is written BEFORE
/// `show_focus` so it's in place no matter how quickly the renderer pulls, and
/// `menu_take_pending` takes-and-clears atomically, so the intent is delivered
/// exactly once and can't re-fire on a later unrelated focus. The immediate emit
/// stays purely as a low-latency *trigger* (its payload is ignored by the
/// renderer) for the already-visible case where no focus/visibility change fires
/// (notably the macOS menu bar, where the window never loses key focus).
pub fn dispatch_menu(app: &AppHandle, event: &str, payload: serde_json::Value) {
    log::info!("[menu] dispatch {event} {payload}");
    // Buffer BEFORE showing so the renderer's pull (which may fire as soon as the
    // window is shown / focused) always finds the intent.
    if let Some(s) = app.try_state::<PendingMenu>() {
        *s.0.lock() = Some((event.to_string(), payload.clone()));
    }
    show_focus(app);
    // Low-latency trigger so a live webview drains immediately (the renderer pulls
    // the buffer rather than trusting this payload).
    let _ = app.emit(event, payload.clone());
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.emit(event, payload.clone());
    }
    // Deferred re-trigger: on macOS the webview defers IPC while the menu owns the
    // foreground, so the immediate emit above is dropped when the window was already
    // visible (menu-bar case — no focus/visibility change to fall back on). Re-emit
    // shortly after the menu closes, when IPC has resumed. The renderer drains the
    // PendingMenu buffer exactly-once, so this duplicate is a no-op if the first emit
    // already landed. Platform-agnostic and harmless on Windows/tray.
    let app_retry = app.clone();
    let event_retry = event.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
        let _ = app_retry.emit(&event_retry, payload);
    });
}

/// After a run surfaces `new_count` brand-new jobs (>0): push a Notification
/// Center record (which also raises the OS banner + an in-app toast via
/// [`push_and_notify`]) and bump the tray "New jobs: N" counter, remembering
/// which autopilot to focus when the user clicks it. The banner policy is
/// [`OsBanner::Always`]: a background run finishing warrants the OS nudge even
/// while the window is focused.
pub fn on_new_jobs(app: &AppHandle, autopilot_id: &str, autopilot_name: &str, new_count: u32) {
    if new_count == 0 {
        return;
    }
    let mut search = serde_json::Map::new();
    search.insert(
        "focus".to_string(),
        serde_json::Value::String(autopilot_id.to_string()),
    );
    push_and_notify(
        app,
        NewNotification {
            kind: "autopilot.new_jobs".to_string(),
            title: autopilot_name.to_string(),
            body: new_jobs_body(autopilot_name, new_count),
            route: Some(NotificationRoute {
                to: "/autopilot".to_string(),
                search: Some(search),
            }),
        },
        OsBanner::Always,
    );
    if let Some(state) = app.try_state::<TrayState>() {
        let total = {
            let mut g = state.inner.lock();
            g.total = g.total.saturating_add(new_count);
            g.last_autopilot = Some(autopilot_id.to_string());
            g.total
        };
        let _ = state.new_jobs_item.set_text(format!("New jobs: {total}"));
        let _ = state.new_jobs_item.set_enabled(total > 0);
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

/// The human "N new job(s) for …" notification body. Extracted from the old
/// `notify` so the wording is unchanged after the move to `push_and_notify`.
fn new_jobs_body(name: &str, count: u32) -> String {
    if count == 1 {
        format!("1 new job for “{name}”")
    } else {
        format!("{count} new jobs for “{name}”")
    }
}

/// Handle a "new jobs" click from the tray counter ("New jobs: N" menu item):
/// focus the window, jump to the autopilot whose run produced the newest finds,
/// and reset the unseen counter.
pub fn handle_notification_click(app: &AppHandle) {
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
    let _ = state.new_jobs_item.set_enabled(false);
    if let Some(id) = target {
        emit_focus(app, &id);
    }
}

fn on_new_jobs_click(app: &AppHandle) {
    handle_notification_click(app);
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
