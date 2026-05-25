import type { JobPreferences } from '../../schemas/index.js';

export interface JobPreferencesContract {
  get(): Promise<JobPreferences>;

  set(prefs: JobPreferences): Promise<void>;
}

export const JOB_PREFERENCES_CHANNELS = {
  get: 'jobPreferences:get',
  set: 'jobPreferences:set',
} as const;
