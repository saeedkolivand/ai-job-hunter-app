import { useQuery, useMutation } from '@tanstack/react-query';
import { queryClient, keys } from './query-client';
import type { Locale } from '@ajh/shared';

export const useSystemHealth = () =>
  useQuery({
    queryKey: keys.system.health,
    queryFn: () => window.api.system.health(),
    refetchInterval: 15_000, // poll every 15s — matches old Sidebar / StatusBar intervals
    staleTime: 10_000,
  });

export const useAppVersion = () =>
  useQuery({
    queryKey: keys.system.version,
    queryFn: () => window.api.system.getVersion(),
    staleTime: Infinity, // version never changes at runtime
  });

export const useGetPlatform = () =>
  useQuery({
    queryKey: keys.system.platform,
    queryFn: () => window.api.system.getPlatform(),
    staleTime: Infinity, // platform never changes at runtime
  });

export const useSetLocale = () =>
  useMutation({
    mutationFn: (locale: Locale) => window.api.system.setLocale(locale),
  });

export const useOpenExternal = () =>
  useMutation({
    mutationFn: (url: string) => window.api.system.openExternal(url),
  });

/** Convenience: invalidate health cache to force an immediate recheck. */
export const invalidateHealth = () =>
  queryClient.invalidateQueries({ queryKey: keys.system.health });
