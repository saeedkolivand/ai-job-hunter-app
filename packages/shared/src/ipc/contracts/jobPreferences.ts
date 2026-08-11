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

  /**
   * Single-column mirror of the renderer's `semanticScoring` preference
   * (ADR-020 addendum). The setting itself lives in the webview's
   * `localStorage`, which no Rust code can read — the headless Autopilot
   * scheduler needs this copy to decide whether to run its semantic re-rank.
   * Write-only from the renderer's perspective (the preference store stays the
   * source of truth); like the two setters above it NEVER touches another
   * column.
   */
  setSemanticScoring(enabled: boolean): Promise<void>;
}

export const JOB_PREFERENCES_CHANNELS = {
  get: 'jobPreferences:get',
  set: 'jobPreferences:set',
  setSalaryExpectation: 'jobPreferences:setSalaryExpectation',
  setExtraAgencyCompanies: 'jobPreferences:setExtraAgencyCompanies',
  setSemanticScoring: 'jobPreferences:setSemanticScoring',
} as const;
