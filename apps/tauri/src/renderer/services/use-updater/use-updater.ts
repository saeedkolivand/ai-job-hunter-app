import { useCallback, useEffect, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';

import {
  calculateDownloadSpeed,
  calculateTimeRemaining,
  formatBytes,
  formatDownloadSpeed,
  formatTimeRemaining,
} from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

export type UpdateStatus =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'available'; version: string; releaseNotes?: string }
  | { state: 'not-available' }
  | { state: 'downloading'; percent: number; downloaded?: number; total?: number }
  | { state: 'downloaded'; version: string }
  | { state: 'error'; message: string };

export function useUpdater() {
  const api = useAppClient();
  const [status, setStatus] = useState<UpdateStatus>({ state: 'idle' });
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
      setStatus(newStatus);

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
    staleTime: 10 * 60_000,
  });
}
