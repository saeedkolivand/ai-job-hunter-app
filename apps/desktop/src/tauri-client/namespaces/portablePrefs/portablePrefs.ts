import { invoke } from '@tauri-apps/api/core';

import type { PortablePrefsSnapshot } from '@ajh/shared/ipc';

export const portablePrefs = {
  get: () => invoke<PortablePrefsSnapshot>('portable_prefs_get'),
  set: (values: Record<string, unknown>) =>
    invoke<{ success: true }>('portable_prefs_set', { values }),
  migrate: (values: Record<string, unknown>) =>
    invoke<PortablePrefsSnapshot>('portable_prefs_migrate', { values }),
};
