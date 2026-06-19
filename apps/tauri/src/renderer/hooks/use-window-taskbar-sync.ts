import { useEffect, useRef } from 'react';

import type { JobEvent } from '@ajh/shared';

import { useJobEvents, useJobQueue } from '@/services/use-jobs/use-jobs';
import { useWindowControls } from '@/services/use-window-controls/use-window-controls';

/**
 * Side-effect hook — mounted ONCE in __root.tsx.
 * 1. Syncs running-job progress to the OS taskbar progress bar.
 * 2. Flashes window attention (and re-surfaces on macOS) when a job
 *    completes or fails while the window is not focused.
 */
export function useWindowTaskbarSync() {
  const controls = useWindowControls();
  const { data: jobs } = useJobQueue();
  // M3: explicit type includes undefined so first call always proceeds.
  const lastProgressRef = useRef<number | null | undefined>(undefined);

  // Progress sync: derive a single taskbar value from the live job list.
  useEffect(() => {
    const running = (jobs ?? []).filter((j) => j.status === 'running' || j.status === 'streaming');

    let next: number | null;
    if (running.length === 0) {
      next = null; // idle — clear bar
    } else {
      // Scraping jobs expose a 0..1 progress; AI streaming stays at 0.
      // Treat progress > 0 as determinate, otherwise show indeterminate (-1).
      const p = running[0]?.progress ?? 0;
      next = p > 0 ? p : -1;
    }

    // Guard redundant calls.
    if (next === lastProgressRef.current) return;
    lastProgressRef.current = next;
    void controls.setTaskbarProgress(next);

    // M2: clear taskbar bar on unmount while a job is still running.
    return () => {
      void controls.setTaskbarProgress(null);
    };
  }, [jobs, controls]);

  // Attention on job terminal events while window is unfocused.
  useJobEvents((event: JobEvent) => {
    if (event.type !== 'job.completed' && event.type !== 'job.failed') return;
    void (async () => {
      if (await controls.isFocused()) return;
      // H2: await showApp before flashing so attention fires on the visible window.
      if (controls.isMacos) await controls.showApp();
      void controls.flashAttention();
    })();
  });
}
