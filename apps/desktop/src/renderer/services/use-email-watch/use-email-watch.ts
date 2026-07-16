import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { EmailWatchConnectRequest, EmailWatchStatus } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

/**
 * Email-confirmation watching (Task #23, auto-track Layer C) — PR A scope:
 * connect/status/enable/disconnect + a manual re-check. No poller yet (PR B);
 * `enabled` is the standing opt-in the future poller will read.
 */
export const useEmailWatchStatus = () => {
  const api = useAppClient();
  return useQuery<EmailWatchStatus>({
    queryKey: keys.emailWatch.status,
    queryFn: () => api.emailWatch.status(),
  });
};

/**
 * Validate + persist a Gmail app-password connection. Slow (seconds): the
 * backend performs a real IMAP LOGIN + SELECT INBOX before storing anything.
 * Every emailWatch command echoes the fresh status back, so `onSuccess` seeds
 * the cache directly rather than invalidating.
 */
export const useConnectEmailWatch = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<EmailWatchStatus, Error, EmailWatchConnectRequest>({
    mutationFn: (req) => api.emailWatch.connect(req),
    onSuccess: (data) => qc.setQueryData(keys.emailWatch.status, data),
  });
};

/** Removes the keychain app password and clears the account row. */
export const useDisconnectEmailWatch = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<EmailWatchStatus>({
    mutationFn: () => api.emailWatch.disconnect(),
    onSuccess: (data) => qc.setQueryData(keys.emailWatch.status, data),
  });
};

/** Toggle the (future) poller opt-in — independent of the connection itself. */
export const useSetEmailWatchEnabled = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<EmailWatchStatus, Error, boolean>({
    mutationFn: (enabled) => api.emailWatch.setEnabled(enabled),
    onSuccess: (data) => qc.setQueryData(keys.emailWatch.status, data),
  });
};

/** Manual re-check: re-validates the existing connection, updates lastCheckAt. */
export const useEmailWatchCheckNow = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<EmailWatchStatus>({
    mutationFn: () => api.emailWatch.checkNow(),
    onSuccess: (data) => qc.setQueryData(keys.emailWatch.status, data),
  });
};
