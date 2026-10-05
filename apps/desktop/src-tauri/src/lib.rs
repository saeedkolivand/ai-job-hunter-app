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
// Edition 2024 legalizes let-chains (`if let X && let Y { .. }`), which makes
// this lint newly fire at ~96 pre-existing nested `if`/`if let` sites across
// every domain in the crate — none of them wrong, all of them now
// "collapsible" only because the target syntax became legal. Nothing here is
// a defect: this is a style modernization, deliberately deferred rather than
// bundled into the edition bump (a 96-site multi-domain refactor is not a
// mechanical flag flip). Remove this once that cleanup lands.
//
// Two limits worth knowing before you trust or move this:
// * It is WIDER than its own justification. `collapsible_if` also covers plain
//   `if a { if b { } }`, which has nothing to do with let-chains — so this
//   leaves a small genuine lint-coverage hole, not just a let-chain deferral.
// * It covers THIS crate root only. `main.rs`, `benches/` and `tests/` are
//   separate crate roots and do not inherit it, so a new nested `if let` there
//   hard-fails `-D warnings` while identical code under `src/` passes. All 96
//   current sites are in the lib, so this bites nobody today.
#![allow(clippy::collapsible_if)]

pub mod ai_config;
pub mod ai_generations;
pub mod ai_provider;
/// The native app menu (build + click handler) — see its own module doc (R8 relief, PR4).
pub mod app_menu;
pub mod applications;
pub mod autopilot;
pub mod autopilot_helpers;
pub mod autopilot_scheduler;
pub mod commands;
pub mod contact_profile;
pub mod cover_letter;
pub mod crash_reporting;
pub mod credentials;
pub mod data_store;
pub mod db;
pub mod dedup;
pub mod deeplink;
pub mod discovered;
pub mod documents;
pub mod email_watch;
pub mod email_watch_scheduler;
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
pub mod prompt_fence;
pub mod recommend;
pub mod referrals;
pub mod reminder_scheduler;
pub mod retrieval;
pub mod salary_research;
pub mod scraping;
mod shell;
pub mod spend;
#[cfg(test)]
mod tests;
pub mod theme;
pub mod tray;
pub mod updater;
pub mod validate;
pub mod vector;

use parking_lot::Mutex;

use tauri::Manager;

use observability::sanitize_reason;

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

/// `agent <verb>` argv sentinel (issue #1084 PR 1) — mirrors
/// [`run_native_host_if_invoked`]'s shape exactly. `main.rs` calls this BELOW
/// the native-host short-circuit and ABOVE `run()`: placement is
/// load-bearing in both directions — `run()`'s first act forks the minidump
/// supervisor process (everything above that fork line runs in BOTH
/// processes), and the single-instance plugin `run()` installs would
/// otherwise hand this argv to an already-running GUI instance, pop its
/// window, and exit with no stdout at all — exactly the failure this
/// short-circuit exists to avoid.
///
/// Detected purely from argv (`agent` as the FIRST post-exe token) — our deep
/// links use the `ajh://` scheme and the native-host launch is detected by
/// [`is_native_host_launch`]'s own distinct argv shapes, so neither collides
/// with this one.
pub fn run_agent_cli_if_invoked() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !is_agent_cli_launch(&args) {
        return None;
    }
    Some(extension_bridge::agent_cli::run(&args[1..]))
}

/// True when the FIRST post-exe argv token is exactly the `agent` sentinel.
/// Extracted for the same reason as [`is_native_host_launch`]: argv is
/// process-global and can't be set in a test, so the predicate is
/// unit-tested against a synthetic slice instead.
fn is_agent_cli_launch(args: &[String]) -> bool {
    args.first().map(String::as_str) == Some("agent")
}

/// Build and run the Tauri application. Called by the binary shim in `main.rs`.
pub fn run() {
    // Remote crash reporting is initialised before ANYTHING else, for two
    // reasons that both cut the same way:
    //   1. `sentry`'s `MinidumpIntegration` FORKS a crash-reporter process inside
    //      `crash_reporting::init()`'s `sentry::init`. Everything above that call
    //      therefore executes in *both* processes, so it must stay cheap and
    //      side-effect-free — keyring init, the Tauri builder, and the panic hook
    //      all deliberately live below. (The integration keeps the reporter
    //      handle for the client's lifetime, which `sentry_guard` holds for the
    //      whole of `run()`.)
    //   2. `[profile.release] panic = "abort"` means nothing in-process can
    //      outlive a crash to flush it, and a native crash during startup is
    //      only captured if the supervisor is already watching by then.
    //
    // `None` whenever there is no baked-in DSN (every non-release build) or the
    // user has not consented — no client is constructed at all, so there is
    // nothing that could transmit, rather than a client sampled to zero.
    let sentry_guard = crash_reporting::init();

    // ── Below here runs in the app process only ──────────────────────────────

    shell::install_crash_log_hook();

    // Initialise the OS keyring up front. A failure here is non-fatal: the app
    // still boots, and credential operations (AI provider keys, factory reset)
    // surface the error later through `AppError` rather than aborting startup.
    if let Err(e) = credentials::init_keyring() {
        log::warn!(
            "[startup] OS keyring unavailable (credential features degraded): {}",
            sanitize_reason(&e.to_string())
        );
    }

    let mut builder = tauri::Builder::default();
    if let Some(client) = sentry_guard.as_ref() {
        // Injects `@sentry/browser` into every WebView, wired to a transport that
        // forwards renderer events and breadcrumbs to THIS Rust client over
        // `invoke`. The WebView never talks to the network itself, so the CSP
        // `connect-src` allowlist in tauri.conf.json is deliberately untouched
        // (widening it is rated HIGH/CRITICAL — docs/knowledge/security-rules.md).
        //
        // Renderer events do NOT all inherit `before_send`, which this comment
        // used to claim. The plugin only routes a renderer envelope through
        // `capture_event` (and therefore through our redaction) when it parses
        // AND contains an event item; everything else it hands to
        // `Client::send_envelope`, which reaches the transport directly. That is
        // why the privacy guarantee is enforced one level lower, in
        // `crash_reporting::transport` — see that module.
        builder = builder.plugin(tauri_plugin_sentry::init(client));
    }

    builder
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
            let target = deeplink::parse_focus_target(&argv);
            if target.is_none() {
                shell::log_rejected_deep_link(&argv);
            }
            shell::handle_deep_link(app, target);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .max_file_size(5_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .level(log::LevelFilter::Warn)
                // Global stays Warn (every ad-hoc `log::warn!`/`log::error!` across
                // the ~80 modules that call them directly is already visible and
                // would drown a diagnostics bundle if the whole crate went to
                // Info). Instead, raise only the app's own operational-tracing
                // targets to Info:
                //
                // `observability` is where EVERY `Span`/`RequestTrace`/`StageTrace`
                // begin/end line actually logs from — `log::info!`'s implicit
                // target is the module the macro is *written* in, not the
                // caller's module, so this single entry covers every current and
                // future `Span::begin("ai"|"scrape"|"apply"|"autopilot"|
                // "applications"|"pipeline:*"|"export", ..)` call site project-wide
                // without a per-domain entry. The one bulk operation that runs
                // this at volume (`ai_reembed_all`, one `[ai]` span pair per
                // job) still stays well under the 5 MB/file budget below.
                .level_for("ajh_tauri::observability", log::LevelFilter::Info)
                // `commands::ai_provider::stream`'s "[ai] stream start/end" lines
                // are plain `log::info!`, not routed through `Span`, so they need
                // their own entry to reach the file for a failed generation.
                .level_for(
                    "ajh_tauri::commands::ai_provider::stream",
                    log::LevelFilter::Info,
                )
                // Surface the company-research brief (logged via `tracing::info!`,
                // bridged to `log` by the `tracing` crate's `log` feature) in the
                // terminal/logs.
                .level_for("ajh_tauri::cover_letter::research", log::LevelFilter::Info)
                // `tray::dispatch_menu`'s "[menu] dispatch …" line is a plain
                // `log::info!` (not routed through `Span`), so the tray target
                // needs its own entry to reach the file for a deep-link diagnosis
                // — it sits below the global `Warn` otherwise.
                .level_for("ajh_tauri::tray", log::LevelFilter::Info)
                // Renderer `console.*` forwarded by `src/log-bridge.ts`. The
                // plugin's `log` command targets these records at
                // `tauri_plugin_log::WEBVIEW_TARGET` ("webview"), which is NOT a
                // Rust module path — without this entry the global `Warn` above
                // drops every forwarded `console.info`, i.e. exactly the
                // "where did the generation get to" breadcrumbs the bridge
                // exists for. The bridge sends no `location` on purpose so the
                // target stays bare `webview`: fern's `level_for` prefix match
                // only walks `::` boundaries, so the `webview:{location}` form
                // would match nothing here. Keep the two in lockstep.
                .level_for(tauri_plugin_log::WEBVIEW_TARGET, log::LevelFilter::Info)
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
        // `app_name` pinned to the kebab-case slug (defaults to productName
        // "AI Job Hunter" otherwise) — the Snap Store's manifest schema
        // rejects a `.desktop` filename containing a space, and the Linux
        // autostart file is named after this value.
        .plugin({
            #[cfg(target_os = "macos")]
            let builder = tauri_plugin_autostart::Builder::new()
                .macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent);
            #[cfg(not(target_os = "macos"))]
            let builder = tauri_plugin_autostart::Builder::new();
            builder.app_name("ai-job-hunter").build()
        })
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
        .setup(shell::setup)
        .invoke_handler(shell::invoke_handler())
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
