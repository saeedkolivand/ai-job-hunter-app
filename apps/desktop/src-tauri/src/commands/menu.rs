//! Native-menu IPC commands.
//!
//! The shell delivers menu intents (`menu:navigate` / `menu:action`) by emitting
//! to the renderer. But `emit` is fire-and-forget: when the window was hidden
//! (close-to-tray) the WebView2 is suspended, so an emit fired right after the
//! window is un-hidden lands before the resumed webview re-attaches its
//! listeners and is dropped. To cover that, `tray::dispatch_menu` buffers the
//! intent in [`crate::tray::PendingMenu`] and the renderer PULLS it via
//! [`menu_take_pending`] once its JS loop is provably live (on mount + on window
//! focus/visibility-restore). The IPC response is reliable where the event was
//! not.

/// A menu intent buffered shell-side, returned to the renderer. `event` is the
/// same name the shell would otherwise `emit` (`menu:navigate` / `menu:action`);
/// `payload` is its untyped JSON (the renderer discriminates on `event`).
#[derive(Debug, PartialEq, serde::Serialize)]
pub struct PendingMenuIntent {
    pub event: String,
    pub payload: serde_json::Value,
}

/// Take + clear the buffered intent. Split from the command so it's unit-testable
/// without a Tauri `State`. Atomic: the lock is held across take+map.
pub(crate) fn take_pending(buf: &crate::tray::PendingMenu) -> Option<PendingMenuIntent> {
    buf.0
        .lock()
        .take()
        .map(|(event, payload)| PendingMenuIntent { event, payload })
}

/// Atomically take + clear the buffered menu intent. Returns `None` when nothing
/// is buffered (the common case — only set while the window was hidden).
#[tauri::command]
pub fn menu_take_pending(
    state: tauri::State<'_, crate::tray::PendingMenu>,
) -> Option<PendingMenuIntent> {
    take_pending(state.inner())
}

#[cfg(test)]
mod tests {
    use parking_lot::Mutex;
    use serde_json::json;

    use super::*;
    use crate::tray::PendingMenu;

    #[test]
    fn returns_buffered_intent_then_clears() {
        let payload = json!({ "route": "/settings", "section": null });
        let buf = PendingMenu(Mutex::new(Some((
            "menu:navigate".to_string(),
            payload.clone(),
        ))));

        assert_eq!(
            take_pending(&buf),
            Some(PendingMenuIntent {
                event: "menu:navigate".to_string(),
                payload,
            })
        );
        // Atomic take cleared the slot — a second pull (e.g. a later focus) is empty,
        // so an intent is delivered exactly once and can't re-fire.
        assert_eq!(take_pending(&buf), None);
    }

    #[test]
    fn returns_none_when_empty() {
        let buf = PendingMenu(Mutex::new(None));
        assert_eq!(take_pending(&buf), None);
    }
}
