import { useEffect, useRef } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';

import { type Locale, PROTOCOL_VERSION } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';
import { usePreferencesStore } from '@/store/preferences-store';

import { keys, queryClient } from '../query-client';

export const useSystemHealth = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.system.health,
    queryFn: () => api.system.health(),
    refetchInterval: 5_000,
    staleTime: 4_000,
  });
};

export const useAppVersion = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.system.version,
    queryFn: () => api.system.getVersion(),
    staleTime: Infinity,
  });
};

export interface ProtocolVersionCheck {
  /** True once the backend version has been fetched and compared. */
  checked: boolean;
  /** True when the backend version differs from the renderer's expected version. */
  mismatch: boolean;
  expected: string;
  actual?: string;
}

/**
 * Boot-time IPC contract handshake. Renderer and Rust ship in one binary, so a
 * mismatch only happens with a stale webview cache or partial install — in that
 * state IPC calls may silently misbehave, so we surface it as a hard error.
 */
export const useProtocolVersionCheck = (): ProtocolVersionCheck => {
  const api = useAppClient();
  const query = useQuery({
    queryKey: keys.system.protocolVersion,
    queryFn: () => api.system.getProtocolVersion(),
    staleTime: Infinity,
    retry: false,
  });

  return {
    checked: query.isSuccess,
    mismatch: query.isSuccess && query.data !== PROTOCOL_VERSION,
    expected: PROTOCOL_VERSION,
    actual: query.data,
  };
};

export const useGetPlatform = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.system.platform,
    queryFn: () => api.system.getPlatform(),
    staleTime: Infinity,
  });
};

export const useSetLocale = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (locale: Locale) => api.system.setLocale(locale) });
};

export const useOpenExternal = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (url: string) => api.system.openExternal(url) });
};

/** Open the webview devtools (Developer settings). On-demand, so a mutation. */
export const useOpenDevtools = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: () => api.system.openDevtools() });
};

/** Current launch-at-login state, sourced from the OS (not local prefs). */
export const useLaunchAtLogin = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.system.launchAtLogin,
    queryFn: () => api.system.getLaunchAtLogin(),
    staleTime: Infinity,
  });
};

/** Toggle launch-at-login; writes the resulting OS state straight into the cache. */
export const useSetLaunchAtLogin = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: (enabled: boolean) => api.system.setLaunchAtLogin(enabled),
    onSuccess: (actual) => queryClient.setQueryData(keys.system.launchAtLogin, actual),
  });
};

/**
 * Toggle close-to-tray: persist the preference (source of truth) AND push the
 * live flag to the shell so the window-close handler reflects it immediately.
 */
export const useSetCloseToTray = () => {
  const api = useAppClient();
  const setCloseToTray = usePreferencesStore((s) => s.setCloseToTray);
  return useMutation({
    mutationFn: (enabled: boolean) => api.system.setCloseToTray(enabled),
    // Flip the persisted preference only after the backend push succeeded, so a
    // rejected IPC can't leave the store and the Rust flag diverged.
    onSuccess: (_d, enabled) => setCloseToTray(enabled),
  });
};

/**
 * Boot-time push of the persisted close-to-tray preference to the shell. The
 * Rust flag defaults to `true`; this aligns it with the user's stored choice
 * once on mount (mirrors how launch-at-login is reconciled at startup). Mount in
 * an app-global root/provider.
 */
export const useSyncCloseToTray = () => {
  const api = useAppClient();
  const pushed = useRef(false);
  useEffect(() => {
    if (pushed.current) return;
    pushed.current = true;
    // Read once at mount (not via a reactive selector) so this fires exactly once
    // and doesn't re-push on every preference change — the toggle handles changes.
    const enabled = usePreferencesStore.getState().closeToTray ?? true;
    void api.system.setCloseToTray(enabled);
  }, [api]);
};

/** Convenience: invalidate health cache to force an immediate recheck. */
export const invalidateHealth = () =>
  queryClient.invalidateQueries({ queryKey: keys.system.health });

/** Full process/boot/queue metrics — refreshed every 30 s. */
export const useSystemMetrics = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.system.metrics,
    queryFn: () => api.system.getMetrics(),
    refetchInterval: 30_000,
    staleTime: 20_000,
  });
};

/**
 * OS accent color for the 'System' accent source. `supported` is false on
 * platforms we can't read (Linux) — the Appearance UI hides the System option
 * rather than showing an error. Accent rarely changes, so it's cached.
 */
export const useSystemAccent = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.system.accent,
    queryFn: () => api.system.accentColor(),
    staleTime: Infinity,
  });
};

/** Check if Chrome/Edge is available for browser automation. */
export const useCheckBrowser = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.system.checkBrowser,
    queryFn: () => api.system.checkBrowser(),
    staleTime: Infinity,
  });
};
