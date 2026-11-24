import { useEffect, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { PortablePrefsSnapshot } from '@ajh/shared';
import { applyTheme, getThemePrefs, onThemeApplied } from '@ajh/ui';

import { useAppClient } from '@/providers/AppClientProvider';
import { usePreferencesStore } from '@/store/preferences-store';

import { keys } from '../query-client';
import {
  changedKeys,
  hydrationOf,
  localForMigration,
  portableOf,
  type PortableValues,
  same,
} from './portable-prefs';

/** The backend-owned settings that sync across devices (#1433). */
export const usePortablePrefs = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.portablePrefs.all,
    queryFn: () => api.portablePrefs.get(),
  });
};

/**
 * Write the user's changes through. The cache is updated up front (not
 * invalidated), so a refetch racing a keystroke can never put an older value back.
 */
export const useSetPortablePrefs = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (values: PortableValues) => api.portablePrefs.set(values),
    onMutate: (values) =>
      qc.setQueryData<PortablePrefsSnapshot>(keys.portablePrefs.all, (old) =>
        old ? { ...old, values: { ...old.values, ...values } } : old
      ),
  });
};

/** One-time copy of the renderer's own storage; the backend keeps anything it already holds. */
export const useMigratePortablePrefs = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (values: PortableValues) => api.portablePrefs.migrate(values),
    onSuccess: (data) => qc.setQueryData(keys.portablePrefs.all, data),
  });
};

/**
 * Mount in the app root. Migrates the old localStorage values once, applies the
 * backend values (stored, or received by sync) to the store and theme, and
 * writes the user's later edits through. Only edits are written, never defaults.
 */
export const usePortablePrefsSync = () => {
  const { data } = usePortablePrefs();
  const { mutate: set } = useSetPortablePrefs();
  const { mutate: migrate } = useMigratePortablePrefs();
  const applying = useRef(false);
  const lastTheme = useRef(getThemePrefs());
  const migrated = data?.migrated === true;

  useEffect(() => {
    if (data && !data.migrated) {
      migrate(localForMigration(usePreferencesStore.getState(), getThemePrefs()));
    }
  }, [data, migrate]);

  useEffect(() => {
    if (!data?.migrated) return;
    const next = hydrationOf(data.values, usePreferencesStore.getState(), getThemePrefs());
    applying.current = true;
    try {
      if (next.patch) usePreferencesStore.setState(next.patch);
      if (next.theme) {
        lastTheme.current = next.theme;
        applyTheme(next.theme);
      }
    } finally {
      applying.current = false;
    }
  }, [data]);

  useEffect(() => {
    if (!migrated) return;
    const stopStore = usePreferencesStore.subscribe((state, prev) => {
      if (applying.current) return;
      const changes = changedKeys(portableOf(state), portableOf(prev));
      if (Object.keys(changes).length) set(changes);
    });
    const stopTheme = onThemeApplied((theme) => {
      if (applying.current || same(theme, lastTheme.current)) return;
      lastTheme.current = theme;
      set({ theme });
    });
    return () => {
      stopStore();
      stopTheme();
    };
  }, [migrated, set]);
};
