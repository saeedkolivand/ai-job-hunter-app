import { useMutation, useQuery } from '@tanstack/react-query';

import { type Locale, PROTOCOL_VERSION } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

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

export const useSetPerformanceMode = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: (mode: 'low-memory' | 'balanced' | 'performance') =>
      api.system.setPerformanceMode(mode),
  });
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

/** Check if Chrome/Edge is available for browser automation. */
export const useCheckBrowser = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.system.checkBrowser,
    queryFn: () => api.system.checkBrowser(),
    staleTime: Infinity,
  });
};
