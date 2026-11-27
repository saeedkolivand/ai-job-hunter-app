//! Reload the main webview after its page process dies (issue #1578).
//!
//! A hidden-to-tray window whose web process crashed otherwise restores as a
//! blank page. macOS needs nothing here: Tauri's default
//! `on_web_content_process_terminate` already reloads. Linux (WebKitGTK) and
//! Windows (WebView2) have no default, so we add our own handler; both engines
//! allow several, so this coexists with `tauri-plugin-sentry`'s capture handler.
// The guard is only reached from the Linux/Windows `install`; elsewhere it's unused.
#![cfg_attr(
    not(any(windows, target_os = "linux")),
    allow(dead_code, unused_imports)
)]

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{Runtime, WebviewWindow};

/// A second crash inside this window is treated as a crash loop: no reload.
const RELOAD_COOLDOWN: Duration = Duration::from_secs(30);

/// Pure crash-loop guard: reload unless the previous reload was < cooldown ago.
fn should_reload(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|t| now.saturating_duration_since(t) >= RELOAD_COOLDOWN)
}

/// Records a crash and answers whether to reload now.
#[cfg(any(windows, target_os = "linux"))]
fn admit(last: &Mutex<Option<Instant>>) -> bool {
    let now = Instant::now();
    let mut guard = last
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if should_reload(*guard, now) {
        *guard = Some(now);
        true
    } else {
        log::warn!("[webview-recovery] repeated crash within cooldown; not reloading");
        false
    }
}

/// Install the recovery handler on `window`'s webview. Call once, at setup.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn install<R: Runtime>(_window: &WebviewWindow<R>) {
    // macOS: covered by Tauri's default reload. Other targets: unsupported.
}

#[cfg(target_os = "linux")]
pub fn install<R: Runtime>(window: &WebviewWindow<R>) {
    use webkit2gtk::{WebProcessTerminationReason, WebViewExt};

    let last = Mutex::new(None);
    let _ = window.with_webview(move |platform| {
        platform
            .inner()
            .connect_web_process_terminated(move |view, reason| {
                // TerminatedByApi is our own doing; never reload on it.
                if matches!(
                    reason,
                    WebProcessTerminationReason::Crashed
                        | WebProcessTerminationReason::ExceededMemoryLimit
                ) && admit(&last)
                {
                    view.reload();
                }
            });
    });
}

#[cfg(windows)]
pub fn install<R: Runtime>(window: &WebviewWindow<R>) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
    };
    use webview2_com::ProcessFailedEventHandler;

    let last = Mutex::new(None);
    let _ = window.with_webview(move |platform| {
        // ponytail: BROWSER_PROCESS_EXITED needs a full webview recreate and
        // RENDER_PROCESS_UNRESPONSIVE is still alive; both out of scope.
        // SAFETY: COM calls on the UI thread (with_webview) against a live
        // controller; the handler only reads its own args.
        let Ok(core) = (unsafe { platform.controller().CoreWebView2() }) else {
            return;
        };
        // Only the main frame's renderer: a subframe (FRAME_RENDER_PROCESS_EXITED)
        // crash leaves the page alive, so reloading would discard state for nothing.
        // Reload via the `sender` arg, not a captured clone of `core` (that would be
        // a core -> handler -> core reference cycle).
        let handler = ProcessFailedEventHandler::create(Box::new(move |sender, args| {
            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
            // SAFETY: `kind` is a valid out-param; `a` is the live event args.
            let died = args.is_some_and(|a| unsafe { a.ProcessFailedKind(&mut kind) }.is_ok())
                && kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED;
            if let Some(sender) = sender.filter(|_| died && admit(&last)) {
                // SAFETY: invoked on the WebView2 UI thread that owns `sender`.
                let _ = unsafe { sender.Reload() };
            }
            Ok(())
        }));
        let mut token = 0;
        // SAFETY: `core` is live; WebView2 AddRefs the handler.
        if let Err(e) = unsafe { core.add_ProcessFailed(&handler, &mut token) } {
            log::warn!("[webview-recovery] ProcessFailed handler not installed: {e}");
        }
    });
}

#[cfg(test)]
mod tests;
