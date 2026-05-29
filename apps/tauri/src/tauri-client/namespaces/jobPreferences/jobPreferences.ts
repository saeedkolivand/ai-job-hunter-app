import { invoke } from '@tauri-apps/api/core';

export const jobPreferences = {
  get: () => invoke('job_preferences_get'),
  set: (prefs: unknown) => invoke('job_preferences_set', { prefs }),
};
