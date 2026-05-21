import { useMutation, useQuery } from '@tanstack/react-query';

import type { Locale } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys, queryClient } from './query-client';

export const useSystemHealth = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.system.health,
    queryFn: () => api.system.health(),
    refetchInterval: 15_000,
    staleTime: 10_000,
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
