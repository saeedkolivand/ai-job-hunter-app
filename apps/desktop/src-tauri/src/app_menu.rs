//! The native app menu (macOS menu bar / Windows app menu): build + the click handler. Split out
//! of `lib.rs` (R8 relief, PR4 — the Prep tab deep link's new `handle_deep_link` match arm pushed
//! that module to the hard LOC cap with only 2 lines of headroom) — behaviourally identical, only
//! the file it lives in moved. `pub(crate)` so `lib.rs`'s `run()` keeps calling
//! `app_menu::build_app_menu`/`app_menu::on_app_menu_event` at the two sites that used to call
//! these unqualified.

use tauri::menu::{
    AboutMetadataBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::{AppHandle, Manager};

// Custom (non-predefined) menu-item ids. Predefined roles still self-handle; only
// these ids are dispatched in `on_app_menu_event` (registered in `lib.rs`'s `setup`). The
// `menu_nav_*` ids each map to a renderer route via [`NAV_ITEMS`].
const MENU_SETTINGS: &str = "menu_settings";
const MENU_CHECK_UPDATES: &str = "menu_check_updates";
const MENU_DOCS: &str = "menu_docs";
const MENU_SHORTCUTS: &str = "menu_shortcuts";
const MENU_REPORT: &str = "menu_report";
const MENU_RELOAD: &str = "menu_reload";
const MENU_DEVTOOLS: &str = "menu_devtools";

/// Public-facing URLs for the Help submenu. Derived from the repository (the only
/// canonical URL the project ships — the updater endpoint in tauri.conf.json points
/// at this repo's releases). No `homepage` is set in any package.json, so docs maps
/// to the repo root and "Report an Issue" to the issues tracker.
const REPO_URL: &str = "https://github.com/saeedkolivand/ai-job-hunter-app";
const ISSUES_URL: &str = "https://github.com/saeedkolivand/ai-job-hunter-app/issues";

/// View-submenu go-to-route items: `(id, accelerator, route, label)`. The route
/// strings are the canonical values from
/// `apps/desktop/src/renderer/constants/routes/routes.ts` (Documents → `/documents`,
/// Resume Analyzer → `/analyze`, AI Generate → `/ai-generate`). The label is the
/// menu-item title. `on_app_menu_event` emits `menu:navigate { route, section: null }`
/// for each. One row per item keeps id/accel/route/label in lockstep.
const NAV_ITEMS: &[(&str, &str, &str, &str)] = &[
    ("menu_nav_dashboard", "CmdOrCtrl+1", "/", "Dashboard"),
    ("menu_nav_jobs", "CmdOrCtrl+2", "/jobs", "Jobs"),
    (
        "menu_nav_analyze",
        "CmdOrCtrl+3",
        "/analyze",
        "Resume Analyzer",
    ),
    (
        "menu_nav_ai_generate",
        "CmdOrCtrl+4",
        "/ai-generate",
        "AI Generate",
    ),
    (
        "menu_nav_documents",
        "CmdOrCtrl+5",
        "/documents",
        "Documents",
    ),
    (
        "menu_nav_autopilot",
        "CmdOrCtrl+6",
        "/autopilot",
        "Autopilot",
    ),
    ("menu_nav_settings", "CmdOrCtrl+7", "/settings", "Settings"),
];

/// Resolve a `menu_nav_*` id to its route. Returns `None` for any other id.
fn nav_route_for(id: &str) -> Option<&'static str> {
    NAV_ITEMS
        .iter()
        .find_map(|(item_id, _, route, _)| (*item_id == id).then_some(*route))
}

pub(crate) fn build_app_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    // Predefined roles self-handle and carry the standard platform accelerator.
    // The custom items below (Settings, Check for Updates, the Help entries, the
    // View nav/reload/devtools entries) carry explicit ids dispatched by
    // `on_app_menu_event`.
    let about_metadata = AboutMetadataBuilder::new()
        .name(Some("AI Job Hunter"))
        .version(Some(env!("CARGO_PKG_VERSION")))
        .comments(Some("Your local-first AI copilot for the job hunt."))
        .copyright(Some("© 2026 AI Job Hunter"))
        .website(Some(REPO_URL))
        .website_label(Some("GitHub"))
        .build();

    // App submenu — must remain the FIRST submenu (the macOS app-name menu).
    let app_submenu = SubmenuBuilder::new(app, "AI Job Hunter")
        .item(&PredefinedMenuItem::about(app, None, Some(about_metadata))?)
        .separator()
        .item(
            &MenuItemBuilder::with_id(MENU_SETTINGS, "Settings…")
                .accelerator("CmdOrCtrl+,")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id(MENU_CHECK_UPDATES, "Check for Updates…").build(app)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, None)?)
        .item(&PredefinedMenuItem::hide_others(app, None)?)
        .item(&PredefinedMenuItem::show_all(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()?;

    let edit_submenu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .build()?;

    // View submenu — Fullscreen, then go-to-route items (Cmd/Ctrl+1..7),
    // Reload, and Toggle DevTools.
    let mut view_builder =
        SubmenuBuilder::new(app, "View").item(&PredefinedMenuItem::fullscreen(app, None)?);
    view_builder = view_builder.separator();
    for (id, accel, _route, label) in NAV_ITEMS.iter() {
        view_builder = view_builder.item(
            &MenuItemBuilder::with_id(*id, label)
                .accelerator(accel)
                .build(app)?,
        );
    }
    let view_submenu = view_builder
        .separator()
        .item(
            &MenuItemBuilder::with_id(MENU_RELOAD, "Reload")
                .accelerator("CmdOrCtrl+R")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id(MENU_DEVTOOLS, "Toggle DevTools").build(app)?)
        .build()?;

    let window_submenu = SubmenuBuilder::new(app, "Window")
        .item(&PredefinedMenuItem::minimize(app, None)?)
        .item(&PredefinedMenuItem::maximize(app, None)?)
        // No `zoom` PredefinedMenuItem exists in Tauri 2.11; `bring_all_to_front`
        // does, so we add only that (the spec says add it only if it exists).
        .item(&PredefinedMenuItem::bring_all_to_front(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, None)?)
        .build()?;

    // Help submenu (last) — opens external URLs / emits the shortcuts action.
    let help_submenu = SubmenuBuilder::new(app, "Help")
        .item(&MenuItemBuilder::with_id(MENU_DOCS, "Documentation").build(app)?)
        .item(&MenuItemBuilder::with_id(MENU_SHORTCUTS, "Keyboard Shortcuts").build(app)?)
        .item(&MenuItemBuilder::with_id(MENU_REPORT, "Report an Issue").build(app)?)
        .build()?;

    MenuBuilder::new(app)
        .item(&app_submenu)
        .item(&edit_submenu)
        .item(&view_submenu)
        .item(&window_submenu)
        .item(&help_submenu)
        .build()
}

/// App-level menu-event handler (custom ids only — predefined roles self-handle).
/// Registered in `setup` after `set_menu`. Emits the renderer contract events
/// (`menu:navigate` / `menu:action`) or performs the shell-side action directly.
pub(crate) fn on_app_menu_event(app: &AppHandle, id: &str) {
    match id {
        MENU_SETTINGS => crate::tray::dispatch_menu(
            app,
            crate::events::MENU_NAVIGATE,
            serde_json::json!({ "route": "/settings", "section": serde_json::Value::Null }),
        ),
        MENU_CHECK_UPDATES => crate::tray::dispatch_menu(
            app,
            crate::events::MENU_ACTION,
            serde_json::json!({ "action": "check-updates" }),
        ),
        MENU_SHORTCUTS => crate::tray::dispatch_menu(
            app,
            crate::events::MENU_ACTION,
            serde_json::json!({ "action": "shortcuts" }),
        ),
        MENU_DOCS => open_external(app, REPO_URL),
        MENU_REPORT => open_external(app, ISSUES_URL),
        MENU_RELOAD => {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.reload();
            }
        }
        MENU_DEVTOOLS => {
            if let Some(win) = app.get_webview_window("main") {
                // `open_devtools` is a no-op in release builds without the
                // `devtools` feature, but always compiles (matches the existing
                // `system_open_devtools` command).
                win.open_devtools();
            }
        }
        // Go-to-route items: emit `menu:navigate` with the resolved route.
        other => {
            if let Some(route) = nav_route_for(other) {
                crate::tray::dispatch_menu(
                    app,
                    crate::events::MENU_NAVIGATE,
                    serde_json::json!({ "route": route, "section": serde_json::Value::Null }),
                );
            }
            // Unknown id → no-op.
        }
    }
}

/// Open an external URL in the user's default handler via `tauri_plugin_opener`
/// (same path as the `system_open_external` command). Best-effort.
fn open_external(app: &AppHandle, url: &str) {
    use tauri_plugin_opener::OpenerExt;
    let _ = app.opener().open_url(url, None::<&str>);
}
