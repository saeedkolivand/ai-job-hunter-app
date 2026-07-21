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

  /**
   * Single-column extra-agency-companies write (ADR-029 §i) — like
   * {@link setSalaryExpectation}, this NEVER touches the other columns, so an
   * agency-list edit can't NULL the user's saved location/techStack/countryCode/
   * salaryExpectation via a stale spread (PR #695 pattern). `undefined`/empty
   * clears the list.
   */
  setExtraAgencyCompanies(companies: string[] | undefined): Promise<void>;
}

export const JOB_PREFERENCES_CHANNELS = {
  get: 'jobPreferences:get',
  set: 'jobPreferences:set',
  setSalaryExpectation: 'jobPreferences:setSalaryExpectation',
  setExtraAgencyCompanies: 'jobPreferences:setExtraAgencyCompanies',
} as const;
