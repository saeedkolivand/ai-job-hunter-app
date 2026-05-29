import { type ReactNode, useEffect, useRef } from 'react';

import { useAppClient } from '@/providers/AppClientProvider';
import { usePerformanceMode } from '@/store/preferences-store';

/**
 * Syncs the renderer's performanceMode preference to two side effects:
 *   1. data-performance-mode attribute on <html> — drives CSS-level blur
 *      reduction.
 *   2. system.setPerformanceMode IPC — lets the main process adjust JobQueue
 *      concurrency and AiRuntime idle-unload timeout accordingly.
 *
 * Place this inside <AppClientProvider> so getClient() is always ready.
 */
export function PerformanceModeProvider({ children }: { children: ReactNode }) {
  const client = useAppClient();
  const mode = usePerformanceMode();
  const prevMode = useRef<string | null>(null);

  useEffect(() => {
    if (mode === prevMode.current) return;
    prevMode.current = mode;

    document.documentElement.setAttribute('data-performance-mode', mode);

    client.system.setPerformanceMode(mode).catch(() => {
      // Silently ignore — main process may not be ready on first render.
    });
  }, [client, mode]);

  return <>{children}</>;
}
