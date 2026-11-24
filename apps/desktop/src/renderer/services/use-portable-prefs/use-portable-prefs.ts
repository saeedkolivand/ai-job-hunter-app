import { useEffect, useRef } from 'react';
import { type QueryClient, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

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

/** Edits sent but not yet answered, per client (a failed one is dropped before the refetch). */
const writing = new WeakMap<QueryClient, Set<PortableValues>>();

/** The backend-owned settings that sync across devices (#1433). */
export const usePortablePrefs = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useQuery({
    queryKey: keys.portablePrefs.all,
    queryFn: async () => {
      const snap = await api.portablePrefs.get();
      // Edits still being written are newer than this read: a refetch racing a
      // keystroke must not put the older value back.
      const inFlight = [...(writing.get(qc) ?? [])];
      return inFlight.length
        ? { ...snap, values: Object.assign({}, snap.values, ...inFlight) }
        : snap;
    },
  });
};

/**
 * Write the user's changes through. The cache takes a value only once the
 * backend accepted it; a rejected write refetches the authoritative state.
 */
export const useSetPortablePrefs = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (values: PortableValues) => api.portablePrefs.set(values),
    // Drop any older read still in flight so it cannot land after this write.
    onMutate: (values) => {
      writing.set(qc, (writing.get(qc) ?? new Set()).add(values));
      return qc.cancelQueries({ queryKey: keys.portablePrefs.all });
    },
    onSuccess: async (_res, values) => {
      writing.get(qc)?.delete(values);
      await qc.cancelQueries({ queryKey: keys.portablePrefs.all });
      qc.setQueryData<PortablePrefsSnapshot>(keys.portablePrefs.all, (old) =>
        old ? { ...old, values: { ...old.values, ...values } } : old
      );
    },
    onError: (_err, values) => {
      writing.get(qc)?.delete(values);
      return qc.invalidateQueries({ queryKey: keys.portablePrefs.all });
    },
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
  // Edits made before the migration answered: they win over the snapshot.
  const buffered = useRef<PortableValues>({});
  const ready = useRef(false);

  useEffect(() => {
    if (data && !data.migrated) {
      migrate(localForMigration(usePreferencesStore.getState(), getThemePrefs()));
    }
  }, [data, migrate]);

  useEffect(() => {
    if (!data?.migrated) return;
    const edits = buffered.current;
    buffered.current = {};
    const next = hydrationOf(
      { ...data.values, ...edits },
      usePreferencesStore.getState(),
      getThemePrefs()
    );
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
    ready.current = true;
    if (Object.keys(edits).length) set(edits);
  }, [data, set]);

  useEffect(() => {
    const write = (changes: PortableValues) => {
      if (ready.current) set(changes);
      else buffered.current = { ...buffered.current, ...changes };
    };
    const stopStore = usePreferencesStore.subscribe((state, prev) => {
      if (applying.current) return;
      const changes = changedKeys(portableOf(state), portableOf(prev));
      if (Object.keys(changes).length) write(changes);
    });
    const stopTheme = onThemeApplied((theme) => {
      if (applying.current || same(theme, lastTheme.current)) return;
      lastTheme.current = theme;
      write({ theme });
    });
    return () => {
      stopStore();
      stopTheme();
    };
  }, [set]);
};
