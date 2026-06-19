import { useMemo } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * Imperative service hook — the ONLY place in the renderer that imports
 * @tauri-apps/api/window. Exposes fire-and-forget shell actions; this is
 * not data-fetching so no React Query wrapper is needed.
 */
export function useWindowControls() {
  return useMemo(() => {
    const win = getCurrentWindow();
    return {
      toggleMaximize: () => win.toggleMaximize(),
    };
  }, []);
}
