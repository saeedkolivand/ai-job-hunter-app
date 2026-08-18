import { useCallback, useEffect, useSyncExternalStore } from 'react';
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

interface UpdaterSnapshot {
  status: UpdateStatus;
  downloadSpeed: string;
  timeRemaining: string;
}

// `status` AND the download-progress readout (speed, time remaining) are
// shared MODULE state, not local `useState`/`useRef`. There are three
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
//
// `downloadedBytes`/`totalBytes` don't get a store slot at all: the
// `downloading` status variant already carries `.downloaded`/`.total`, so
// every instance derives them straight off the (already-shared) `status`
// below — one source of truth instead of a second copy that can fall out of
// sync with it.
//
// `downloadSpeed`/`timeRemaining` are rate calculations, so unlike the byte
// counts they need HISTORY (a previous sample + its timestamp) to compute.
// That history (`prevBytes`/`prevTime` below) is module-level too, for the
// same reason `status` is: a settings panel that unmounts mid-download and
// remounts must not restart its own blank history and sit through one more
// silent tick before the first speed reading appears. Because the
// always-mounted banner keeps an `updater.onStatus` listener alive for the
// whole download, this history is never actually gapped by a remount in
// practice — a remount only adds a second listener recomputing the same
// delta from the same shared previous sample, which is idempotent (once the
// first listener advances `prevBytes` for an event, the rest see
// `bytes === prevBytes` and skip the calculation). Reset on
// `downloaded`/`error` keeps a finished download from leaking a stale sample
// into the next one.
let sharedSnapshot: UpdaterSnapshot = {
  status: { state: 'idle' },
  downloadSpeed: '',
  timeRemaining: '',
};
let prevBytes = 0;
let prevTime = 0;
let lastSpeedUpdate = 0;
let lastTimeUpdate = 0;

const updateStatusListeners = new Set<() => void>();
function setSharedSnapshot(next: UpdaterSnapshot) {
  sharedSnapshot = next;
  updateStatusListeners.forEach((listener) => listener());
}
function subscribeToUpdateStatus(listener: () => void) {
  updateStatusListeners.add(listener);
  return () => updateStatusListeners.delete(listener);
}
function getSharedSnapshot() {
  return sharedSnapshot;
}

function recordStatus(newStatus: UpdateStatus) {
  let downloadSpeed = sharedSnapshot.downloadSpeed;
  let timeRemaining = sharedSnapshot.timeRemaining;

  if (newStatus.state === 'downloading') {
    const now = Date.now();
    const bytes = newStatus.downloaded ?? 0;
    const total = newStatus.total ?? 0;

    if (prevTime > 0 && bytes > prevBytes) {
      const bytesPerSecond = calculateDownloadSpeed(bytes, prevBytes, now, prevTime);

      if (bytesPerSecond > 0) {
        // Throttle speed updates to every 500ms
        if (now - lastSpeedUpdate > 500) {
          downloadSpeed = formatDownloadSpeed(bytesPerSecond);
          lastSpeedUpdate = now;
        }

        // Calculate time remaining (throttled to 500ms)
        if (total > 0 && bytes > 0 && bytes < total && now - lastTimeUpdate > 500) {
          timeRemaining = formatTimeRemaining(calculateTimeRemaining(total, bytes, bytesPerSecond));
          lastTimeUpdate = now;
        }
      }
    }

    prevBytes = bytes;
    prevTime = now;
  } else if (newStatus.state === 'downloaded' || newStatus.state === 'error') {
    downloadSpeed = '';
    timeRemaining = '';
    prevBytes = 0;
    prevTime = 0;
    lastSpeedUpdate = 0;
    lastTimeUpdate = 0;
  }

  setSharedSnapshot({ status: newStatus, downloadSpeed, timeRemaining });
}

/** Test-only: reset the shared status between tests (module state persists across `it()`s). */
export function resetUpdaterStatusForTests() {
  sharedSnapshot = { status: { state: 'idle' }, downloadSpeed: '', timeRemaining: '' };
  prevBytes = 0;
  prevTime = 0;
  lastSpeedUpdate = 0;
  lastTimeUpdate = 0;
}

export function useUpdater() {
  const api = useAppClient();
  const { status, downloadSpeed, timeRemaining } = useSyncExternalStore(
    subscribeToUpdateStatus,
    getSharedSnapshot
  );

  useEffect(() => {
    const off = api.updater.onStatus((s: unknown) => {
      recordStatus(s as UpdateStatus);
    });
    return () => {
      off();
    };
  }, [api]);

  const check = useCallback(() => api.updater.check(), [api]);
  const download = useCallback(() => api.updater.download(), [api]);
  const install = useCallback(() => api.updater.install(), [api]);

  const downloadedBytes = status.state === 'downloading' ? (status.downloaded ?? 0) : 0;
  const totalBytes = status.state === 'downloading' ? (status.total ?? 0) : 0;

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
