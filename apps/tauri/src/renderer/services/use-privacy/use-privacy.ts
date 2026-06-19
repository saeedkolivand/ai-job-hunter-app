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
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.interactions() }),
  });
};

export const useResetApp = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const resetPreferences = usePreferencesStore((s) => s.resetPreferences);
  return useMutation({
    mutationFn: () => api.privacy.resetApp(),
    onSuccess: () => {
      qc.clear();
      resetPreferences();
      void clearOnboardingMirror();
    },
  });
};
