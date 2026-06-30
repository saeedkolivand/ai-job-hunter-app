//! Live OS accent-color watcher (Windows + macOS).
//!
//! The OS has no media query for the personalization accent color (unlike
//! light/dark, which the renderer tracks via `matchMedia`). To make the app
//! repaint live when the user changes their accent, we subscribe to the
//! platform's accent-change signal and emit [`SYSTEM_ACCENT_CHANGED`] to the
//! renderer, which re-pulls `system_accent_color` and re-applies the theme:
//!
//! - **Windows** — the WinRT `UISettings::ColorValuesChanged` event.
//! - **macOS** — the distributed notification `AppleColorPreferencesChangedNotification`
//!   delivered via `CFNotificationCenter` (the distributed center).
//! - **Linux** — no watcher; the renderer's window-focus refetch covers it.
//!
//! We do NOT read the new color here on any platform — the renderer owns
//! resolution via the existing command, keeping the Rust side a pure signal.
//!
//! Lifetime: the platform subscription handle MUST stay alive for the whole app
//! or it silently stops firing — the Windows `UISettings` instance + event token,
//! and the macOS CF observer registration. We park each in Tauri-managed state
//! ([`AccentWatcher`] / [`MacAccentWatcher`]) so they live as long as the app
//! does and are torn down cleanly on drop.

#[cfg(windows)]
use crate::events::{emit_event, SYSTEM_ACCENT_CHANGED};
#[cfg(target_os = "macos")]
use crate::events::{emit_event, SYSTEM_ACCENT_CHANGED};
#[cfg(target_os = "macos")]
use core_foundation_sys::base::{kCFAllocatorDefault, CFRelease};
#[cfg(target_os = "macos")]
use core_foundation_sys::notification_center::{
    CFNotificationCenterAddObserver, CFNotificationCenterGetDistributedCenter,
    CFNotificationCenterRef, CFNotificationCenterRemoveEveryObserver,
    CFNotificationSuspensionBehaviorDeliverImmediately,
};
#[cfg(target_os = "macos")]
use core_foundation_sys::string::{kCFStringEncodingUTF8, CFStringCreateWithBytes, CFStringRef};
#[cfg(target_os = "macos")]
use std::ffi::c_void;
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

/// Keeps the macOS distributed-notification observer registered for the app
/// lifetime. We register the (leaked) `AppHandle` box as the CF `observer`
/// pointer; dropping this removes the observer so the callback can no longer
/// fire on a freed handle. Managed in Tauri state by [`start`], mirroring the
/// Windows [`AccentWatcher`].
#[cfg(target_os = "macos")]
pub struct MacAccentWatcher {
    /// Distributed notification center (process-global singleton; not owned, so
    /// it is never released — held only to deregister the observer on drop).
    center: CFNotificationCenterRef,
    /// The `AppHandle` we leaked into a raw pointer to pass as the CF observer.
    /// Reclaimed in `Drop` AFTER the observer is removed, so the callback can
    /// never run against a freed handle.
    observer: *mut AppHandle,
}

#[cfg(target_os = "macos")]
impl Drop for MacAccentWatcher {
    fn drop(&mut self) {
        // SAFETY: `center` is the distributed center we registered against and
        // `observer` is the leaked AppHandle pointer we passed as the CF
        // observer. We remove the observer FIRST so no in-flight callback can
        // dereference the handle, THEN reclaim (free) the box. Order matters.
        unsafe {
            CFNotificationCenterRemoveEveryObserver(self.center, self.observer as *const c_void);
            drop(Box::from_raw(self.observer));
        }
    }
}

// SAFETY: `center` is a process-global singleton handle (never mutated, only
// used to deregister the observer); `observer` is only dereferenced on the main
// run-loop callback thread and freed exactly once on drop. We otherwise only
// ever store the struct in Tauri-managed state, so sharing it across threads is
// sound.
#[cfg(target_os = "macos")]
unsafe impl Send for MacAccentWatcher {}
#[cfg(target_os = "macos")]
unsafe impl Sync for MacAccentWatcher {}

/// CF distributed-notification callback. Matches the
/// `core_foundation_sys::notification_center::CFNotificationCallback` ABI
/// exactly (`CFNotificationName == CFStringRef`).
#[cfg(target_os = "macos")]
extern "C" fn on_accent_changed(
    _center: CFNotificationCenterRef,
    observer: *mut c_void,
    _name: core_foundation_sys::string::CFStringRef,
    _object: *const c_void,
    _user_info: core_foundation_sys::dictionary::CFDictionaryRef,
) {
    // `observer` is the leaked AppHandle pointer we registered. Borrow it
    // (do NOT take ownership — the holder's Drop frees it) and emit. The
    // renderer re-pulls `system_accent_color` (defaults read -g AppleAccentColor)
    // and re-applies; we never read the color here.
    if observer.is_null() {
        return;
    }
    // SAFETY: `observer` is the `*mut AppHandle` we passed to
    // CFNotificationCenterAddObserver; it stays valid until MacAccentWatcher
    // is dropped, which first removes this observer. AppHandle is Send+Sync.
    let app = unsafe { &*(observer as *const AppHandle) };
    emit_event(app, SYSTEM_ACCENT_CHANGED, ());
}

/// Observe the macOS distributed notification posted when the user changes the
/// accent / highlight color (`AppleColorPreferencesChangedNotification`) and
/// emit [`SYSTEM_ACCENT_CHANGED`] on each fire. Call once from the Tauri `setup`
/// flow. Best-effort: if the distributed center or the notification-name string
/// can't be created, it logs and leaves the renderer's window-focus refetch as
/// the fallback. The observer is anchored in Tauri-managed state for the app
/// lifetime — CF observers stop firing if dropped.
#[cfg(target_os = "macos")]
pub fn start(app: &AppHandle) {
    use tauri::Manager;

    // SAFETY: the distributed center is a process-global singleton; the call
    // returns a borrowed (non-owned) pointer.
    let center = unsafe { CFNotificationCenterGetDistributedCenter() };
    if center.is_null() {
        log::warn!("[accent-watcher] distributed notification center unavailable; live accent disabled (macOS)");
        return;
    }

    // The notification name macOS posts when the accent / highlight color changes.
    const NAME: &[u8] = b"AppleColorPreferencesChangedNotification";
    // SAFETY: NAME is valid UTF-8 bytes with a known length; CFStringCreateWithBytes
    // copies them. We own the returned string and release it before returning.
    let cf_name: CFStringRef = unsafe {
        CFStringCreateWithBytes(
            kCFAllocatorDefault,
            NAME.as_ptr(),
            NAME.len() as core_foundation_sys::base::CFIndex,
            kCFStringEncodingUTF8,
            false as core_foundation_sys::base::Boolean,
        )
    };
    if cf_name.is_null() {
        log::warn!("[accent-watcher] failed to create CFString for notification name; live accent disabled (macOS)");
        return;
    }

    // Leak the AppHandle into a raw pointer used as the CF `observer`. The
    // holder's Drop reclaims it after removing the observer.
    let observer = Box::into_raw(Box::new(app.clone()));

    // SAFETY: `center` is the distributed center; `on_accent_changed` matches
    // CFNotificationCallback; `cf_name` is a valid CFString; `observer` is a
    // valid leaked AppHandle pointer kept alive by MacAccentWatcher. object=null
    // observes the name from any sender.
    unsafe {
        CFNotificationCenterAddObserver(
            center,
            observer as *const c_void,
            on_accent_changed,
            cf_name,
            std::ptr::null(),
            CFNotificationSuspensionBehaviorDeliverImmediately,
        );
        // CFNotificationCenterAddObserver retains the name; release our copy.
        CFRelease(cf_name as *const c_void);
    }

    // Anchor the observer for the app lifetime (dropping it removes the observer
    // and frees the AppHandle box).
    app.manage(MacAccentWatcher { center, observer });
}

/// No-op on platforms without an accent watcher (Linux). The renderer's
/// `refetchOnWindowFocus` fallback re-pulls the accent on focus there.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn start(_app: &AppHandle) {}
