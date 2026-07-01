import { useMutation, useQueryClient } from '@tanstack/react-query';

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
