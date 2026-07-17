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
 * Write-through for `applicant.salaryExpectation` only (review fix, PR #695)
 * — a single-column backend write via `setSalaryExpectation`, NEVER the
 * full-row `set()` merged with a possibly-stale/not-yet-loaded
 * `useJobPreferences` cache (that full-row write would silently NULL
 * `location`/`techStack`/`countryCode` whenever this fired before the query
 * had loaded). Used by `ApplicantDetailsSection`'s onChange; invalidates the
 * shared `jobPreferences` query on success so a concurrently-mounted
 * `useJobPreferences` never serves the pre-write value.
 */
export const useSetSalaryExpectation = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (salaryExpectation: string | undefined) =>
      api.jobPreferences.setSalaryExpectation(salaryExpectation),
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
 *
 * Uses the SAME single-column `setSalaryExpectation` write as the onChange
 * path (review fix, PR #695) — never the full-row `set()`, and invalidates
 * the query cache on success so a concurrently-mounting `useJobPreferences`
 * can't cache the pre-sync value.
 *
 * Best-effort: `.catch` swallows a rejected `setSalaryExpectation` (or the
 * invalidate, though that itself can't reject) — `pushed` is already set by
 * the time either runs, so a failure here is never retried; the impact is
 * only "not synced this launch" (the value stays in the renderer store and
 * the ordinary onChange path still fires normally on the next edit), never
 * data loss. Without this, a rejection would surface as an unhandled promise
 * rejection (review fix, PR #695).
 */
export const useSyncSalaryExpectation = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  const pushed = useRef(false);
  useEffect(() => {
    if (pushed.current) return;
    pushed.current = true;
    const salaryExpectation = usePreferencesStore.getState().applicant?.salaryExpectation?.trim();
    if (!salaryExpectation) return; // nothing saved yet — nothing to sync
    void api.jobPreferences
      .setSalaryExpectation(salaryExpectation)
      .then(() => qc.invalidateQueries({ queryKey: keys.jobPreferences.all }))
      .catch(() => {
        // Best-effort boot sync — see the doc above. Never surfaced to the
        // user, never retried this session.
      });
  }, [api, qc]);
};
