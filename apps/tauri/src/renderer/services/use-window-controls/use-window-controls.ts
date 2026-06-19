import { useMemo } from 'react';
import { hide, show } from '@tauri-apps/api/app';
import { getCurrentWindow, ProgressBarStatus, UserAttentionType } from '@tauri-apps/api/window';
import { platform } from '@tauri-apps/plugin-os';
import { moveWindow, Position } from '@tauri-apps/plugin-positioner';

/**
 * Imperative service hook — the ONLY place in the renderer that imports
 * @tauri-apps/api/window, @tauri-apps/api/app, @tauri-apps/plugin-os, and
 * @tauri-apps/plugin-positioner. Exposes fire-and-forget shell actions; this is
 * not data-fetching so no React Query wrapper is needed.
 */

export function useWindowControls() {
  return useMemo(() => {
    // True only inside the Tauri webview; false in the Playwright/browser harness.
    // Evaluated inside useMemo (not module scope) so test beforeEach can set/clear
    // window.__TAURI_INTERNALS__ and the check sees the current value each render.
    if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
      // ponytail: browser/E2E harness — no Tauri runtime; degrade to no-ops so
      // the renderer boots. Keys + return types match the real branch.
      const noop = async () => {};
      return {
        isMacos: false,
        toggleMaximize: noop,
        isFocused: async () => true,
        foreground: noop,
        setTaskbarProgress: (_p: number | null): Promise<void> => Promise.resolve(),
        flashAttention: noop,
        resetPosition: noop,
        hideApp: noop,
        showApp: noop,
      };
    }

    const win = getCurrentWindow();
    return {
      isMacos: platform() === 'macos',
      toggleMaximize: () => win.toggleMaximize(),
      isFocused: () => win.isFocused(),
      /** Brings the window to front. ponytail: window is usually already focused
       *  when this is reachable; harmless no-op then. */
      foreground: async () => {
        await win.unminimize();
        await win.setFocus();
      },
      /**
       * Maps a 0..1 fraction (or sentinel) to the OS taskbar progress bar.
       * null → clear, <0 → indeterminate, else → normal with clamped percent.
       */
      setTaskbarProgress: (p: number | null): Promise<void> => {
        if (p === null) return win.setProgressBar({ status: ProgressBarStatus.None });
        if (p < 0) return win.setProgressBar({ status: ProgressBarStatus.Indeterminate });
        return win.setProgressBar({
          status: ProgressBarStatus.Normal,
          progress: Math.round(p * 100),
        });
      },
      flashAttention: () => win.requestUserAttention(UserAttentionType.Informational),
      /** Moves window to screen centre — off-screen recovery. */
      resetPosition: () => moveWindow(Position.Center),
      // ponytail: macOS only, contrived, duplicates ⌘H
      hideApp: () => hide(),
      // ponytail: macOS re-surface a hidden app
      showApp: () => show(),
    };
  }, []);
}
