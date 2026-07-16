import { useEffect, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { JobPreferences } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';
import { usePreferencesStore } from '@/store/preferences-store';

import { keys } from '../query-client';

export const useJobPreferences = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.jobPreferences.all,
    queryFn: () => api.jobPreferences.get(),
  });
};

export const useSetJobPreferences = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (prefs: JobPreferences) => api.jobPreferences.set(prefs),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.jobPreferences.all }),
  });
};

/**
 * Boot-time push of the renderer-only `applicant.salaryExpectation` onto the
 * backend-owned `job_preferences` store (Task #30) — the bridge's
 * `answers.suggest` reads it from there for the synthetic salary-question row
 * (a renderer-only value it has no read access to otherwise). Mirrors
 * `useSyncCloseToTray`'s boot-push shape: reads the persisted value once at
 * mount (not a reactive selector, so this fires exactly once) and aligns the
 * backend copy with it — existing users who set this before the field became
 * backend-readable get it synced on their next launch. A no-op when nothing
 * is saved. The settings field's own `onChange` (see `ApplicantDetailsSection`)
 * is what keeps it in sync on every SUBSEQUENT edit; this hook only covers the
 * gap for whatever was already saved. Mount once in an app-global root/provider.
 */
export const useSyncSalaryExpectation = () => {
  const api = useAppClient();
  const pushed = useRef(false);
  useEffect(() => {
    if (pushed.current) return;
    pushed.current = true;
    const salaryExpectation = usePreferencesStore.getState().applicant?.salaryExpectation?.trim();
    if (!salaryExpectation) return; // nothing saved yet — nothing to sync
    void api.jobPreferences.get().then((current) => {
      void api.jobPreferences.set({ ...current, salaryExpectation });
    });
  }, [api]);
};
