//! The Tauri `setup` hook: what runs once the app exists but before the window is
//! shown. Split out of `lib.rs` for R8 (issue #1280); the order of every step below
//! is the order it always ran in — state is managed before the cold-start deep link
//! is handled, the stores before the schedulers that read them.

use parking_lot::Mutex;
use tauri::Manager;

use super::state;
use crate::observability::sanitize_reason;
use crate::{
    app_menu, autopilot_scheduler, commands, email_watch_scheduler, extension_bridge,
    notifications, platform, reminder_scheduler, tray, updater,
};

/// Build the app's managed state, menu, tray and background tasks.
pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Global shortcuts are a desktop-only capability; register the plugin
    // here (gated) rather than in the always-compiled builder chain so a
    // mobile/other-target build is not broken. No shortcuts are bound yet
    // — this only makes the `global-shortcut:default` capability available.
    #[cfg(desktop)]
    if let Err(e) = app
        .handle()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    {
        log::warn!(
            "[setup] global-shortcut plugin init failed (non-fatal): {}",
            sanitize_reason(&e.to_string())
        );
    }

    let handle = app.handle();

    // Buffers for the cold-start deep-link intents (autopilot-focus
    // `ajh://autopilot/<id>` and menu-intent `ajh://settings/extension` /
    // `ajh://open?url=…` etc.), managed HERE — before the cold-start
    // deep-link block below, which runs during setup, well before
    // `tray::build`. `dispatch_focus` writes the id into `PendingFocus`
    // and `dispatch_menu` writes its intent into `PendingMenu` BEFORE
    // emitting, so both must already be in state when the cold-start
    // deep link is handled — a `try_state` miss silently no-ops the
    // write and drops the intent on a cold launch (the window focuses,
    // nothing navigates).
    app.manage(tray::PendingFocus(Mutex::new(None)));
    app.manage(tray::PendingMenu(Mutex::new(None)));

    // `ajh://` deep links. The OS routes a cold/click-launched URL here
    // (macOS via `on_open_url`; Windows/Linux a second instance forwards
    // it as argv → the single-instance guard above). Every URL is funneled
    // through the same strict allowlist (`deeplink::parse_focus_target`)
    // before any navigation — a hostile URL focuses the window and stops.
    #[cfg(desktop)]
    super::deep_link::register(app);

    // Data dir for all persistent state. Resolved + exported once here so
    // AppHandle-less workers (scrapers/appliers) reach the same path.
    // All path/env knowledge lives in `platform::config`.
    let data_dir = platform::config::resolve_and_export_data_dir(handle);

    let mut reset_registry = commands::privacy::ResetRegistry::default();
    state::manage_user_stores(app, &mut reset_registry, &data_dir);
    let scraper_engine = state::manage_process_state(app);
    state::manage_late_stores(app, &mut reset_registry, &data_dir, &scraper_engine);

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
    let menu = app_menu::build_app_menu(handle)?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| app_menu::on_app_menu_event(app, event.id().as_ref()));

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
        log::warn!(
            "[setup] tray build error (non-fatal): {}",
            sanitize_reason(&e.to_string())
        );
    }

    // Schedule background update checks (10 s after launch, then every
    // 4 h) — on an NSIS/MSI install. A Store build announces once that
    // the Store owns updates and never polls or checks; see
    // `updater::setup_auto_check`.
    updater::setup_auto_check(handle);

    // Start autopilot schedule runner (checks every minute).
    autopilot_scheduler::start(handle.clone());

    // Start the email-confirmation watch scheduler (task #23, auto-track
    // Layer C) — a no-op sweep whenever the feature is disconnected/
    // disabled/no credential; see `email_watch_scheduler` for the
    // due/backoff logic.
    email_watch_scheduler::start(handle.clone());

    // Start the follow-up reminder sweep: a due/overdue `nextActionAt`
    // on an open Application raises one notification per due date. No
    // network, one indexed SQLite read per tick — see
    // `reminder_scheduler` for the dedupe/cap rules.
    reminder_scheduler::start(handle.clone());

    // Start the browser-extension WS bridge (loopback only). Fire-and-
    // forget on the tokio runtime; a bind failure logs + disables the
    // bridge and never blocks boot. `BridgeState` was managed above.
    extension_bridge::start(handle.clone());

    // Keep the native-messaging host manifests + OS registration current
    // (path tracks app moves/updates) so the browser can spawn our stdio
    // relay — the Firefox HTTPS-Only transport that survives the ws→wss
    // upgrade. Best-effort; never blocks boot.
    extension_bridge::register::register_native_host(&data_dir);

    // Build the bundled offline geocoding index now, off the hot path.
    // It is 60-250 ms of pure CPU; left lazy it lands on whichever
    // command worker touches it first, including the one inside
    // `derive_country_code`'s `tokio::time::timeout` — which cannot
    // interrupt synchronous work, so that 2 s cap would silently not
    // apply. `spawn_blocking` (not `spawn`) because this is CPU-bound,
    // and via `tauri::async_runtime` so it uses the app's runtime.
    tauri::async_runtime::spawn_blocking(commands::geocoding::warm_index);

    // Watch the OS accent color (Windows): on a personalization accent
    // change, emit `system:accentChanged` so the renderer re-pulls the
    // color and re-applies the theme live. The watcher parks its WinRT
    // subscription in managed state to stay alive. No-op off Windows;
    // there the renderer's window-focus refetch covers it. Best-effort.
    platform::accent_watcher::start(handle);

    Ok(())
}
