//! `ajh://` deep-link delivery: the shared dispatcher, the rejected-link log line
//! and the setup-time registration + cold-start handling. Split out of `lib.rs`
//! for R8 (issue #1280).

use tauri::AppHandle;

use crate::{deeplink, tray};

/// Drive the renderer for a validated deep-link target. Shared by every delivery
/// path (single-instance relaunch, `on_open_url`, cold first-instance launch) so
/// they stay in lockstep. `None` (a hostile/unrecognized URL) navigates nowhere
/// — the caller has already focused the window. Both arms use a cold-start-robust
/// buffered intent so the signal survives a renderer that hasn't attached its
/// listeners yet (the cold first-instance launch fires during Rust setup):
/// autopilot buffers in `PendingFocus` (`dispatch_focus`); pairing buffers the
/// `menu:navigate` intent (`dispatch_extension_pairing`).
pub(crate) fn handle_deep_link(app: &AppHandle, target: Option<deeplink::FocusTarget>) {
    match target {
        Some(deeplink::FocusTarget::Autopilot(id)) => tray::dispatch_focus(app, &id),
        Some(deeplink::FocusTarget::ExtensionPairing) => tray::dispatch_extension_pairing(app),
        Some(deeplink::FocusTarget::GenerateForJob(url)) => {
            tray::dispatch_generate_for_job(app, &url)
        }
        Some(deeplink::FocusTarget::OpenJob(url)) => tray::dispatch_open_job(app, &url),
        Some(deeplink::FocusTarget::PrepForJob(url)) => tray::dispatch_prep_for_job(app, &url),
        None => {}
    }
}

/// Warn about a rejected `ajh://` deep-link argv so a hostile/malformed deep
/// link is diagnosable — today nothing on the deep-link path logs a rejection
/// (the tray info line only fires for an ACCEPTED target). Called by each
/// delivery path (single-instance relaunch, `on_open_url`, cold start) exactly
/// when [`deeplink::parse_focus_target`] returns `None`, so the parser stays
/// pure. Path privacy: logs only the ACTION segment (the first path segment
/// after [`deeplink::SCHEME`]), bounded and allowlist-filtered by
/// [`deeplink::sanitize_action_for_log`] — never the full URL, never the `url=`
/// query value (a job URL is user data), no raw control characters.
pub(crate) fn log_rejected_deep_link(argv: &[String]) {
    let Some(rest) = argv
        .iter()
        .find_map(|arg| arg.trim().strip_prefix(deeplink::SCHEME))
    else {
        return;
    };
    let action = rest.split(['/', '?', '#', '\\']).next().unwrap_or("");
    let Some(action) = deeplink::sanitize_action_for_log(action) else {
        return; // nothing allowlisted survived — skip the log line entirely
    };
    log::warn!("[deeplink] rejected ajh://{action} — not an allowlisted deep-link target");
}

/// Register the `ajh://` scheme and route a cold-start link. Desktop-only, like the
/// plugin it drives; runs from `setup` after the pending-intent buffers are managed.
#[cfg(desktop)]
pub(super) fn register(app: &tauri::App) {
    let handle = app.handle();

    use tauri_plugin_deep_link::DeepLinkExt;
    // Register the scheme at runtime (no-op once the installer has;
    // required for Linux + `pnpm dev`). Best-effort.
    //
    // NEVER on a Microsoft Store build. There the manifest's
    // `windows.protocol` extension IS the registration, and because
    // the package disables registry write virtualization this call's
    // `HKCU\Software\Classes\ajh` write would be REAL: it would
    // shadow the package activation with a command line pinned to
    // the current version's WindowsApps path — a directory the user
    // cannot execute from, and one that changes with every Store
    // update — and it would outlive an uninstall. `on_open_url`
    // below is untouched; that is how the packaged activation
    // arrives.
    //
    // Deliberately `msix::is_packaged()`, not the
    // `platform::is_packaged_build()` aggregator: the HKCU-write
    // shadowing above is Windows/registry-specific, and Snap has
    // no analogous "this write would shadow the manifest's own
    // registration" hazard — its desktop-file registration
    // doesn't go through this call at all. Do not "fix" this
    // into the aggregator.
    if !crate::platform::msix::is_packaged() {
        let _ = app.deep_link().register_all();
    }
    let dl_handle = handle.clone();
    app.deep_link().on_open_url(move |event| {
        let urls: Vec<String> = event.urls().iter().map(|u| u.to_string()).collect();
        // `show_focus` so a deep-link reopen also restores the Dock
        // icon (macOS) when hidden to the tray. (The pairing path's
        // `dispatch_menu` calls `show_focus` itself, so calling it
        // first here is a harmless no-op for that target.)
        tray::show_focus(&dl_handle);
        let target = deeplink::parse_focus_target(&urls);
        if target.is_none() {
            log_rejected_deep_link(&urls);
        }
        handle_deep_link(&dl_handle, target);
    });

    // Cold start: when the app was NOT already running, the OS launches
    // it FRESH with the `ajh://…` URL and the single-instance callback
    // never fires (it only triggers on a *second* launch). On Windows/
    // Linux that first-instance URL arrives on argv; on macOS the plugin
    // surfaces it via `get_current()`. Parse both so a not-running launch
    // (the primary case for `ajh://settings/extension`) still routes.
    let cold_argv: Vec<String> = std::env::args_os()
        .filter_map(|a| a.into_string().ok())
        .collect();
    let current_urls = app
        .deep_link()
        .get_current()
        .ok()
        .flatten()
        .map(|urls| urls.iter().map(|u| u.to_string()).collect::<Vec<_>>());
    let initial = deeplink::parse_focus_target(&cold_argv).or_else(|| {
        current_urls
            .as_deref()
            .and_then(deeplink::parse_focus_target)
    });
    if initial.is_none() {
        // The URL may have arrived via the plugin (`get_current`, macOS
        // cold start) rather than argv — log a rejected action from
        // either source once.
        let mut candidates = cold_argv;
        if let Some(urls) = &current_urls {
            candidates.extend(urls.iter().cloned());
        }
        log_rejected_deep_link(&candidates);
    }
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
