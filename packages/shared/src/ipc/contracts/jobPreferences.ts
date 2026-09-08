import type { JobPreferences } from '../../schemas/index.js';

export interface JobPreferencesContract {
  get(): Promise<JobPreferences>;

  /**
   * Writes the preference row with MERGE semantics: a key the body OMITS keeps
   * its stored value, a key sent as explicit `null` CLEARS that column. Since
   * `JSON.stringify` drops `undefined` keys on the way to the command, an
   * `undefined` field reads as "leave it alone" — a caller clearing a field
   * MUST send `null` (hence the nullish fields on `JobPreferencesSchema`).
   * A field whose meaning depends on another (`countryCode` describes
   * `location`) is cleared together with it.
   */
  set(prefs: JobPreferences): Promise<void>;

  /**
   * Single-column salary-expectation write (review fix, PR #695) — unlike
   * `set()`, this addresses exactly one column, so it can never carry a stale
   * copy of another. `set()` merges (above), so `set({ ...maybeStaleOrUndefined,
   * salaryExpectation })` no longer NULLs the fields it omits — but a spread of
   * a STALE cache still overwrites every field it does carry with the stale
   * value. Callers that only have the salary value on hand (not a freshly-read
   * copy of the other fields) MUST therefore still use this.
   */
  setSalaryExpectation(salaryExpectation: string | undefined): Promise<void>;

  /**
   * Single-column extra-agency-companies write (ADR-029 §i) — like
   * {@link setSalaryExpectation}, this NEVER touches the other columns, so an
   * agency-list edit can't overwrite the user's saved location/techStack/
   * countryCode/salaryExpectation with a stale spread (PR #695 pattern).
   * `undefined`/empty clears the list.
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
