//! Shared test-only fixtures reused across many `extension_bridge` submodules' own test files —
//! split out of `import_tests.rs`/`test.rs` during their R8 redistribution rather than duplicated
//! into every destination (DRY: one builder, not N hand-typed copies). `#[cfg(test)]` throughout;
//! never linked into a release build.

#![cfg(test)]

use tempfile::TempDir;

use super::BridgeState;
use crate::applications::{ApplicationMeta, ApplicationStore};

/// A fresh, empty `ApplicationStore` under its own temp dir.
pub(super) fn open_store() -> (TempDir, ApplicationStore) {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    (dir, store)
}

/// A fresh bridge state with a known persisted token (a temp data dir).
pub(super) fn bridge_state() -> (TempDir, BridgeState) {
    let dir = TempDir::new().unwrap();
    let state = BridgeState::load(dir.path());
    (dir, state)
}

/// A minimal `ApplicationMeta` fixture — every field the caller doesn't name defaults empty/None.
pub(super) fn app_meta(company: &str, title: &str) -> ApplicationMeta {
    ApplicationMeta {
        company: company.into(),
        title: title.into(),
        candidate: "Test User".into(),
        brief: String::new(),
        job_description: String::new(),
        answers: vec![],
        job_summary: String::new(),
        salary_min: None,
        salary_max: None,
        salary_currency: None,
    }
}

/// A minimal `JobPosting` fixture for a given url/company/title — every other field defaults
/// empty/None.
pub(super) fn sample_posting(
    url: &str,
    company: &str,
    title: &str,
) -> crate::scraping::types::JobPosting {
    crate::scraping::types::JobPosting {
        id: "p1".into(),
        external_id: None,
        title: title.into(),
        company: company.into(),
        location: None,
        url: url.into(),
        source: "linkedin".into(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: std::collections::HashMap::new(),
    }
}
