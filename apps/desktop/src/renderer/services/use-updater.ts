import { useCallback, useEffect, useState } from 'react';

export type UpdateStatus =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'available'; version: string; releaseNotes?: string }
  | { state: 'not-available' }
  | { state: 'downloading'; percent: number }
  | { state: 'downloaded'; version: string }
  | { state: 'error'; message: string };

export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>({ state: 'idle' });

  useEffect(() => {
    const off = window.api.updater.onStatus((s) => setStatus(s as UpdateStatus));
    return () => {
      off();
    };
  }, []);

  const check = useCallback(() => window.api.updater.check(), []);
  const download = useCallback(() => window.api.updater.download(), []);
  const install = useCallback(() => window.api.updater.install(), []);

  return { status, check, download, install };
}
