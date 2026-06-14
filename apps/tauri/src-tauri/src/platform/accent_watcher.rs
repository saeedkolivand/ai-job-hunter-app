//! Live OS accent-color watcher (Windows-only).
//!
//! Windows has no media query for the personalization accent color (unlike
//! light/dark, which the renderer tracks via `matchMedia`). To make the app
//! repaint live when the user changes their accent, we subscribe to the WinRT
//! `UISettings::ColorValuesChanged` event and emit [`SYSTEM_ACCENT_CHANGED`] to
//! the renderer, which re-pulls `system_accent_color` and re-applies the theme.
//!
//! We do NOT read the new color here — the renderer owns resolution via the
//! existing command, keeping the Rust side a pure signal.
//!
//! Lifetime: the `UISettings` instance and its event-registration token MUST
//! stay alive for the whole app, or the subscription silently stops firing. We
//! park them in Tauri-managed state ([`AccentWatcher`]) so they live as long as
//! the app does. Non-Windows targets are a no-op (macOS live-accent is a
//! documented follow-up; the renderer's window-focus refetch covers it).

#[cfg(windows)]
use crate::events::{emit_event, SYSTEM_ACCENT_CHANGED};
use tauri::AppHandle;

/// Keeps the WinRT `UISettings` subscription alive for the app lifetime. The
/// fields are never read — dropping this de-registers the handler, so it is held
/// purely to anchor the subscription. Managed in Tauri state by [`start`].
#[cfg(windows)]
pub struct AccentWatcher {
    /// The `UISettings` instance the handler is registered on. Must outlive the
    /// token; dropping it tears down the subscription.
    _settings: windows::UI::ViewManagement::UISettings,
    /// Event-registration cookie (kept for symmetry/debuggability; the OS drops
    /// the registration when `_settings` is released at process exit).
    _token: i64,
}

// `UISettings` is an agile WinRT runtime class (free-threaded marshaling), so the
// handle is safe to hold across threads. We only ever store it — never call into
// it off the original apartment — so anchoring it in `Send + Sync` managed state
// is sound.
#[cfg(windows)]
unsafe impl Send for AccentWatcher {}
#[cfg(windows)]
unsafe impl Sync for AccentWatcher {}

/// Subscribe to OS accent-color changes and emit [`SYSTEM_ACCENT_CHANGED`] on
/// each fire. Call once from the Tauri `setup` flow. Best-effort: a failure to
/// create `UISettings` or register the handler logs and leaves the renderer's
/// window-focus refetch as the fallback. No-op on non-Windows.
#[cfg(windows)]
pub fn start(app: &AppHandle) {
    use tauri::Manager;
    use windows::core::IInspectable;
    use windows::Foundation::TypedEventHandler;
    use windows::UI::ViewManagement::UISettings;

    let settings = match UISettings::new() {
        Ok(s) => s,
        Err(e) => {
            log::warn!("[accent-watcher] UISettings unavailable; live accent disabled: {e}");
            return;
        }
    };

    // The callback runs on a background WinRT apartment. `AppHandle` is Send+Sync
    // and `emit` is thread-safe, so we just clone it in and emit. We don't read
    // the new color — the renderer re-pulls it via the existing command (and
    // React Query dedups any rapid double-fire).
    let app_handle = app.clone();
    let handler = TypedEventHandler::<UISettings, IInspectable>::new(move |_sender, _args| {
        emit_event(&app_handle, SYSTEM_ACCENT_CHANGED, ());
        Ok(())
    });

    match settings.ColorValuesChanged(&handler) {
        Ok(token) => {
            // Park the instance + token in managed state so the subscription
            // lives for the app lifetime (dropping `settings` would unregister).
            app.manage(AccentWatcher {
                _settings: settings,
                _token: token,
            });
        }
        Err(e) => {
            log::warn!("[accent-watcher] ColorValuesChanged subscribe failed: {e}");
        }
    }
}

/// No-op on platforms without an accent watcher (macOS/Linux). The renderer's
/// `refetchOnWindowFocus` fallback re-pulls the accent on focus there.
#[cfg(not(windows))]
pub fn start(_app: &AppHandle) {}
