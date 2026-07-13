import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type {
  ExtensionAutofillSetting,
  ExtensionBridgeStatus,
  ExtensionBridgeTokenResult,
} from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys, QUERY_TIMES } from '../query-client';

/**
 * Live view of the local extension-bridge status (bound port, whether a browser
 * is paired, and the current pairing token).
 *
 * Polls every 30s: the bridge does not emit a dedicated connect/disconnect event
 * to the renderer (only `applications:changed` on a successful import), so a
 * modest interval is the lightest way to keep the connection indicator fresh.
 * 30s (not 5s) because this query mounts app-globally — the indicator is a
 * status hint, not a real-time signal, so a slower cadence trims background work
 * without a perceptible staleness cost.
 */
export const useExtensionBridgeStatus = () => {
  const api = useAppClient();
  return useQuery<ExtensionBridgeStatus>({
    queryKey: keys.extensionBridge.status,
    queryFn: () => api.extensionBridge.status(),
    refetchInterval: QUERY_TIMES.MEDIUM,
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

/**
 * Assisted-autofill opt-in (default OFF). Gates whether the extension's "Fill"
 * button can pull the contact profile from the desktop into the current page.
 */
export const useExtensionAutofillSetting = () => {
  const api = useAppClient();
  return useQuery<ExtensionAutofillSetting>({
    queryKey: keys.extensionBridge.autofill,
    queryFn: () => api.extensionBridge.autofillEnabled(),
  });
};

/** Toggle + persist the assisted-autofill opt-in; refreshes the cached setting. */
export const useSetExtensionAutofillSetting = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<ExtensionAutofillSetting, Error, boolean>({
    mutationFn: (enabled: boolean) => api.extensionBridge.setAutofillEnabled(enabled),
    onSuccess: (data) => {
      qc.setQueryData(keys.extensionBridge.autofill, data);
    },
  });
};
