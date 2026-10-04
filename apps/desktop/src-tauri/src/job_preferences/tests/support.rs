//! Fixtures shared by the `job_preferences` store tests.

use super::*;

/// A fresh store in a temp dir. Hold the guard for as long as the store is used.
pub(super) fn open_store() -> (TempDir, JobPreferencesStore) {
    let dir = TempDir::new().unwrap();
    let store = JobPreferencesStore::open(&dir.path().to_path_buf()).unwrap();
    (dir, store)
}

/// A row with every field unset; tests override only what they exercise.
pub(super) fn blank() -> JobPreferences {
    JobPreferences {
        location: None,
        country_code: None,
        tech_stack: None,
        salary_expectation: None,
        extra_agency_companies: None,
    }
}
