import type { JobPreferences } from '../../schemas/index.js';

export interface JobPreferencesContract {
  get(): Promise<JobPreferences>;

  set(prefs: JobPreferences): Promise<void>;

  /**
   * Single-column salary-expectation write (review fix, PR #695) — unlike
   * `set()`, this NEVER touches `location`/`techStack`/`countryCode`. Callers
   * that only have the salary value on hand (not a freshly-read copy of the
   * other fields) MUST use this instead of `set({ ...maybeStaleOrUndefined,
   * salaryExpectation })`, which would silently NULL every other field when
   * the spread source is stale or hasn't loaded yet.
   */
  setSalaryExpectation(salaryExpectation: string | undefined): Promise<void>;
}

export const JOB_PREFERENCES_CHANNELS = {
  get: 'jobPreferences:get',
  set: 'jobPreferences:set',
  setSalaryExpectation: 'jobPreferences:setSalaryExpectation',
} as const;
