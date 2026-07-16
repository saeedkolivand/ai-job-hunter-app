//! AI Job Hunter — Tauri desktop shell (library crate).
//!
//! Every application module lives here so the crate is reachable as a library
//! (`ajh_tauri::…`) from integration tests and `benches/`. `main.rs` is a thin
//! binary shim that calls [`run`]. This is the canonical Tauri 2 layout (lib
//! owns the app, bin is a launcher) and is what lets `benches/export_render.rs`
//! call `export::pdf::generate_pdf` directly.

// Async-safety: never hold a lock guard across an `.await` (the app uses
// `parking_lot::Mutex` inside async command handlers). See docs/architecture-rules.md R14.
#![deny(clippy::await_holding_lock)]

pub mod agent;
pub mod ai_config;
pub mod ai_generations;
pub mod applications;
pub mod autopilot;
pub mod autopilot_helpers;
pub mod autopilot_scheduler;
pub mod commands;
pub mod contact_profile;
pub mod cover_letter;
pub mod credentials;
pub mod data_store;
pub mod db;
pub mod deeplink;
pub mod documents;
pub mod error;
pub mod events;
pub mod export;
pub mod extension_bridge;
pub mod extraction;
pub mod ipc_contracts;
pub mod job_preferences;
pub mod jobs;
pub mod limits;
pub mod locale;
pub mod model;
pub mod net;
pub mod notifications;
pub mod observability;
pub mod performance;
pub mod pipeline;
pub mod platform;
pub mod postings;
pub mod profile_import;
pub mod recommend;
pub mod referrals;
pub mod salary_research;
pub mod scraping;
pub mod spend;
pub mod theme;
pub mod tray;
pub mod updater;
pub mod validate;

use parking_lot::Mutex;

use tauri::menu::{
    AboutMetadataBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::{AppHandle, Manager};

use autopilot::AutopilotStore;
use credentials::CredentialStore;
use jobs::JobTracker;
use postings::{InteractionStore, PostingsCache};
use scraping::ScraperEngine;
use updater::UpdaterState;

/// Live in-memory "close hides to tray" flag. Managed as a distinct newtype (not
/// a bare `Mutex<bool>`, which would collide with other managed `Mutex<bool>`
/// state) so the window-close handler can resolve it unambiguously via
/// `app.state::<CloseToTray>()`. Defaults to `true` so behaviour is unchanged
/// until the renderer pushes the persisted preference (via
/// `system_set_close_to_tray`) on boot. The renderer's preferences store is the
/// source of truth; this is just the value the shell reads on close.
pub struct CloseToTray(pub Mutex<bool>);

impl Default for CloseToTray {
    fn default() -> Self {
        Self(Mutex::new(true))
    }
}

// ── App menu ──────────────────────────────────────────────────────────────────

// Custom (non-predefined) menu-item ids. Predefined roles still self-handle; only
// these ids are dispatched in `on_app_menu_event` (registered in `setup`). The
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

/// Drive the renderer for a validated deep-link target. Shared by every delivery
/// path (single-instance relaunch, `on_open_url`, cold first-instance launch) so
/// they stay in lockstep. `None` (a hostile/unrecognized URL) navigates nowhere
/// — the caller has already focused the window. Both arms use a cold-start-robust
/// buffered intent so the signal survives a renderer that hasn't attached its
/// listeners yet (the cold first-instance launch fires during Rust setup):
/// autopilot buffers in `PendingFocus` (`dispatch_focus`); pairing buffers the
/// `menu:navigate` intent (`dispatch_extension_pairing`).
fn handle_deep_link(app: &AppHandle, target: Option<deeplink::FocusTarget>) {
    match target {
        Some(deeplink::FocusTarget::Autopilot(id)) => tray::dispatch_focus(app, &id),
        Some(deeplink::FocusTarget::ExtensionPairing) => tray::dispatch_extension_pairing(app),
        None => {}
    }
}

fn build_app_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
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
fn on_app_menu_event(app: &AppHandle, id: &str) {
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

// ── Entry point ───────────────────────────────────────────────────────────────

/// Detect a browser native-messaging launch from argv and, if so, run the stdio
/// relay host ([`extension_bridge::native_host`]) instead of booting Tauri,
/// returning `true` once it has handled and run the relay. `main.rs` calls this
/// first so a native-host launch never reaches the Tauri builder or the
/// single-instance plugin (which would forward argv to the running app and kill
/// the stdio Port).
///
/// The BROWSER controls argv, so we detect by what browsers actually pass:
/// - Firefox: `[exe, <manifest_path>, <extension_id>]` — an arg ends with the
///   host-manifest filename.
/// - Chrome: `[exe, chrome-extension://<id>/, (--parent-window=<hwnd> on Win)]` —
///   an arg starts with `chrome-extension://`.
///
/// Our deep links use the `ajh://` scheme, so there is no collision with these.
pub fn run_native_host_if_invoked() -> bool {
    let is_native_host = is_native_host_launch(std::env::args().skip(1));
    if is_native_host {
        extension_bridge::native_host::run();
    }
    is_native_host
}

/// True if these argv-tail args look like a browser native-messaging launch:
/// Chrome passes the extension origin (`chrome-extension://…`); Firefox passes the
/// full path to our host manifest, whose filename starts with `NATIVE_HOST_NAME` on
/// every OS (mac/linux: `…bridge.json`; Windows: `…bridge.firefox.json` / `.chrome.json`).
/// The host name is matched on the manifest FILENAME (basename) only — not anywhere
/// in the path — so a directory that merely contains the host name can't false-positive.
/// Extracted from `run_native_host_if_invoked` so the per-OS filename matching is
/// unit-testable (argv is process-global and can't be set in a test).
fn is_native_host_launch<I: IntoIterator<Item = String>>(args: I) -> bool {
    args.into_iter().any(|arg| {
        if arg.starts_with("chrome-extension://") {
            return true;
        }
        // Match the host manifest by FILENAME (basename), not anywhere in the
        // path, so a directory that merely contains the host name can't trigger
        // a false native-host launch. Every browser/OS manifest filename starts
        // with NATIVE_HOST_NAME and ends with `.json` (…bridge.json /
        // …bridge.firefox.json / …bridge.chrome.json). Split on both separators
        // so a Windows backslash path is handled too.
        let basename = arg.rsplit(['/', '\\']).next().unwrap_or(arg.as_str());
        basename.starts_with(extension_bridge::NATIVE_HOST_NAME) && basename.ends_with(".json")
    })
}

/// Build and run the Tauri application. Called by the binary shim in `main.rs`.
pub fn run() {
    // Install the crash-reporter panic hook before everything else so panics
    // that occur during setup are also caught.  We chain the previous hook
    // (the default) so stderr still prints as usual.
    // ponytail: file-log panic hook; upgrade to sentry only if remote crash aggregation is ever needed.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Chain default hook first — keeps the familiar stderr output intact.
        prev_hook(info);
        // Best-effort append to crashes.log; ignore every IO error so we never
        // panic inside the panic hook.
        let _ = (|| -> std::io::Result<()> {
            use std::io::Write as _;
            let log_path = crate::platform::config::data_dir().join("crashes.log");
            if let Some(dir) = log_path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)?;
            let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
                .unwrap_or("<non-string panic payload>");
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| String::from("<unknown>"));
            let bt = std::backtrace::Backtrace::force_capture();
            writeln!(
                file,
                "[{timestamp}] PANIC at {location}: {msg}\nBacktrace:\n{bt}\n---"
            )?;
            Ok(())
        })();
    }));

    // Initialise the OS keyring up front. A failure here is non-fatal: the app
    // still boots, and credential operations (AI provider keys, factory reset)
    // surface the error later through `AppError` rather than aborting startup.
    if let Err(e) = credentials::init_keyring() {
        log::warn!("[startup] OS keyring unavailable (credential features degraded): {e}");
    }

    tauri::Builder::default()
        // Close-to-tray: intercept the window close (X / Cmd-W) and hide to the
        // tray instead — but ONLY when (a) a tray actually exists and (b) the
        // user's `CloseToTray` preference is on. A non-fatal `tray::build` failure
        // leaves no `TrayState`; in that case we fall through to the default close
        // so the window can never be soft-trapped hidden with no tray to restore
        // it. When the preference is off the window closes / app quits normally
        // (we never call `prevent_close`). SAFETY: `prevent_close()` intercepts
        // only the window close; `PredefinedMenuItem::quit`, Cmd-Q, and the tray
        // Quit (`app.exit(0)`) bypass window events and still fully quit the app.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let hide_to_tray = app.try_state::<crate::tray::TrayState>().is_some()
                    && *app.state::<CloseToTray>().0.lock();
                if hide_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                    // Drop the Dock icon while hidden (restored by `tray::show_focus`).
                    #[cfg(target_os = "macos")]
                    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
            // A menu intent buffered while the window was hidden (close-to-tray) is
            // NOT re-emitted from here: a Rust `emit` races the resumed webview's JS
            // readiness the same way the original emit did. Instead the renderer
            // pulls it via `menu_take_pending` on focus/visibility-restore.
        })
        // Single-instance must be the FIRST plugin: on a second launch it focuses
        // the already-running window instead of spawning another process. If that
        // launch carried an `ajh://autopilot/<id>` or `ajh://settings/extension`
        // deep link, the guard validates it against a strict route allowlist
        // before driving any navigation — a hostile argv navigates nowhere (see
        // `deeplink`).
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // Route through `show_focus` so a second launch also restores the Dock
            // icon (macOS) if the window was hidden to the tray.
            tray::show_focus(app);
            handle_deep_link(app, deeplink::parse_focus_target(&argv));
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .max_file_size(5_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .level(log::LevelFilter::Warn)
                // Surface the company-research brief (logged at info) in the
                // terminal/logs without lowering the global level (which would
                // flood with per-request RequestTrace info lines).
                .level_for("ajh_tauri::cover_letter::research", log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        // Persist + restore window size/position/maximized across launches; the
        // width/height/center in tauri.conf.json become first-run defaults only.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        // Opt-in launch-at-login (default OFF; toggled via `system_*` commands).
        // Registered after single-instance so a login launch focuses the
        // existing window rather than spawning a duplicate. No launch args.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // Added for FUTURE use — no renderer callers yet (their `*:default`
        // capabilities are listed in capabilities/default.json so they are ready
        // to wire). OS info, process control, window positioning, a JSON
        // key-value store, and a client WebSocket. `global-shortcut` is
        // desktop-only and gated below, mirroring the `#[cfg(desktop)]` deep-link
        // pattern used in `setup`.
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_websocket::init())
        .setup(|app| {
            // Global shortcuts are a desktop-only capability; register the plugin
            // here (gated) rather than in the always-compiled builder chain so a
            // mobile/other-target build is not broken. No shortcuts are bound yet
            // — this only makes the `global-shortcut:default` capability available.
            #[cfg(desktop)]
            if let Err(e) = app
                .handle()
                .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            {
                log::warn!("[setup] global-shortcut plugin init failed (non-fatal): {e}");
            }

            let handle = app.handle();

            // Buffer for an autopilot-focus intent (cold-start `ajh://autopilot/<id>`)
            // managed HERE, before the cold-start deep-link block below — that block
            // runs during setup, well before `tray::build` (which manages the sibling
            // `PendingMenu`). `dispatch_focus` writes the id into this buffer BEFORE
            // emitting, so it must already be in state when the cold-start deep link
            // is handled, or the write silently no-ops and the intent is lost.
            app.manage(tray::PendingFocus(Mutex::new(None)));

            // `ajh://` deep links. The OS routes a cold/click-launched URL here
            // (macOS via `on_open_url`; Windows/Linux a second instance forwards
            // it as argv → the single-instance guard above). Every URL is funneled
            // through the same strict allowlist (`deeplink::parse_focus_target`)
            // before any navigation — a hostile URL focuses the window and stops.
            #[cfg(desktop)]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                // Register the scheme at runtime (no-op once the installer has;
                // required for Linux + `pnpm dev`). Best-effort.
                let _ = app.deep_link().register_all();
                let dl_handle = handle.clone();
                app.deep_link().on_open_url(move |event| {
                    let urls: Vec<String> = event.urls().iter().map(|u| u.to_string()).collect();
                    // `show_focus` so a deep-link reopen also restores the Dock
                    // icon (macOS) when hidden to the tray. (The pairing path's
                    // `dispatch_menu` calls `show_focus` itself, so calling it
                    // first here is a harmless no-op for that target.)
                    tray::show_focus(&dl_handle);
                    handle_deep_link(&dl_handle, deeplink::parse_focus_target(&urls));
                });

                // Cold start: when the app was NOT already running, the OS launches
                // it FRESH with the `ajh://…` URL and the single-instance callback
                // never fires (it only triggers on a *second* launch). On Windows/
                // Linux that first-instance URL arrives on argv; on macOS the plugin
                // surfaces it via `get_current()`. Parse both so a not-running launch
                // (the primary case for `ajh://settings/extension`) still routes.
                let initial = deeplink::parse_focus_target(
                    &std::env::args_os()
                        .filter_map(|a| a.into_string().ok())
                        .collect::<Vec<String>>(),
                )
                .or_else(|| {
                    app.deep_link()
                        .get_current()
                        .ok()
                        .flatten()
                        .map(|urls| urls.iter().map(|u| u.to_string()).collect::<Vec<_>>())
                        .and_then(|urls| deeplink::parse_focus_target(&urls))
                });
                if initial.is_some() {
                    // Cold start with a deep link: the main window is visible from
                    // boot (config `visible: true`), so just show+focus it before
                    // routing. On this fresh-launch path the macOS Dock-icon/
                    // activation-policy restore `show_focus` adds is a no-op (the
                    // app launched `Regular`, never hid to the tray), so show+focus
                    // is the full intent; deep-link routing below is unchanged.
                    tray::show_focus(handle);
                    handle_deep_link(handle, initial);
                }
            }

            // Data dir for all persistent state. Resolved + exported once here so
            // AppHandle-less workers (scrapers/appliers) reach the same path.
            // All path/env knowledge lives in `platform::config`.
            let data_dir = platform::config::resolve_and_export_data_dir(handle);

            // Persistent user-data stores are managed via `manage_resettable`, which
            // also registers each one into the `ResetRegistry` so `privacy_reset_app`
            // wipes it on factory reset — no hand-maintained clear list.
            use commands::privacy::manage_resettable;
            let mut reset_registry = commands::privacy::ResetRegistry::default();

            manage_resettable(
                app,
                &mut reset_registry,
                "autopilots",
                std::sync::Arc::new(Mutex::new(AutopilotStore::new(&data_dir))),
            );
            manage_resettable(
                app,
                &mut reset_registry,
                "credentials",
                Mutex::new(CredentialStore::new(&data_dir)),
            );
            match documents::DocumentStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "documents", store),
                Err(e) => log::warn!("[setup] document store failed to open (non-fatal): {e}"),
            }
            match ai_generations::AiGenerationStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "ai_generations", store),
                Err(e) => {
                    log::warn!("[setup] ai generations store failed to open (non-fatal): {e}")
                }
            }
            // Applications: the status-bearing aggregate root (ADR 0001). Opening it
            // runs the one-time, idempotent backfill from ai_generations.db, so it
            // is managed AFTER the generation store above (order is cosmetic — the
            // backfill reads ai_generations.db by path, not via Tauri state).
            match applications::ApplicationStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "applications", store),
                Err(e) => {
                    log::warn!("[setup] applications store failed to open (non-fatal): {e}")
                }
            }
            match job_preferences::JobPreferencesStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "job_preferences", store),
                Err(e) => {
                    log::warn!("[setup] job preferences store failed to open (non-fatal): {e}")
                }
            }
            match contact_profile::ContactProfileStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "contact_profile", store),
                Err(e) => {
                    log::warn!("[setup] contact profile store failed to open (non-fatal): {e}")
                }
            }
            // Backend-owned active AI provider store (task #16): the single source of
            // truth for generation routing (provider/model/base_url). Holds no
            // secrets; wiped on factory reset, included in backups.
            match ai_config::AiConfigStore::open(&data_dir) {
                Ok(store) => {
                    manage_resettable(app, &mut reset_registry, "ai_provider_config", store)
                }
                Err(e) => {
                    log::warn!("[setup] ai provider config store failed to open (non-fatal): {e}")
                }
            }
            match referrals::ReferralStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "referrals", store),
                Err(e) => {
                    log::warn!("[setup] referrals store failed to open (non-fatal): {e}")
                }
            }
            manage_resettable(
                app,
                &mut reset_registry,
                "job_tracker",
                Mutex::new(JobTracker::open(&data_dir)),
            );
            manage_resettable(
                app,
                &mut reset_registry,
                "postings",
                Mutex::new(PostingsCache::default()),
            );
            manage_resettable(
                app,
                &mut reset_registry,
                "interactions",
                Mutex::new(InteractionStore::new(&data_dir)),
            );
            // AI-spend visibility: real per-call token usage + estimated cost,
            // recorded by the streaming and pipeline chokepoints in
            // `commands::ai_provider::stream` / `pipeline::Completer::complete`.
            match spend::SpendStore::open(&data_dir) {
                Ok(store) => manage_resettable(app, &mut reset_registry, "spend", store),
                Err(e) => log::warn!("[setup] spend store failed to open (non-fatal): {e}"),
            }
            app.manage(Mutex::new(UpdaterState::default()));
            app.manage(std::sync::Arc::new(ScraperEngine::new()));
            // In-memory anti-abuse limiter (rate + concurrency + per-provider daily
            // ceiling) for the expensive commands `ai_generate`, `scrape_board`, and
            // `scrape_url`. Process-local; resets on restart. Not in the reset
            // registry — it holds no user data, only transient counters.
            app.manage(std::sync::Arc::new(limits::Limiter::new()));
            // Agent confirm gate: pending human-in-the-loop Write confirmations,
            // keyed by (jobId, callId). `agent_run` registers an entry when it
            // suspends on a Write tool; `agent_confirm` resolves it. Process-local,
            // holds no user data — not in the reset registry.
            app.manage(agent::gate::AgentGate::default());
            // Live performance config (balanced default). Updated by system_set_performance_mode.
            crate::performance::set(crate::performance::PerformanceConfig::default());
            app.manage(commands::translation::TranslationCache::new());
            // Live close-to-tray flag (default on). The renderer pushes the
            // persisted preference via `system_set_close_to_tray` on boot; the
            // window-close handler above reads it.
            app.manage(CloseToTray::default());
            // The conversations (chat) feature was removed; best-effort delete the
            // now-orphaned conversations.db (+ WAL/SHM sidecars) it left in the app-data
            // dir so dead chat history isn't kept on disk. Idempotent (no-op once gone).
            if let Ok(app_data) = handle.path().app_data_dir() {
                for f in [
                    "conversations.db",
                    "conversations.db-wal",
                    "conversations.db-shm",
                ] {
                    let _ = std::fs::remove_file(app_data.join(f));
                }
            }
            match pipeline::cache::KvCache::open(&data_dir) {
                Ok(cache) => manage_resettable(app, &mut reset_registry, "cache", cache),
                Err(e) => log::warn!("[setup] pipeline cache failed to open (non-fatal): {e}"),
            }

            // Guard: the registry must contain exactly the labels the
            // completeness test pins (`MANAGE_RESETTABLE_LABELS`) before the
            // bridge/notification stores register their own labels. A forgotten
            // `manage_resettable` above trips this in debug builds.
            debug_assert_eq!(
                reset_registry.labels(),
                commands::privacy::MANAGE_RESETTABLE_LABELS.to_vec(),
                "manage_resettable registrations drifted from MANAGE_RESETTABLE_LABELS"
            );

            // Browser-extension bridge (Feature 2): manage the pairing-token state
            // (+ register its factory-reset token rotation). The loopback WS server
            // itself is started below, after the registry is in state.
            extension_bridge::manage(app, &mut reset_registry, &data_dir);

            // Notification Center (Phase 1): manage the persisted notification
            // store (+ register its factory-reset wipe). Pure data + disk; the
            // push orchestration (OS banner / tray / renderer event) is Phase 4.
            notifications::manage(app, &mut reset_registry, &data_dir);

            app.manage(reset_registry);

            // Build and set the application menu. Predefined roles self-handle;
            // the custom items (Settings, Check for Updates, nav, reload, devtools,
            // Help) are dispatched by the app-level handler registered below.
            let menu = build_app_menu(handle)?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| on_app_menu_event(app, event.id().as_ref()));

            // Platform-specific window decorations
            #[cfg(target_os = "windows")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_decorations(false)?;
                }
            }
            #[cfg(target_os = "macos")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_decorations(true)?;
                }
            }

            // Build system tray.
            if let Err(e) = tray::build(handle) {
                log::warn!("[setup] tray build error (non-fatal): {e}");
            }

            // Schedule background update checks (10 s after launch, then every 4 h).
            updater::setup_auto_check(handle);

            // Start autopilot schedule runner (checks every minute).
            autopilot_scheduler::start(handle.clone());

            // Start the browser-extension WS bridge (loopback only). Fire-and-
            // forget on the tokio runtime; a bind failure logs + disables the
            // bridge and never blocks boot. `BridgeState` was managed above.
            extension_bridge::start(handle.clone());

            // Keep the native-messaging host manifests + OS registration current
            // (path tracks app moves/updates) so the browser can spawn our stdio
            // relay — the Firefox HTTPS-Only transport that survives the ws→wss
            // upgrade. Best-effort; never blocks boot.
            extension_bridge::register::register_native_host(&data_dir);

            // Watch the OS accent color (Windows): on a personalization accent
            // change, emit `system:accentChanged` so the renderer re-pulls the
            // color and re-applies the theme live. The watcher parks its WinRT
            // subscription in managed state to stay alive. No-op off Windows;
            // there the renderer's window-focus refetch covers it. Best-effort.
            platform::accent_watcher::start(handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // cli agents (install status — #22)
            commands::cli_agents::cli_agents_status,
            commands::cli_agents::cli_agents_redetect,
            // system
            commands::system::system_health,
            commands::system::system_get_version,
            commands::system::system_get_locale,
            commands::system::system_set_locale,
            commands::system::system_get_platform,
            commands::system::system_accent_color,
            commands::system::system_open_external,
            commands::system::system_set_performance_mode,
            commands::system::system_get_launch_at_login,
            commands::system::system_set_launch_at_login,
            commands::system::system_set_close_to_tray,
            commands::system::system_get_metrics,
            commands::system::system_check_browser,
            commands::system::system_open_devtools,
            commands::system::system_get_protocol_version,
            // native menu (pull buffered intent after close-to-tray restore)
            commands::menu::menu_take_pending,
            // jobs
            commands::jobs::jobs_list,
            commands::jobs::jobs_get,
            commands::jobs::jobs_cancel,
            commands::jobs::jobs_retry,
            // ai
            commands::ai::ai_generate,
            commands::ai::ai_list_models,
            commands::ai::ai_model_capabilities,
            commands::ai::ai_inspect_model,
            commands::ai::ai_research_company,
            commands::ai::ai_research_answer,
            commands::ai::ai_lookup_salary,
            commands::ai::ai_pull_model,
            commands::ai::ai_unload_model,
            commands::ai::ai_embed,
            commands::ai::ai_set_provider_key,
            commands::ai::ai_remove_provider_key,
            commands::ai::ai_has_provider_key,
            commands::ai::ai_test_provider_key,
            commands::ai::ai_list_provider_models,
            commands::ai::ai_embedding_status,
            commands::ai::ai_set_embedding_config,
            commands::ai::ai_reembed_all,
            commands::ai::ai_spend_summary,
            commands::ai::ai_active_config,
            commands::ai::ai_set_active_provider,
            commands::ai::ai_set_provider_settings,
            commands::ai::ai_seed_active_config,
            commands::pipeline::generate_pipeline,
            // agent ("prep this application" flow + human-in-the-loop confirm gate)
            commands::agent::agent_run,
            commands::agent::agent_confirm,
            // resume extraction
            commands::resume::extract_resume,
            // documents
            commands::documents::documents_list,
            commands::documents::documents_import,
            commands::documents::documents_recommend_template,
            commands::documents::documents_remove,
            commands::documents::documents_set_default,
            commands::documents::documents_get_text,
            // job preferences
            commands::job_preferences::job_preferences_get,
            commands::job_preferences::job_preferences_set,
            // contact profile (header source of truth)
            commands::contact_profile::contact_profile_get,
            commands::contact_profile::contact_profile_set,
            // scrape
            commands::scrape::scrape_boards,
            commands::scrape::scrape_url,
            commands::scrape::scrape_resolve_url,
            commands::scrape::scrape_update_description,
            commands::scrape::scrape_persist_job,
            commands::scrape::scrape_list_postings,
            commands::scrape::scrape_clear_postings,
            commands::scrape::scrape_list_interactions,
            // data backup / restore
            commands::data::data_export,
            commands::data::data_import,
            // match
            commands::match_resume::match_resume,
            commands::match_resume::resume_extract_text,
            // credentials (board-login CRUD removed — sessions auth via boards.*)
            commands::credentials::credentials_available,
            // boards
            commands::boards::boards_login_with_browser,
            commands::boards::boards_import_cookies,
            commands::boards::boards_logout,
            commands::boards::boards_get_status,
            commands::boards::boards_list,
            commands::boards::boards_catalog,
            // privacy
            commands::privacy::privacy_clear_data,
            commands::privacy::privacy_clear_interactions,
            commands::privacy::privacy_sign_out_all,
            commands::privacy::privacy_reset_app,
            // support
            commands::support::support_export_diagnostics,
            commands::support::support_get_system_info,
            // dialog
            commands::dialog::dialog_open_files,
            // geocoding
            commands::geocoding::geocode_suggest,
            // autopilot
            commands::autopilot::autopilot_list,
            commands::autopilot::autopilot_get,
            commands::autopilot::autopilot_create,
            commands::autopilot::autopilot_update,
            commands::autopilot::autopilot_remove,
            commands::autopilot::autopilot_run,
            commands::autopilot::autopilot_pause,
            commands::autopilot::autopilot_resume,
            commands::autopilot::autopilot_take_pending_focus,
            // ai generations
            commands::ai_generations::ai_generations_list,
            commands::ai_generations::ai_generations_save,
            commands::ai_generations::ai_generations_update,
            commands::ai_generations::ai_generations_remove,
            commands::ai_generations::ai_generations_remove_bulk,
            // applications (status-bearing aggregate — ADR 0001)
            commands::applications::applications_list,
            commands::applications::applications_get,
            commands::applications::applications_set_status,
            commands::applications::applications_update,
            commands::applications::applications_delete,
            commands::applications::applications_track,
            commands::applications::applications_save_from_posting,
            // notification center (Phase 2 — IPC seam over the persisted store)
            commands::notifications::notifications_list,
            commands::notifications::notifications_mark_read,
            commands::notifications::notifications_mark_all_read,
            commands::notifications::notifications_remove,
            commands::notifications::notifications_clear_all,
            commands::notifications::notifications_clicked,
            // referrals (manual referral helper)
            commands::referrals::referrals_list,
            commands::referrals::referrals_upsert,
            commands::referrals::referrals_remove,
            // profile import
            commands::profile_import::profile_import_from_url,
            // github repos import (resume-builder projects step)
            commands::github::github_import_repos,
            // browser-extension bridge (Feature 2 — loopback WS control)
            commands::extension_bridge::extension_bridge_status,
            commands::extension_bridge::extension_bridge_regenerate_token,
            commands::extension_bridge::extension_bridge_autofill_enabled,
            commands::extension_bridge::extension_bridge_set_autofill_enabled,
            commands::extension_bridge::extension_bridge_ai_assist_enabled,
            commands::extension_bridge::extension_bridge_set_ai_assist_enabled,
            commands::extension_bridge::extension_bridge_auto_track_enabled,
            commands::extension_bridge::extension_bridge_set_auto_track_enabled,
            // export
            export::commands::documents_export_document,
            export::commands::documents_export_and_save,
            export::commands::documents_render_preview_images,
            // updater
            updater::updater_check,
            updater::updater_download,
            updater::updater_install,
            updater::updater_changelog,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}

#[cfg(test)]
mod tests {
    use super::is_native_host_launch;

    /// Build the argv tail a browser would pass (owned `String`s, as
    /// `std::env::args` yields).
    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn windows_firefox_manifest_path_is_native_host() {
        // The regression: the `.firefox` infix broke the old `ends_with` match.
        assert!(is_native_host_launch(args(&[
            r"C:\Users\me\AppData\...\app.aijobhunter.bridge.firefox.json",
            "job-importer@aijobhunter.app",
        ])));
    }

    #[test]
    fn windows_chrome_manifest_path_is_native_host() {
        assert!(is_native_host_launch(args(&[
            r"C:\Users\me\...\app.aijobhunter.bridge.chrome.json",
        ])));
    }

    #[test]
    fn unix_manifest_path_is_native_host() {
        assert!(is_native_host_launch(args(&[
            "/home/me/.mozilla/native-messaging-hosts/app.aijobhunter.bridge.json",
        ])));
    }

    #[test]
    fn chrome_extension_origin_is_native_host() {
        assert!(is_native_host_launch(args(&[
            "chrome-extension://oaoekkgkhmgdfnpmfkpphgiikliaicll/",
        ])));
    }

    #[test]
    fn deep_link_launch_is_not_native_host() {
        assert!(!is_native_host_launch(args(&["ajh://settings/extension"])));
    }

    #[test]
    fn empty_launch_is_not_native_host() {
        assert!(!is_native_host_launch(args(&[])));
    }

    #[test]
    fn unrelated_json_arg_is_not_native_host() {
        // A `.json` arg that does NOT contain the host name must not match.
        assert!(!is_native_host_launch(args(&[
            "/tmp/some-other-config.json"
        ])));
    }

    #[test]
    fn host_name_in_directory_is_not_native_host() {
        // The host name in a DIRECTORY component (not the filename) must NOT match —
        // only the manifest basename counts.
        assert!(!is_native_host_launch(args(&[
            "/opt/app.aijobhunter.bridge/other.json"
        ])));
    }
}
