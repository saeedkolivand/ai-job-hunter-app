import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { CrashReportingSettings } from '@ajh/shared';

import { clearOnboardingMirror } from '@/lib/onboarding-mirror';
import { useAppClient } from '@/providers/AppClientProvider';
import { usePreferencesStore } from '@/store/preferences-store';

import { keys } from '../query-client';

export const useSignOutAll = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.privacy.signOutAll(),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.credentials.all });
      void qc.invalidateQueries({ queryKey: ['boards'] });
    },
  });
};

export const useClearInteractions = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.privacy.clearInteractions(),
    // Invalidate the PREFIX so every typed interactions query ('viewed', 'opened', …)
    // refetches — keys.postings.interactions(type) = ['postings','interactions',type]
    // and React Query matches on prefix, so omitting the type segment hits all of them.
    onSuccess: () => qc.invalidateQueries({ queryKey: ['postings', 'interactions'] }),
  });
};

/**
 * Crash-reporting consent. Rust-owned (the Sentry client is built before the
 * WebView exists), so this reads through IPC rather than the preferences store.
 */
export const useCrashReporting = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.privacy.crashReporting,
    queryFn: () => api.privacy.getCrashReporting(),
  });
};

export const useSetCrashReporting = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (settings: CrashReportingSettings) => api.privacy.setCrashReporting(settings),
    // Seed the cache from the command's own return value rather than refetching:
    // the Rust side is the authority on what was actually persisted, so echoing
    // it back avoids a toggle that snaps back on a failed write.
    onSuccess: (saved) => qc.setQueryData(keys.privacy.crashReporting, saved),
  });
};

export const useResetApp = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const resetPreferences = usePreferencesStore((s) => s.resetPreferences);
  return useMutation({
    mutationFn: () => api.privacy.resetApp(),
    onSuccess: async () => {
      qc.clear();
      resetPreferences();
      await clearOnboardingMirror();
    },
  });
};
