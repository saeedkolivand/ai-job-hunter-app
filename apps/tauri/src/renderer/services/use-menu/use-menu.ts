import { useEffect } from 'react';

import type { MenuActionEvent, MenuNavigateEvent } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

/**
 * Deliver native-menu intents (Settings / Check-Updates / nav / shortcuts) from
 * BOTH the system tray and the macOS menu bar, reliably, in every window state.
 *
 * The shell's Rust→JS `emit` is fire-and-forget with no per-listener queue, so a
 * `menu.navigate` / `menu.action` fired right after a menu click can be dropped:
 * the webview is suspended (close-to-tray), it hasn't re-attached its listeners
 * yet, or WebView2 defers IPC while the tray menu holds the foreground. So the
 * shell instead BUFFERS the intent (`tray::PendingMenu`) and the renderer PULLS
 * it (`menu_take_pending`) over a reliable IPC response.
 *
 * The buffer is the single source of truth; everything else is just a trigger to
 * drain it:
 *  - the emitted event — low-latency; covers the visible macOS-menu-bar case
 *    where the window never loses focus (so no focus/visibility change fires);
 *  - window `focus` + visibility-restore — cover the tray case (clicking the menu
 *    stole then returned focus) and the hidden → shown restore;
 *  - mount — anything buffered before the listeners attached.
 *
 * `takePending` takes-and-clears atomically, so no matter how many triggers fire,
 * the intent is delivered exactly once and never re-fires on a later unrelated
 * focus.
 */
export const useMenuIntents = (
  onNavigate?: (event: MenuNavigateEvent) => void,
  onAction?: (event: MenuActionEvent) => void
) => {
  const api = useAppClient();
  useEffect(() => {
    let cancelled = false;
    const drain = async () => {
      const intent = await api.menu.takePending();
      if (cancelled || !intent) return;
      if (intent.event === 'menu.navigate') onNavigate?.(intent.payload);
      else if (intent.event === 'menu.action') onAction?.(intent.payload);
    };
    const trigger = () => void drain();
    const onVisibility = () => {
      if (document.visibilityState === 'visible') void drain();
    };

    void drain(); // mount: drain anything buffered before listeners attached
    window.addEventListener('focus', trigger);
    document.addEventListener('visibilitychange', onVisibility);
    // The shell also emits the event as a low-latency trigger; we ignore its
    // payload and always drain the buffer so delivery is exactly-once.
    const offNavigate = api.menu.onNavigate(trigger);
    const offAction = api.menu.onAction(trigger);

    return () => {
      cancelled = true;
      window.removeEventListener('focus', trigger);
      document.removeEventListener('visibilitychange', onVisibility);
      offNavigate?.();
      offAction?.();
    };
  }, [api, onNavigate, onAction]);
};
