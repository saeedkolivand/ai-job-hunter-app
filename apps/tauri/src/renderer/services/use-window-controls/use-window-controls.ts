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
      setTaskbarProgress: (p: number | null) => {
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
