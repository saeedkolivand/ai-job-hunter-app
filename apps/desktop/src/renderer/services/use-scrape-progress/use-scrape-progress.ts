import { useEffect, useRef, useState } from 'react';

import type { ScrapeProgressEvent } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

/**
 * Live progress for the active board scrape. Subscribes to `scrape:progress`
 * through the app-client port (Ports & Adapters — never a raw `listen()`), and
 * exposes the latest fraction (0..1, boards-done/total) reported for
 * `activeJobId`. Events for other/stale job ids are ignored.
 *
 * Returns `null` before the first progress event and whenever `activeJobId`
 * changes or clears — so a new scrape starts fresh and a finished scrape (id →
 * null) resets automatically.
 */
export function useScrapeProgress(activeJobId: string | null): number | null {
  const api = useAppClient();
  const [progress, setProgress] = useState<number | null>(null);

  // Read the current active id inside the listener without re-subscribing on
  // every scrape (one long-lived listener, autopilot-style).
  const activeRef = useRef(activeJobId);
  activeRef.current = activeJobId;

  // Reset on a new scrape (id changes) and on completion (id → null).
  useEffect(() => {
    setProgress(null);
  }, [activeJobId]);

  useEffect(() => {
    const off = api.scrape.onProgress((e: ScrapeProgressEvent) => {
      if (!activeRef.current || e.jobId !== activeRef.current) return;
      setProgress(e.progress);
    });
    return () => off();
  }, [api]);

  return progress;
}
