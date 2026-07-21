import { invoke } from '@tauri-apps/api/core';

export const jobPreferences = {
  get: () => invoke('job_preferences_get'),
  set: (prefs: unknown) => invoke('job_preferences_set', { prefs }),
  setSalaryExpectation: (salaryExpectation: string | undefined) =>
    invoke('job_preferences_set_salary_expectation', { salaryExpectation }),
  setExtraAgencyCompanies: (companies: string[] | undefined) =>
    invoke('job_preferences_set_extra_agency_companies', { companies }),
};
