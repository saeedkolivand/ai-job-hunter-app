import { useCallback, useEffect, useState } from 'react';

import { useAppClient } from '@/providers/AppClientProvider';

export type UpdateStatus =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'available'; version: string; releaseNotes?: string }
  | { state: 'not-available' }
  | { state: 'downloading'; percent: number }
  | { state: 'downloaded'; version: string }
  | { state: 'error'; message: string };

export function useUpdater() {
  const api = useAppClient();
  const [status, setStatus] = useState<UpdateStatus>({ state: 'idle' });

  useEffect(() => {
    const off = api.updater.onStatus((s: unknown) => setStatus(s as UpdateStatus));
    return () => {
      off();
    };
  }, [api]);

  const check = useCallback(() => api.updater.check(), [api]);
  const download = useCallback(() => api.updater.download(), [api]);
  const install = useCallback(() => api.updater.install(), [api]);

  return { status, check, download, install };
}
