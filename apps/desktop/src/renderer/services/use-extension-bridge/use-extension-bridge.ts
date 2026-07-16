import { useEffect, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type {
  ExtensionAiAssistSetting,
  ExtensionAutofillSetting,
  ExtensionAutoTrackSetting,
  ExtensionBridgeChangedEvent,
  ExtensionBridgeStatus,
  ExtensionBridgeTokenResult,
} from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys, QUERY_TIMES } from '../query-client';

/**
 * Live view of the local extension-bridge status (bound port, whether a browser
 * is paired, and the current pairing token).
 *
 * Polls every 30s as a fallback — {@link useExtensionBridgeEvents} invalidates
 * this query immediately on the bridge's `extensionBridge:changed` push, so the
 * poll only covers a missed/dropped event (e.g. the window was backgrounded).
 * 30s (not 5s) because this query mounts app-globally — the indicator is a
 * status hint, not a real-time signal, so a slower fallback cadence trims
 * background work without a perceptible staleness cost.
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
 * App-global subscription to the bridge's live-connection transitions
 * (`extensionBridge:changed`, fired on a 0→1 / →0 count change — see
 * `extension_bridge::EXTENSION_BRIDGE_CHANGED`'s doc). Invalidates the status
 * query so the Settings pill flips instantly on pair/unpair instead of waiting
 * on the 30s poll.
 *
 * Mounted ONCE in the root layout (like `useApplicationEvents`); never call
 * from a feature component, or the listener would attach/detach per route.
 */
export const useExtensionBridgeEvents = (
  onChanged?: (event: ExtensionBridgeChangedEvent) => void
) => {
  const api = useAppClient();
  const qc = useQueryClient();
  // Keep the latest handler in a ref so the listener subscribes ONCE — re-subscribing
  // on every render races the async Tauri `listen` and can drop an event in the gap.
  const handlerRef = useRef(onChanged);
  handlerRef.current = onChanged;
  useEffect(() => {
    const off = api.extensionBridge.onChanged((event) => {
      void qc.invalidateQueries({ queryKey: keys.extensionBridge.status });
      handlerRef.current?.(event);
    });
    return () => off();
  }, [api, qc]);
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

/**
 * AI-answer-assist opt-in (default OFF) — a SEPARATE gate from
 * {@link useExtensionAutofillSetting}: billable provider spend, a
 * materially different consent class from the local/free autofill verbs.
 * A bare boolean: which provider/model a draft uses is the backend active
 * config (`useActiveConfig`), resolved at answer-time (task #16), so a
 * Settings row reads that store for its "Using: X · Y" label.
 */
export const useExtensionAiAssistSetting = () => {
  const api = useAppClient();
  return useQuery<ExtensionAiAssistSetting>({
    queryKey: keys.extensionBridge.aiAssist,
    queryFn: () => api.extensionBridge.aiAssistEnabled(),
  });
};

/**
 * Toggle + persist the AI-answer-assist opt-in; refreshes the cached setting.
 * A bare `{ enabled }` — the opt-in no longer snapshots a provider: a draft
 * resolves the active provider from the backend active config at answer-time
 * (task #16), so nothing more needs capturing here.
 */
export const useSetExtensionAiAssistSetting = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<ExtensionAiAssistSetting, Error, { enabled: boolean }>({
    mutationFn: ({ enabled }) => api.extensionBridge.setAiAssistEnabled(enabled),
    onSuccess: (data) => {
      qc.setQueryData(keys.extensionBridge.aiAssist, data);
    },
  });
};

/**
 * Auto-track opt-in (default OFF) — a SEPARATE gate from autofill/ai-assist
 * (Task #22). When on, the extension arms a gesture submit-watcher that
 * auto-marks a matched `saved` application `applied` on a detected form submit.
 * Desktop-enforced: the desktop also re-checks it before honoring the write.
 */
export const useExtensionAutoTrackSetting = () => {
  const api = useAppClient();
  return useQuery<ExtensionAutoTrackSetting>({
    queryKey: keys.extensionBridge.autoTrack,
    queryFn: () => api.extensionBridge.autoTrackEnabled(),
  });
};

/** Toggle + persist the auto-track opt-in; refreshes the cached setting. */
export const useSetExtensionAutoTrackSetting = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation<ExtensionAutoTrackSetting, Error, boolean>({
    mutationFn: (enabled: boolean) => api.extensionBridge.setAutoTrackEnabled(enabled),
    onSuccess: (data) => {
      qc.setQueryData(keys.extensionBridge.autoTrack, data);
    },
  });
};
