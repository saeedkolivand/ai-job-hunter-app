import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { useQuery } from '@tanstack/react-query';

import {
  calculateDownloadSpeed,
  calculateTimeRemaining,
  formatBytes,
  formatDownloadSpeed,
  formatTimeRemaining,
} from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys, QUERY_TIMES } from '../query-client';

export type UpdateStatus =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'available'; version: string; releaseNotes?: string }
  | { state: 'not-available' }
  | { state: 'downloading'; percent: number; downloaded?: number; total?: number }
  | { state: 'downloaded'; version: string }
  | { state: 'error'; message: string };

// `status` is shared module state, not local `useState`. There are three
// independent useUpdater() call sites (this settings panel, the always-mounted
// banner, and the menu), each with its OWN `updater:status` subscription; the
// banner/menu never unmount, so they never lose it, but the settings panel
// mounts/unmounts with route navigation. There is no backend command to ask
// "what's the status right now" — only this push event — so a fresh instance
// previously had no way to learn the truth and rendered a stale 'idle',
// including a live "Check now" button for an update that was already
// downloading or done. Not React Query: there is nothing to FETCH, only a
// pushed value to remember, so a plain synchronous external store (every
// mounted instance reads the SAME value, updated the instant any one of them
// receives an event) is the whole fix — no extra `check()` call, no re-fetch.
let sharedUpdateStatus: UpdateStatus = { state: 'idle' };
const updateStatusListeners = new Set<() => void>();
function setSharedUpdateStatus(next: UpdateStatus) {
  sharedUpdateStatus = next;
  updateStatusListeners.forEach((listener) => listener());
}
function subscribeToUpdateStatus(listener: () => void) {
  updateStatusListeners.add(listener);
  return () => updateStatusListeners.delete(listener);
}
function getSharedUpdateStatus() {
  return sharedUpdateStatus;
}

/** Test-only: reset the shared status between tests (module state persists across `it()`s). */
export function resetUpdaterStatusForTests() {
  sharedUpdateStatus = { state: 'idle' };
}

export function useUpdater() {
  const api = useAppClient();
  const status = useSyncExternalStore(subscribeToUpdateStatus, getSharedUpdateStatus);
  const [downloadSpeed, setDownloadSpeed] = useState<string>('');
  const [downloadedBytes, setDownloadedBytes] = useState<number>(0);
  const [totalBytes, setTotalBytes] = useState<number>(0);
  const [timeRemaining, setTimeRemaining] = useState<string>('');

  const prevBytesRef = useRef(0);
  const prevTimeRef = useRef(0);
  const lastSpeedUpdateRef = useRef(0);
  const lastTimeUpdateRef = useRef(0);

  useEffect(() => {
    const off = api.updater.onStatus((s: unknown) => {
      const newStatus = s as UpdateStatus;
      setSharedUpdateStatus(newStatus);

      // Track download metrics
      if (newStatus.state === 'downloading') {
        const now = Date.now();
        const bytes = newStatus.downloaded ?? 0;
        const total = newStatus.total ?? 0;

        setDownloadedBytes(bytes);
        setTotalBytes(total);

        // Calculate download speed
        if (prevTimeRef.current > 0 && bytes > prevBytesRef.current) {
          const bytesPerSecond = calculateDownloadSpeed(
            bytes,
            prevBytesRef.current,
            now,
            prevTimeRef.current
          );

          if (bytesPerSecond > 0) {
            // Throttle speed updates to every 500ms
            if (now - lastSpeedUpdateRef.current > 500) {
              setDownloadSpeed(formatDownloadSpeed(bytesPerSecond));
              lastSpeedUpdateRef.current = now;
            }

            // Calculate time remaining (throttled to 500ms)
            if (total > 0 && bytes > 0 && bytes < total) {
              if (now - lastTimeUpdateRef.current > 500) {
                const remainingSeconds = calculateTimeRemaining(total, bytes, bytesPerSecond);
                setTimeRemaining(formatTimeRemaining(remainingSeconds));
                lastTimeUpdateRef.current = now;
              }
            }
          }
        }

        prevBytesRef.current = bytes;
        prevTimeRef.current = now;
      } else if (newStatus.state === 'downloaded' || newStatus.state === 'error') {
        // Reset download state
        setDownloadSpeed('');
        setDownloadedBytes(0);
        setTotalBytes(0);
        setTimeRemaining('');
        prevBytesRef.current = 0;
        prevTimeRef.current = 0;
        lastSpeedUpdateRef.current = 0;
        lastTimeUpdateRef.current = 0;
      }
    });
    return () => {
      off();
    };
  }, [api]);

  const check = useCallback(() => api.updater.check(), [api]);
  const download = useCallback(() => api.updater.download(), [api]);
  const install = useCallback(() => api.updater.install(), [api]);

  return {
    status,
    check,
    download,
    install,
    downloadSpeed,
    downloadedBytes,
    totalBytes,
    timeRemaining,
    formatBytes,
  };
}

/**
 * Recent release history (current + previous versions) for the in-app changelog.
 * Fetched lazily — pass `enabled` so the GitHub round-trip only happens once the
 * user expands the changelog. Release data changes rarely, so it stays fresh for
 * 10 minutes.
 */
export function useChangelog(enabled: boolean) {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.updater.changelog,
    queryFn: () => api.updater.changelog(),
    enabled,
    staleTime: QUERY_TIMES.TEN_MIN,
  });
}
