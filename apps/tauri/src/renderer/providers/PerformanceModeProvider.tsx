import { MotionConfig } from 'motion/react';
import { type ReactNode, useEffect, useRef } from 'react';

import { useAppClient } from '@/providers/AppClientProvider';
import { usePerformanceMode } from '@/store/preferences-store';

/**
 * Syncs the renderer's performanceMode preference to three side effects:
 *   1. data-performance-mode attribute on <html> — drives CSS-level blur
 *      reduction and kills CSS transitions/keyframe animations.
 *   2. system.setPerformanceMode IPC — lets the main process adjust JobQueue
 *      concurrency and AiRuntime idle-unload timeout accordingly.
 *   3. MotionConfig reducedMotion — disables all Framer Motion JS animations
 *      in low-memory mode (CSS override alone does not reach them).
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

  return (
    <MotionConfig reducedMotion={mode === 'low-memory' ? 'always' : 'user'}>
      {children}
    </MotionConfig>
  );
}
