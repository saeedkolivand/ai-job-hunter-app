import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { ExtensionBridgeStatus, ExtensionBridgeTokenResult } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

/**
 * Live view of the local extension-bridge status (bound port, whether a browser
 * is paired, and the current pairing token).
 *
 * Polls every 5s: the bridge does not emit a dedicated connect/disconnect event
 * to the renderer (only `applications:changed` on a successful import), so a
 * modest interval is the lightest way to keep the connection indicator fresh.
 */
export const useExtensionBridgeStatus = () => {
  const api = useAppClient();
  return useQuery<ExtensionBridgeStatus>({
    queryKey: keys.extensionBridge.status,
    queryFn: () => api.extensionBridge.status(),
    refetchInterval: 5_000,
  });
};

/**
 * Rotate the pairing token. Invalidates the status query so the displayed token
 * (and connection state — existing sockets must re-pair) refreshes immediately.
 */
export const useRegenerateExtensionToken = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<ExtensionBridgeTokenResult>({
    mutationFn: () => api.extensionBridge.regenerateToken(),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.extensionBridge.status });
    },
  });
};
