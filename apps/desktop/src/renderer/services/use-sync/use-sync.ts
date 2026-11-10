import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { SyncConfigureRequest, SyncStatus } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

/**
 * Optional self-hosted sync (ADR-051). Every sync command echoes the fresh
 * status back, so mutations seed the cache directly rather than invalidating.
 * The status is refetched every minute while mounted (the app root mounts it via
 * `useSyncDataRefresh`), so background runs show up without a reload.
 */
export const useSyncStatus = () => {
  const api = useAppClient();
  return useQuery<SyncStatus>({
    queryKey: keys.sync.status,
    queryFn: () => api.sync.status(),
    refetchInterval: 60_000,
  });
};

/** Check the server, save URL + token (OS keychain), turn sync on, run once. */
export const useConfigureSync = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<SyncStatus, Error, SyncConfigureRequest>({
    mutationFn: (req) => api.sync.configure(req),
    onSuccess: (data) => qc.setQueryData(keys.sync.status, data),
  });
};

/** Turn sync off and remove the token. Local data is untouched. */
export const useDisableSync = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<SyncStatus>({
    mutationFn: () => api.sync.disable(),
    onSuccess: (data) => qc.setQueryData(keys.sync.status, data),
  });
};

/** Run one sync now. */
export const useSyncNow = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<SyncStatus>({
    mutationFn: () => api.sync.syncNow(),
    onSuccess: (data) => qc.setQueryData(keys.sync.status, data),
    // A failed run is also recorded in the status (lastError).
    onError: () => void qc.invalidateQueries({ queryKey: keys.sync.status }),
  });
};
